use std::collections::HashMap;
use std::rc::Rc;

use prost::Message;

use crate::error::Result;
use crate::io::ByteReader;

use super::field_decoder::{self, FieldMetadata};

use boon_proto::proto::{CDemoSendTables, CsvcMsgFlattenedSerializer};

/// Parsed type information from a var_type string.
#[derive(Debug, Clone)]
pub struct FieldType {
    pub base_type: String,
    pub pointer: bool,
    pub generic_type: Option<Box<FieldType>>,
    pub array_length: Option<usize>,
    pub count: Option<String>,
}

/// A single field within a serializer.
#[derive(Debug, Clone)]
pub struct SerializerField {
    pub var_type: String,
    pub var_name: String,
    pub bit_count: Option<i32>,
    pub low_value: Option<f32>,
    pub high_value: Option<f32>,
    pub encode_flags: Option<i32>,
    pub field_serializer_name: Option<String>,
    pub send_node: Option<String>,
    pub var_encoder: Option<String>,
    pub field_serializer: Option<Rc<Serializer>>,
    pub field_type: FieldType,
    pub metadata: FieldMetadata,
}

impl SerializerField {
    pub fn get_child(&self, index: usize) -> Option<&SerializerField> {
        self.field_serializer
            .as_ref()
            .and_then(|s| s.fields.get(index).map(|f| f.as_ref()))
    }

    pub fn is_dynamic_array(&self) -> bool {
        self.metadata.is_dynamic_array()
    }
}

/// A serializer: a named collection of fields describing an entity class.
#[derive(Debug, Clone)]
pub struct Serializer {
    pub name: String,
    pub fields: Vec<Rc<SerializerField>>,
}

/// Container holding all parsed serializers, indexed by name.
pub struct SerializerContainer {
    pub serializers: HashMap<String, Rc<Serializer>>,
}

impl SerializerContainer {
    /// Parse a CDemoSendTables message into a SerializerContainer.
    pub fn parse(cmd: CDemoSendTables) -> Result<Self> {
        let data = cmd.data.unwrap_or_default();
        let mut data_reader = ByteReader::new(&data);

        // Read varint size prefix, then decode the flattened serializer message
        let _size = data_reader.read_uvarint64()?;
        let remaining = data_reader.read_bytes(data_reader.remaining())?;
        let msg = CsvcMsgFlattenedSerializer::decode(remaining)?;

        let symbols = &msg.symbols;

        let resolve_sym = |i: i32| -> &str { &symbols[i as usize] };

        // Build fields and serializers
        let mut field_cache: HashMap<i32, Rc<SerializerField>> = HashMap::new();
        let mut serializer_map: HashMap<String, Rc<Serializer>> = HashMap::new();

        for serializer_proto in &msg.serializers {
            let ser_name = resolve_sym(serializer_proto.serializer_name_sym.unwrap_or(0));
            let mut serializer = Serializer {
                name: ser_name.to_string(),
                fields: Vec::with_capacity(serializer_proto.fields_index.len()),
            };

            for &field_index in &serializer_proto.fields_index {
                if let Some(cached) = field_cache.get(&field_index) {
                    serializer.fields.push(cached.clone());
                    continue;
                }

                let field_proto = &msg.fields[field_index as usize];

                let var_type = field_proto
                    .var_type_sym
                    .map(resolve_sym)
                    .unwrap_or("")
                    .to_string();
                let var_name = field_proto
                    .var_name_sym
                    .map(resolve_sym)
                    .unwrap_or("")
                    .to_string();
                let send_node = field_proto.send_node_sym.map(resolve_sym).map(String::from);
                let var_encoder = field_proto
                    .var_encoder_sym
                    .map(resolve_sym)
                    .map(String::from);
                let field_serializer_name = field_proto
                    .field_serializer_name_sym
                    .map(resolve_sym)
                    .map(String::from);

                let field_type = parse_type(&var_type);
                let metadata = field_decoder::get_field_metadata(
                    &var_type,
                    &var_name,
                    field_proto.bit_count,
                    field_proto.low_value,
                    field_proto.high_value,
                    field_proto.encode_flags,
                    var_encoder.as_deref(),
                    field_serializer_name.is_some(),
                );

                // Resolve field serializer
                let field_serializer = match &metadata {
                    fm if fm.is_fixed_array() => {
                        let length = fm.fixed_array_length().unwrap_or(0);
                        // Build a pseudo-serializer containing `length` copies of the inner field
                        let inner_ser = field_serializer_name
                            .as_deref()
                            .and_then(|n| serializer_map.get(n).cloned());

                        let inner_field = SerializerField {
                            var_type: var_type.clone(),
                            var_name: var_name.clone(),
                            bit_count: field_proto.bit_count,
                            low_value: field_proto.low_value,
                            high_value: field_proto.high_value,
                            encode_flags: field_proto.encode_flags,
                            field_serializer_name: field_serializer_name.clone(),
                            send_node: send_node.clone(),
                            var_encoder: var_encoder.clone(),
                            field_serializer: inner_ser,
                            field_type: field_type.clone(),
                            metadata: metadata.clone(),
                        };
                        let inner_rc = Rc::new(inner_field);
                        let mut fields = Vec::with_capacity(length);
                        fields.resize(length, inner_rc);
                        Some(Rc::new(Serializer {
                            name: String::new(),
                            fields,
                        }))
                    }
                    fm if fm.is_dynamic_array() => {
                        // For dynamic arrays of serializers, build a single-element serializer
                        if fm.is_dynamic_serializer_array() {
                            let inner_ser = field_serializer_name
                                .as_deref()
                                .and_then(|n| serializer_map.get(n).cloned());
                            let inner = Rc::new(SerializerField {
                                var_type: String::new(),
                                var_name: String::new(),
                                bit_count: None,
                                low_value: None,
                                high_value: None,
                                encode_flags: None,
                                field_serializer_name: None,
                                send_node: None,
                                var_encoder: None,
                                field_serializer: inner_ser,
                                field_type: parse_type(""),
                                metadata: FieldMetadata::default(),
                            });
                            Some(Rc::new(Serializer {
                                name: String::new(),
                                fields: vec![inner],
                            }))
                        } else {
                            // Dynamic array of primitives: single-element serializer with inner decoder
                            let inner_metadata = fm.dynamic_array_inner_metadata();
                            let inner = Rc::new(SerializerField {
                                var_type: String::new(),
                                var_name: String::new(),
                                bit_count: None,
                                low_value: None,
                                high_value: None,
                                encode_flags: None,
                                field_serializer_name: None,
                                send_node: None,
                                var_encoder: None,
                                field_serializer: None,
                                field_type: parse_type(""),
                                metadata: inner_metadata,
                            });
                            Some(Rc::new(Serializer {
                                name: String::new(),
                                fields: vec![inner],
                            }))
                        }
                    }
                    fm if fm.is_pointer() => field_serializer_name
                        .as_deref()
                        .and_then(|n| serializer_map.get(n).cloned()),
                    _ => field_serializer_name
                        .as_deref()
                        .and_then(|n| serializer_map.get(n).cloned()),
                };

                let field = Rc::new(SerializerField {
                    var_type,
                    var_name,
                    bit_count: field_proto.bit_count,
                    low_value: field_proto.low_value,
                    high_value: field_proto.high_value,
                    encode_flags: field_proto.encode_flags,
                    field_serializer_name,
                    send_node,
                    var_encoder,
                    field_serializer,
                    field_type,
                    metadata,
                });

                field_cache.insert(field_index, field.clone());
                serializer.fields.push(field);
            }

            serializer_map.insert(serializer.name.clone(), Rc::new(serializer));
        }

        Ok(Self {
            serializers: serializer_map,
        })
    }

    pub fn get(&self, name: &str) -> Option<Rc<Serializer>> {
        self.serializers.get(name).cloned()
    }
}

/// Parse a var_type string into a FieldType.
pub fn parse_type(s: &str) -> FieldType {
    let s = s.trim();

    // Check for pointer
    if s.ends_with('*') {
        return FieldType {
            base_type: s[..s.len() - 1].trim().to_string(),
            pointer: true,
            generic_type: None,
            array_length: None,
            count: None,
        };
    }

    // Check for array: type[length]
    if let Some(bracket_pos) = s.find('[') {
        if s.ends_with(']') {
            let base = s[..bracket_pos].trim();
            let len_str = s[bracket_pos + 1..s.len() - 1].trim();
            let array_length = len_str.parse::<usize>().ok();
            let count = if array_length.is_none() {
                Some(len_str.to_string())
            } else {
                None
            };
            return FieldType {
                base_type: base.to_string(),
                pointer: false,
                generic_type: None,
                array_length,
                count,
            };
        }
    }

    // Check for generic: Type< InnerType >
    if let Some(angle_pos) = s.find('<') {
        if let Some(close_pos) = s.rfind('>') {
            let base = s[..angle_pos].trim();
            let inner = s[angle_pos + 1..close_pos].trim();
            return FieldType {
                base_type: base.to_string(),
                pointer: false,
                generic_type: Some(Box::new(parse_type(inner))),
                array_length: None,
                count: None,
            };
        }
    }

    // Simple type
    FieldType {
        base_type: s.to_string(),
        pointer: false,
        generic_type: None,
        array_length: None,
        count: None,
    }
}
