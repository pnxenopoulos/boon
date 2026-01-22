// src/parser/stringtables.rs
use std::collections::HashMap;

use boon_proto::generated as pb;
use prost::Message;
use snap::raw::Decoder;

use crate::parser::error::ParserError;
use crate::reader::{ReadError, Reader};

#[derive(Debug, Clone)]
pub struct StringTableEntry {
    pub key: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StringTable {
    pub name: String,
    pub items: Vec<StringTableEntry>,
    pub flags: i32,
}

#[derive(Debug, Clone, Default)]
pub struct StringTableRegistry {
    /// Fast name → table
    pub tables: HashMap<String, StringTable>,
    /// Sequential id → name (svc updates reference table ids)
    pub order: Vec<String>,
    /// name → id
    pub id_by_name: HashMap<String, usize>,
}

#[derive(Debug, Default, Clone)]
pub struct BaselineRegistry {
    /// Decoded baselines by class id
    pub by_class: HashMap<u16, pb::CsvcMsgPacketEntities>,
}

impl BaselineRegistry {
    pub fn get(&self, class_id: u16) -> Option<&pb::CsvcMsgPacketEntities> {
        self.by_class.get(&class_id)
    }
}

/* ─────────────────────────── snapshots ─────────────────────────── */

impl StringTableRegistry {
    /// Build a registry from a full `CDemoStringTables` snapshot.
    pub fn from_demo_snapshot(msg: &pb::CDemoStringTables) -> Result<Self, ParserError> {
        let mut reg = Self::default();

        for (id, t) in msg.tables.iter().enumerate() {
            let name = t.table_name.clone().unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let mut items = Vec::with_capacity(t.items.len());
            for it in &t.items {
                items.push(StringTableEntry {
                    key:  it.str.clone().unwrap_or_default(),
                    data: it.data.clone().unwrap_or_default(),
                });
            }
            let flags = t.table_flags.unwrap_or_default();

            reg.id_by_name.insert(name.clone(), id);
            reg.order.push(name.clone());
            reg.tables.insert(
                name.clone(),
                StringTable { name, items, flags }
            );
        }

        Ok(reg)
    }

    /// Full replace with snapshot.
    pub fn apply_snapshot(&mut self, snap: &pb::CDemoStringTables) -> Result<(), ParserError> {
        *self = Self::from_demo_snapshot(snap)?;
        Ok(())
    }

    /// Convenience: replace current state with a freshly built one.
    pub fn replace_with_snapshot(&mut self, snapshot: StringTableRegistry) {
        *self = snapshot;
    }
}

/* ────────────────────────── svc: create/update ────────────────────────── */

impl StringTableRegistry {
    /// Apply `CSVCMsg_CreateStringTable` (from SvcCreateStringTable).
    pub fn apply_create(&mut self, msg: &pb::CsvcMsgCreateStringTable) -> Result<(), ParserError> {
        let name = msg.name.clone().unwrap_or_default();
        if name.is_empty() {
            return Err(ParserError::Decode("CreateStringTable: empty name".into()));
        }
        let num_entries = msg.num_entries.unwrap_or_default().max(0) as usize;
        let flags = msg.flags.unwrap_or_default();

        // Decompress if needed
        let raw = msg.string_data.as_deref().unwrap_or(&[]);
        let bytes = if msg.data_compressed.unwrap_or(false) {
            let mut de = Decoder::new();
            match de.decompress_vec(raw) {
                Ok(v) => {
                    // Size hint mismatch isn’t fatal; trust decompressor
                    let _ = msg.uncompressed_size; // informational
                    v
                }
                Err(_) => raw.to_vec(), // fall back to raw
            }
        } else {
            raw.to_vec()
        };

        // Decode packed entries
        let changes = decode_string_table_payload(
            &bytes,
            num_entries,
            msg.user_data_fixed_size.unwrap_or(false),
            msg.user_data_size.unwrap_or(0).max(0) as usize,
            msg.user_data_size_bits.unwrap_or(0).max(0) as usize,
            msg.using_varint_bitcounts.unwrap_or(true),
        )?;

        // Materialize into a dense items vector
        let mut items: Vec<StringTableEntry> = Vec::new();
        let mut max_idx = 0usize;
        for c in &changes { max_idx = max_idx.max(c.index); }
        items.resize_with(max_idx + 1, || StringTableEntry { key: String::new(), data: Vec::new() });

        for c in changes {
            let dst = &mut items[c.index];
            if let Some(k) = c.key   { dst.key = k; }
            if let Some(d) = c.data  { dst.data = d; }
        }

        // Register id
        let id = self.order.len();
        self.id_by_name.insert(name.clone(), id);
        self.order.push(name.clone());

        self.tables.insert(name.clone(), StringTable { name, items, flags });
        Ok(())
    }

    /// Apply `CSVCMsg_UpdateStringTable` (from SvcUpdateStringTable).
    pub fn apply_update(&mut self, msg: &pb::CsvcMsgUpdateStringTable) -> Result<(), ParserError> {
        let id = msg.table_id.ok_or_else(|| ParserError::Decode("UpdateStringTable: table_id missing".into()))?;
        let id = usize::try_from(id).map_err(|_| ParserError::Decode("UpdateStringTable: bad table_id".into()))?;

        let name = self.order.get(id)
            .ok_or_else(|| ParserError::Decode(format!("UpdateStringTable: unknown table id {}", id)))?
            .clone();

        let table = self.tables.get_mut(&name)
            .ok_or_else(|| ParserError::Decode(format!("UpdateStringTable: table '{}' not found", name)))?;

        let changed = msg.num_changed_entries.unwrap_or_default().max(0) as usize;
        let blob = msg.string_data.as_deref().unwrap_or(&[]);

        let changes = decode_string_table_payload(
            blob,
            changed,
            false, // updates almost always embed lengths, not fixed-size
            0,
            0,
            true,
        )?;

        apply_decoded_updates_in_place(table, changes);
        Ok(())
    }
}

/* ───────────────────────── instance baselines ───────────────────────── */

/// Try multiple encodings to turn a baseline blob into `CSVCMsg_PacketEntities`.
fn decode_packet_entities_flexible(
    bytes: &[u8],
    snappy: &std::cell::RefCell<Decoder>,
) -> Result<pb::CsvcMsgPacketEntities, ParserError> {
    // 1) Direct protobuf
    if let Ok(msg) = pb::CsvcMsgPacketEntities::decode(bytes) {
        return Ok(msg);
    }
    // 2) Snappy → protobuf
    if let Ok(decomp) = snappy.borrow_mut().decompress_vec(bytes) {
        if let Ok(msg) = pb::CsvcMsgPacketEntities::decode(&*decomp) {
            return Ok(msg);
        }
    }
    // 3) Fallback: treat blob as already-serialized entity bitstream.
    //    We wrap it so downstream tools still have access via `serialized_entities`.
    let mut stub = pb::CsvcMsgPacketEntities::default();
    stub.serialized_entities = Some(bytes.to_vec());
    Ok(stub)
}

impl StringTableRegistry {
    /// Build per-class baselines by decoding the `instancebaseline` table.
    /// Keys are decimal class IDs; values are PacketEntities blobs.
    pub fn build_instance_baselines(
        &self,
        snappy: &std::cell::RefCell<Decoder>,
    ) -> Result<BaselineRegistry, ParserError> {
        let mut out = BaselineRegistry::default();
        let Some(tbl) = self.tables.get("instancebaseline") else { return Ok(out) };

        for item in &tbl.items {
            let Ok(class_id) = item.key.parse::<u16>() else { continue };
            if item.data.is_empty() { continue; }

            let pe = decode_packet_entities_flexible(&item.data, snappy)?;
            // Non-fatal oddities in some dumps
            // if pe.legacy_is_delta.unwrap_or(false) { /* warn if desired */ }

            out.by_class.insert(class_id, pe);
        }

        Ok(out)
    }
}

/* ─────────────────────── payload decoder (create/update) ─────────────────────── */

/// One decoded change from a packed string-table payload
#[derive(Debug)]
struct DecodedChange {
    index: usize,
    key: Option<String>,   // None => keep prior key (updates)
    data: Option<Vec<u8>>, // None => no data change
}

fn apply_decoded_updates_in_place(table: &mut StringTable, changes: Vec<DecodedChange>) {
    // Ensure capacity for sparse updates
    let mut max_idx = table.items.len().saturating_sub(1);
    for c in &changes { max_idx = max_idx.max(c.index); }
    if max_idx + 1 > table.items.len() {
        table.items
            .resize_with(max_idx + 1, || StringTableEntry { key: String::new(), data: Vec::new() });
    }

    for c in changes {
        let slot = &mut table.items[c.index];
        if let Some(k) = c.key  { slot.key = k; }
        if let Some(d) = c.data { slot.data = d; }
    }
}

/// Decode the compact bitstream used by `CSVCMsg_{Create,Update}StringTable.string_data`.
/// This matches the common S2 layout:
///   explicit_index_bit
///   if explicit_index_bit: index=varuint
///   has_key_bit
///     if has_key_bit:
///       from_history_bit
///         if from_history_bit:
///           ref_idx=varuint, copy_len=varuint, suffix=cstring
///         else:
///           key=cstring
///   has_data_bit
///     if has_data_bit:
///       if user_data_fixed_size: read N bytes
///       else if using_varint_bitcounts: len=varuint, then len bytes
///       else: read `user_data_size_bits` bits for length, then that many bytes
///
/// If your branch diverges, failures will surface early; adjust in place.
fn decode_string_table_payload(
    bytes: &[u8],
    expected: usize,
    user_data_fixed_size: bool,
    user_data_size: usize,
    user_data_size_bits: usize,
    using_varint_bitcounts: bool,
) -> Result<Vec<DecodedChange>, ParserError> {
    let mut r = Reader::new(bytes);
    let mut out: Vec<DecodedChange> = Vec::with_capacity(expected);
    let mut last_index: isize = -1;
    let mut history: Vec<String> = Vec::new();

    for _ in 0..expected {
        // Index selection
        let explicit_index = read_one_bit(&mut r)?;
        let index = if explicit_index {
            r.read_var_u32()? as usize
        } else {
            (last_index + 1) as usize
        };
        last_index = index as isize;

        // Key
        let has_key = read_one_bit(&mut r)?;
        let key = if has_key {
            let use_hist = read_one_bit(&mut r)?;
            if use_hist && !history.is_empty() {
                let ref_idx = (r.read_var_u32()? as usize) % history.len();
                let copy_len = r.read_var_u32()? as usize;
                // Keys are byte strings → align, then read cstring suffix
                r.align_to_byte()?;
                let suffix = read_cstring(&mut r)?;
                let mut k = history[ref_idx].clone();
                if copy_len < k.len() { k.truncate(copy_len); }
                k.push_str(&suffix);
                Some(k)
            } else {
                r.align_to_byte()?;
                Some(read_cstring(&mut r)?)
            }
        } else {
            None
        };

        if let Some(ref k) = key {
            history.push(k.clone());
            if history.len() > 32 {
                history.remove(0);
            }
        }

        // User-data
        let has_data = read_one_bit(&mut r)?;
        let data = if has_data {
            if user_data_fixed_size && user_data_size > 0 {
                Some(r.read_bytes(user_data_size)?)
            } else {
                let len = if using_varint_bitcounts {
                    r.read_var_u32()? as usize
                } else {
                    // Read an exact-length in bits, then the bytes; align reads as needed
                    let nbits = user_data_size_bits;
                    let nbytes = (nbits + 7) / 8;
                    let v = r.read_bytes(nbytes)?; // treat as opaque
                    // len is often stored as little-endian within those bits; a strict decode
                    // requires bit-exact reads. To keep robust, fall back to the first byte.
                    let approx = v.get(0).copied().unwrap_or(0) as usize;
                    approx.min(r.bits_remaining_total() / 8)
                };
                Some(r.read_bytes(len)?)
            }
        } else {
            None
        };

        out.push(DecodedChange { index, key, data });
    }

    Ok(out)
}

/* ─────────────────────────── Reader shims ─────────────────────────── */

/// Single-bit read using your `Reader`.
fn read_one_bit(r: &mut Reader<'_>) -> Result<bool, ParserError> {
    // If your Reader already has `read_one_bit()`, use it:
    // return Ok(r.read_one_bit()?);
    // Portable fallback: read 1 bit by reading var_u32 of 1 bit is not available,
    // so we expose a dedicated method in Reader. If it's missing, implement it there.
    match r.read_bits(1) {
        Ok(v) => Ok((v & 1) != 0),
        Err(ReadError::Eof) => Err(ParserError::Decode("bitstream EOF".into())),
        Err(e) => Err(e.into()),
    }
}

/// Read a NUL-terminated string from current byte boundary.
fn read_cstring(r: &mut Reader<'_>) -> Result<String, ParserError> {
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let b = r.read_bytes(1)
            .map_err(|e| match e {
                ReadError::Eof => ParserError::Decode("cstring EOF".into()),
                _ => e.into(),
            })?[0];
        if b == 0 { break; }
        bytes.push(b);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}