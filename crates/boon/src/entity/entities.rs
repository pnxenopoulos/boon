use std::collections::HashSet;

use rustc_hash::FxHashMap;

use crate::error::{Error, Result};
use crate::io::BitReader;

use super::class_info::ClassInfo;
use super::field_decoder::FieldDecodeContext;
use super::field_path::{self, FieldPath};
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
    pub fields: FxHashMap<u64, FieldValue>,
}

impl Entity {
    fn new(index: i32, class_id: i32, class_name: String) -> Self {
        Self {
            index,
            serial: 0,
            class_id,
            class_name,
            fields: FxHashMap::default(),
        }
    }

    /// Apply field path deltas from a bit reader using the given serializer.
    #[allow(clippy::needless_range_loop)]
    fn apply_update(
        &mut self,
        br: &mut BitReader,
        serializer: &Serializer,
        ctx: &mut FieldDecodeContext,
        fp_buf: &mut Vec<FieldPath>,
    ) -> Result<()> {
        field_path::read_field_paths(br, fp_buf)?;

        for fp_idx in 0..fp_buf.len() {
            // Walk the serializer hierarchy to find the decoder (same as skip_update)
            let fp_last = fp_buf[fp_idx].last;
            let mut field = &serializer.fields[fp_buf[fp_idx].get(0)];

            for i in 1..=fp_last {
                let idx = fp_buf[fp_idx].get(i);
                if field.is_dynamic_array() {
                    if let Some(ref fs) = field.field_serializer {
                        field = &fs.fields[0];
                    }
                } else if let Some(ref fs) = field.field_serializer {
                    field = &fs.fields[idx];
                } else {
                    break;
                }
            }

            let key = fp_buf[fp_idx].pack();
            let value = field
                .metadata
                .decoder
                .decode(ctx, br)
                .map_err(|e| Error::Parse {
                    context: format!(
                        "field #{} key={:#x} (type: {}, decoder: {:?}, pos: {}, remaining: {}): {}",
                        fp_idx,
                        key,
                        field.var_type,
                        field.metadata.decoder,
                        br.position(),
                        br.bits_remaining(),
                        e
                    ),
                })?;
            self.fields.insert(key, value);
        }

        Ok(())
    }

    /// Skip field updates - reads the data to advance the bit reader but doesn't store anything.
    /// This avoids allocations and FxHashMap insertions for entities we don't care about.
    #[allow(clippy::needless_range_loop)]
    fn skip_update(
        br: &mut BitReader,
        serializer: &Serializer,
        ctx: &mut FieldDecodeContext,
        fp_buf: &mut Vec<FieldPath>,
    ) -> Result<()> {
        field_path::read_field_paths(br, fp_buf)?;

        for fp_idx in 0..fp_buf.len() {
            // Walk the serializer hierarchy to find the decoder
            let fp_last = fp_buf[fp_idx].last;
            let mut field = &serializer.fields[fp_buf[fp_idx].get(0)];

            for i in 1..=fp_last {
                let idx = fp_buf[fp_idx].get(i);
                if field.is_dynamic_array() {
                    if let Some(ref fs) = field.field_serializer {
                        field = &fs.fields[0];
                    }
                } else if let Some(ref fs) = field.field_serializer {
                    field = &fs.fields[idx];
                } else {
                    break;
                }
            }

            // Skip the value - just advances the bit reader without decoding
            field.metadata.decoder.skip(ctx, br)?;
        }

        Ok(())
    }

    /// Look up a field by its dotted name string using the serializer to resolve the key.
    pub fn get_by_name(&self, path: &str, serializer: &Serializer) -> Option<&FieldValue> {
        let key = serializer.resolve_field_key(path)?;
        self.fields.get(&key)
    }
}

/// Container managing all active entities.
#[derive(Default)]
pub struct EntityContainer {
    pub entities: FxHashMap<i32, Entity>,
    baseline_cache: FxHashMap<i32, Entity>,
    /// Tracks class_id for entities we're not fully tracking (for filtered parsing).
    /// This lets us skip updates properly by knowing which serializer to use.
    skipped_entity_classes: FxHashMap<i32, i32>,
}

impl EntityContainer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a CSVCMsg_PacketEntities message.
    pub fn handle_packet_entities(
        &mut self,
        msg: CsvcMsgPacketEntities,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
        fp_buf: &mut Vec<FieldPath>,
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
                        fp_buf,
                    )
                    .map_err(|e| Error::Parse {
                        context: format!("entity create #{}: {}", entity_index, e),
                    })?;
                }
                DELTA_UPDATE => {
                    self.handle_update(
                        entity_index,
                        &mut br,
                        class_info,
                        serializers,
                        field_decode_ctx,
                        fp_buf,
                    )
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

    /// Handle a CSVCMsg_PacketEntities message, only tracking specified entity classes.
    /// Entities not in the filter are parsed (to advance the bit reader) but not stored.
    #[allow(clippy::too_many_arguments)]
    pub fn handle_packet_entities_filtered(
        &mut self,
        msg: CsvcMsgPacketEntities,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
        class_filter: &HashSet<&str>,
        fp_buf: &mut Vec<FieldPath>,
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
                    self.handle_create_filtered(
                        entity_index,
                        &mut br,
                        class_info,
                        serializers,
                        string_tables,
                        field_decode_ctx,
                        class_filter,
                        fp_buf,
                    )?;
                }
                DELTA_UPDATE => {
                    self.handle_update_filtered(
                        entity_index,
                        &mut br,
                        class_info,
                        serializers,
                        field_decode_ctx,
                        class_filter,
                        fp_buf,
                    )?;
                }
                DELTA_DELETE | DELTA_LEAVE => {
                    self.entities.remove(&entity_index);
                    self.skipped_entity_classes.remove(&entity_index);
                }
                _ => {}
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_create(
        &mut self,
        index: i32,
        br: &mut BitReader,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
        fp_buf: &mut Vec<FieldPath>,
    ) -> Result<()> {
        let class_id = br.read_bits(class_info.bits)? as i32;
        let _serial = br.read_bits(NUM_SERIAL_NUM_BITS as usize)?;
        let _unknown = br.read_uvarint32()?;

        let class_entry = class_info.by_id(class_id).ok_or_else(|| Error::Parse {
            context: format!("unknown class_id {}", class_id),
        })?;

        let serializer =
            serializers
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
                e.apply_update(&mut baseline_br, &serializer, field_decode_ctx, fp_buf)
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
        entity
            .apply_update(br, &serializer, field_decode_ctx, fp_buf)
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
        fp_buf: &mut Vec<FieldPath>,
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

        entity.apply_update(br, &serializer, field_decode_ctx, fp_buf)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_create_filtered(
        &mut self,
        index: i32,
        br: &mut BitReader,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        string_tables: &StringTableContainer,
        field_decode_ctx: &mut FieldDecodeContext,
        class_filter: &HashSet<&str>,
        fp_buf: &mut Vec<FieldPath>,
    ) -> Result<()> {
        let class_id = br.read_bits(class_info.bits)? as i32;
        let _serial = br.read_bits(NUM_SERIAL_NUM_BITS as usize)?;
        let _unknown = br.read_uvarint32()?;

        let class_entry = class_info.by_id(class_id).ok_or_else(|| Error::Parse {
            context: format!("unknown class_id {}", class_id),
        })?;

        let serializer =
            serializers
                .get(&class_entry.network_name)
                .ok_or_else(|| Error::Parse {
                    context: format!("no serializer for {}", class_entry.network_name),
                })?;

        // Check if this class is in our filter
        if !class_filter.contains(class_entry.network_name.as_str()) {
            // Skip this entity - just advance the bit reader
            // But track its class_id so we can skip updates later
            self.skipped_entity_classes.insert(index, class_id);
            Entity::skip_update(br, &serializer, field_decode_ctx, fp_buf)?;
            return Ok(());
        }

        // Full processing for filtered entities
        let mut entity = if let Some(cached) = self.baseline_cache.get(&class_id) {
            let mut e = cached.clone();
            e.index = index;
            e
        } else {
            let mut e = Entity::new(index, class_id, class_entry.network_name.clone());

            if let Some(baseline_data) = string_tables.instance_baselines.get(&class_id) {
                let mut baseline_br = BitReader::new(baseline_data);
                e.apply_update(&mut baseline_br, &serializer, field_decode_ctx, fp_buf)?;
            }

            self.baseline_cache.insert(class_id, e.clone());
            e
        };

        entity.apply_update(br, &serializer, field_decode_ctx, fp_buf)?;
        self.entities.insert(index, entity);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_update_filtered(
        &mut self,
        index: i32,
        br: &mut BitReader,
        class_info: &ClassInfo,
        serializers: &SerializerContainer,
        field_decode_ctx: &mut FieldDecodeContext,
        _class_filter: &HashSet<&str>,
        fp_buf: &mut Vec<FieldPath>,
    ) -> Result<()> {
        // Check if we're tracking this entity
        if let Some(entity) = self.entities.get_mut(&index) {
            let serializer = serializers
                .get(&entity.class_name)
                .ok_or_else(|| Error::Parse {
                    context: format!("no serializer for {}", entity.class_name),
                })?;

            entity.apply_update(br, &serializer, field_decode_ctx, fp_buf)?;
            return Ok(());
        }

        // Entity is not tracked - check if we know its class from skipped creates
        if let Some(&class_id) = self.skipped_entity_classes.get(&index) {
            let class_entry = class_info.by_id(class_id).ok_or_else(|| Error::Parse {
                context: format!("unknown class_id {}", class_id),
            })?;

            let serializer =
                serializers
                    .get(&class_entry.network_name)
                    .ok_or_else(|| Error::Parse {
                        context: format!("no serializer for {}", class_entry.network_name),
                    })?;

            // Skip this update
            Entity::skip_update(br, &serializer, field_decode_ctx, fp_buf)?;
        }

        // If we don't know about this entity at all, it was created before filtering started
        // This shouldn't happen if we start filtering from the beginning
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
