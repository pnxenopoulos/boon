use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::io::BitReader;

use super::class_info::ClassInfo;
use super::field_decoder::FieldDecodeContext;
use super::field_path;
use super::field_value::FieldValue;
use super::serializers::{Serializer, SerializerContainer};
use super::string_tables::StringTableContainer;

use boon_proto::proto::CsvcMsgPacketEntities;

const MAX_EDICT_BITS: u32 = 14;
const NUM_ENT_ENTRY_BITS: u32 = MAX_EDICT_BITS + 1;
const NUM_SERIAL_NUM_BITS: u32 = 32 - NUM_ENT_ENTRY_BITS;

/// Delta header values indicating entity state changes.
const DELTA_UPDATE: u8 = 0b00;
const DELTA_CREATE: u8 = 0b10;
const DELTA_LEAVE: u8 = 0b01;
const DELTA_DELETE: u8 = 0b11;

/// A single entity with its class, fields, and current state.
#[derive(Debug, Clone)]
pub struct Entity {
    pub index: i32,
    pub serial: u32,
    pub class_id: i32,
    pub class_name: String,
    pub fields: HashMap<String, FieldValue>,
}

impl Entity {
    fn new(index: i32, class_id: i32, class_name: String) -> Self {
        Self {
            index,
            serial: 0,
            class_id,
            class_name,
            fields: HashMap::new(),
        }
    }

    /// Apply field path deltas from a bit reader using the given serializer.
    fn apply_update(
        &mut self,
        br: &mut BitReader,
        serializer: &Serializer,
        ctx: &mut FieldDecodeContext,
    ) -> Result<()> {
        let fps = field_path::read_field_paths(br)?;

        for (fp_idx, fp) in fps.iter().enumerate() {
            // Walk the serializer hierarchy to find the field and build the path name
            let mut path_parts: Vec<String> = Vec::new();
            let mut field = &serializer.fields[fp.get(0)];

            // Add send_node prefix if present
            if let Some(ref sn) = field.send_node {
                if !sn.is_empty() {
                    for part in sn.split('.') {
                        path_parts.push(part.to_string());
                    }
                }
            }
            path_parts.push(field.var_name.clone());

            for i in 1..=fp.last {
                let idx = fp.get(i);
                if field.is_dynamic_array() {
                    // Dynamic array: index into the single-element serializer
                    path_parts.push(idx.to_string());
                    if let Some(ref fs) = field.field_serializer {
                        field = &fs.fields[0];
                    }
                } else if let Some(ref fs) = field.field_serializer {
                    field = &fs.fields[idx];
                    if let Some(ref sn) = field.send_node {
                        if !sn.is_empty() {
                            for part in sn.split('.') {
                                path_parts.push(part.to_string());
                            }
                        }
                    }
                    path_parts.push(field.var_name.clone());
                } else {
                    break;
                }
            }

            let key = path_parts.join(".");
            let value = field.metadata.decoder.decode(ctx, br)
                .map_err(|e| Error::Parse {
                    context: format!(
                        "field #{} '{}' (type: {}, decoder: {:?}, pos: {}, remaining: {}): {}",
                        fp_idx, key, field.var_type, field.metadata.decoder,
                        br.position(), br.bits_remaining(), e
                    ),
                })?;
            self.fields.insert(key, value);
        }

        Ok(())
    }
}

/// Container managing all active entities.
pub struct EntityContainer {
    pub entities: HashMap<i32, Entity>,
    baseline_cache: HashMap<i32, Entity>,
}

impl EntityContainer {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            baseline_cache: HashMap::new(),
        }
    }

    /// Handle a CSVCMsg_PacketEntities message.
    pub fn handle_packet_entities(
        &mut self,
        msg: CsvcMsgPacketEntities,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
    ) -> Result<()> {
        let entity_data = msg.entity_data.unwrap_or_default();
        let mut br = BitReader::new(&entity_data);

        let mut entity_index: i32 = -1;

        for _ in 0..msg.updated_entries.unwrap_or(0) {
            entity_index += br.read_ubitvar()? as i32 + 1;

            // Read delta header (2 bits)
            let dh = br.read_bits(2)? as u8;

            match dh {
                DELTA_CREATE => {
                    self.handle_create(
                        entity_index,
                        &mut br,
                        class_info,
                        serializers,
                        string_tables,
                        field_decode_ctx,
                    )
                    .map_err(|e| Error::Parse {
                        context: format!(
                            "entity create #{}: {}",
                            entity_index, e
                        ),
                    })?;
                }
                DELTA_UPDATE => {
                    self.handle_update(entity_index, &mut br, class_info, serializers, field_decode_ctx)
                    .map_err(|e| Error::Parse {
                        context: format!(
                            "entity update #{} (class: {:?}): {}",
                            entity_index,
                            self.entities.get(&entity_index).map(|e| &e.class_name),
                            e
                        ),
                    })?;
                }
                DELTA_DELETE | DELTA_LEAVE => {
                    self.entities.remove(&entity_index);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_create(
        &mut self,
        index: i32,
        br: &mut BitReader,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
    ) -> Result<()> {
        let class_id = br.read_bits(class_info.bits)? as i32;
        let _serial = br.read_bits(NUM_SERIAL_NUM_BITS as usize)?;
        let _unknown = br.read_uvarint32()?;

        let class_entry = class_info.by_id(class_id).ok_or_else(|| Error::Parse {
            context: format!("unknown class_id {}", class_id),
        })?;

        let serializer = serializers
            .get(&class_entry.network_name)
            .ok_or_else(|| Error::Parse {
                context: format!("no serializer for {}", class_entry.network_name),
            })?;

        // Get or create baseline entity
        let mut entity = if let Some(cached) = self.baseline_cache.get(&class_id) {
            let mut e = cached.clone();
            e.index = index;
            e
        } else {
            let mut e = Entity::new(index, class_id, class_entry.network_name.clone());

            // Apply baseline from instancebaseline string table
            if let Some(baseline_data) = string_tables.instance_baselines.get(&class_id) {
                let mut baseline_br = BitReader::new(baseline_data);
                e.apply_update(&mut baseline_br, &serializer, field_decode_ctx)
                    .map_err(|err| Error::Parse {
                        context: format!(
                            "baseline for {} (class_id {}): {}",
                            class_entry.network_name, class_id, err
                        ),
                    })?;
            }

            self.baseline_cache.insert(class_id, e.clone());
            e
        };

        // Apply create delta
        entity.apply_update(br, &serializer, field_decode_ctx)
            .map_err(|err| Error::Parse {
                context: format!(
                    "create delta for {} (class_id {}): {}",
                    class_entry.network_name, class_id, err
                ),
            })?;
        self.entities.insert(index, entity);

        Ok(())
    }

    fn handle_update(
        &mut self,
        index: i32,
        br: &mut BitReader,
        _class_info: &ClassInfo,
        serializers: &SerializerContainer,
        field_decode_ctx: &mut FieldDecodeContext,
    ) -> Result<()> {
        let entity = match self.entities.get_mut(&index) {
            Some(e) => e,
            None => {
                return Err(Error::Parse {
                    context: format!("tried to update non-existent entity #{}", index),
                });
            }
        };

        let serializer = serializers
            .get(&entity.class_name)
            .ok_or_else(|| Error::Parse {
                context: format!("no serializer for {}", entity.class_name),
            })?;

        entity.apply_update(br, &serializer, field_decode_ctx)?;
        Ok(())
    }

    pub fn get(&self, index: i32) -> Option<&Entity> {
        self.entities.get(&index)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&i32, &Entity)> {
        self.entities.iter()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}
