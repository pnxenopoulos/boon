use std::collections::HashMap;
use std::rc::Rc;

use prost::Message;

use crate::error::Result;
use crate::io::ByteReader;

use super::field_decoder::{self, FieldMetadata};
use super::field_path::FieldPath;

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

impl Serializer {
    /// Resolve a dotted field name (e.g. "m_pGameRules.m_bGamePaused") to a packed u64 key.
    /// Walks the serializer hierarchy matching send_node + var_name against path components.
    pub fn resolve_field_key(&self, path: &str) -> Option<u64> {
        let parts: Vec<&str> = path.split('.').collect();
        self.resolve_parts(&parts, 0)
    }

    fn resolve_parts(&self, parts: &[&str], depth: usize) -> Option<u64> {
        if depth >= parts.len() {
            return None;
        }

        for (field_idx, field) in self.fields.iter().enumerate() {
            // Build the name parts this field contributes (send_node + var_name)
            let mut field_parts: Vec<&str> = Vec::new();
            if let Some(ref sn) = field.send_node
                && !sn.is_empty()
            {
                for part in sn.split('.') {
                    field_parts.push(part);
                }
            }
            if !field.var_name.is_empty() {
                field_parts.push(&field.var_name);
            }

            // Check if the remaining path starts with these field parts
            let remaining = &parts[depth..];
            if remaining.len() < field_parts.len() {
                continue;
            }
            if field_parts != remaining[..field_parts.len()] {
                continue;
            }

            let consumed = depth + field_parts.len();

            // If we've consumed all parts, this is the field
            if consumed == parts.len() {
                let mut fp = FieldPath::default();
                fp.data[0] = field_idx as u8;
                // last stays 0
                return Some(fp.pack());
            }

            // More parts remain — we need to recurse into a sub-serializer
            let next_part = parts[consumed];

            // Dynamic array: next part is a numeric index
            if field.is_dynamic_array() {
                if let Ok(array_idx) = next_part.parse::<usize>()
                    && let Some(ref fs) = field.field_serializer
                {
                    let inner_field = &fs.fields[0];
                    let after_idx = consumed + 1;

                    if after_idx == parts.len() {
                        // The array element itself is the value
                        let mut fp = FieldPath::default();
                        fp.data[0] = field_idx as u8;
                        fp.data[1] = array_idx as u8;
                        fp.last = 1;
                        return Some(fp.pack());
                    }

                    // Recurse into the inner field's serializer
                    if let Some(ref inner_fs) = inner_field.field_serializer
                        && let Some(key) = inner_fs.resolve_parts(parts, after_idx)
                    {
                        let inner_fp = FieldPath::unpack(key);
                        let mut fp = FieldPath::default();
                        fp.data[0] = field_idx as u8;
                        fp.data[1] = array_idx as u8;
                        fp.last = 2 + inner_fp.last;
                        for i in 0..=inner_fp.last {
                            fp.data[2 + i] = inner_fp.data[i];
                        }
                        return Some(fp.pack());
                    }
                }
                continue;
            }

            // Non-dynamic: recurse into field_serializer
            if let Some(ref fs) = field.field_serializer
                && let Some(key) = fs.resolve_parts(parts, consumed)
            {
                let inner_fp = FieldPath::unpack(key);
                let mut fp = FieldPath::default();
                fp.data[0] = field_idx as u8;
                fp.last = 1 + inner_fp.last;
                for i in 0..=inner_fp.last {
                    fp.data[1 + i] = inner_fp.data[i];
                }
                return Some(fp.pack());
            }
        }

        None
    }

    /// Convert a packed u64 key back to a dotted field name string.
    /// Walks the serializer hierarchy using the unpacked FieldPath.
    pub fn field_name_for_key(&self, key: u64) -> Option<String> {
        let fp = FieldPath::unpack(key);
        let mut parts: Vec<String> = Vec::new();
        let mut field = self.fields.get(fp.get(0))?;

        if let Some(ref sn) = field.send_node
            && !sn.is_empty()
        {
            for part in sn.split('.') {
                parts.push(part.to_string());
            }
        }
        parts.push(field.var_name.clone());

        for i in 1..=fp.last {
            let idx = fp.get(i);
            if field.is_dynamic_array() {
                parts.push(idx.to_string());
                if let Some(ref fs) = field.field_serializer {
                    field = &fs.fields[0];
                } else {
                    break;
                }
            } else if let Some(ref fs) = field.field_serializer {
                field = fs.fields.get(idx)?;
                if let Some(ref sn) = field.send_node
                    && !sn.is_empty()
                {
                    for part in sn.split('.') {
                        parts.push(part.to_string());
                    }
                }
                parts.push(field.var_name.clone());
            } else {
                break;
            }
        }

        Some(parts.join("."))
    }
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
    if let Some(stripped) = s.strip_suffix('*') {
        return FieldType {
            base_type: stripped.trim().to_string(),
            pointer: true,
            generic_type: None,
            array_length: None,
            count: None,
        };
    }

    // Check for array: type[length]
    if let Some(bracket_pos) = s.find('[')
        && s.ends_with(']')
    {
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

    // Check for generic: Type< InnerType >
    if let Some(angle_pos) = s.find('<')
        && let Some(close_pos) = s.rfind('>')
    {
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

    // Simple type
    FieldType {
        base_type: s.to_string(),
        pointer: false,
        generic_type: None,
        array_length: None,
        count: None,
    }
}
