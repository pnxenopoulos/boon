use std::collections::HashMap;

use boon_proto::generated as pb;
use prost::Message;

use crate::parser::error::ParserError;
use crate::reader::{ReadError, Reader};

fn hexdump_prefix(bytes: &[u8], max: usize) -> String {
    let mut s = String::new();
    for (i, b) in bytes.iter().take(max).enumerate() {
        if i > 0 {
            if i % 16 == 0 {
                s.push('\n');
            } else if i % 8 == 0 {
                s.push_str("  ");
            } else {
                s.push(' ');
            }
        }
        use std::fmt::Write as _;
        let _ = write!(s, "{:02X}", b);
    }
    s
}

/// Human-friendly report of what's inside CDemoSendTables.data
pub fn sniff_sendtables(sendtables: &[u8]) -> String {
    use std::collections::{BTreeMap, HashSet};

    let mut out = String::new();
    use std::fmt::Write as _;
    let _ = writeln!(out, "SendTables bytes: {} bytes", sendtables.len());
    let _ = writeln!(
        out,
        "First 64 bytes (hex):\n{}",
        hexdump_prefix(sendtables, 64)
    );

    // Try direct flattened decode first
    match pb::CsvcMsgFlattenedSerializer::decode(sendtables) {
        Ok(fs) => {
            let _ = writeln!(out, "Direct decode: CsvcMsgFlattenedSerializer OK");
            let _ = writeln!(
                out,
                "  symbols={}, serializers={}, fields={}",
                fs.symbols.len(),
                fs.serializers.len(),
                fs.fields.len()
            );
            return out; // no need to scan frames if this succeeded
        }
        Err(e) => {
            let _ = writeln!(
                out,
                "Direct decode as CsvcMsgFlattenedSerializer failed: {}",
                e
            );
        }
    }

    // Otherwise, attempt framed scan
    let mut r = Reader::new(sendtables);
    let mut frames = 0usize;
    let mut success_flat = 0usize;
    let mut success_legacy = 0usize;

    let mut size_hist = BTreeMap::<usize, usize>::new();
    let mut type_raws = HashSet::<i32>::new();

    let mut per_frame_preview: Vec<String> = Vec::new();

    // Keep a local cursor notion: Reader doesn't expose absolute byte offset, so derive from remaining.
    let total_bits = r.bits_remaining_total();

    while r.bits_remaining_total() >= 6 {
        let before_bits = r.bits_remaining_total();
        let ty = match r.read_ubit_var() {
            Ok(v) => v as i32,
            Err(_) => break,
        };
        if r.bits_remaining_total() < 8 {
            break;
        }
        let sz = match r.read_var_u32() {
            Ok(v) => v as usize,
            Err(_) => break,
        };
        let payload = match r.read_bytes(sz) {
            Ok(b) => b,
            Err(_) => break,
        };

        frames += 1;
        *size_hist.entry(sz).or_insert(0) += 1;
        type_raws.insert(ty);

        let after_bits = r.bits_remaining_total();
        let consumed_bits = before_bits.saturating_sub(after_bits);
        let approx_offset = (total_bits.saturating_sub(before_bits)) / 8;

        // Try both decodes (content-based)
        let mut tags = Vec::new();
        if let Ok(fs) = pb::CsvcMsgFlattenedSerializer::decode(&*payload) {
            success_flat += 1;
            tags.push(format!(
                "FlattenedSerializer(sym={}, ser={}, fields={})",
                fs.symbols.len(),
                fs.serializers.len(),
                fs.fields.len()
            ));
        }
        if let Ok(st) = pb::CsvcMsgSendTable::decode(&*payload) {
            success_legacy += 1;
            let name = st.net_table_name.as_deref().unwrap_or("");
            let props = st.props.len();
            let is_end = st.is_end.unwrap_or(false);
            tags.push(format!(
                "SendTable(name='{}', props={}, end={})",
                name, props, is_end
            ));
        }

        // Build a short preview line
        let ascii_hint = String::from_utf8(
            payload
                .iter()
                .take(8)
                .map(|&b| if b.is_ascii_graphic() { b } else { b'.' })
                .collect(),
        )
        .unwrap_or_default();

        per_frame_preview.push(format!(
            "#{:03} @{} ty={} size={} consumed_bits={}  {}  preview:[{}]",
            frames,
            approx_offset,
            ty,
            sz,
            consumed_bits,
            if tags.is_empty() {
                "-"
            } else {
                &tags.join(" | ")
            },
            ascii_hint
        ));

        // Limit preview to avoid huge dumps
        if frames >= 200 {
            break;
        }
    }

    let _ = writeln!(
        out,
        "Framed scan: frames={}  flat_ok={}  legacy_ok={}",
        frames, success_flat, success_legacy
    );
    let _ = writeln!(out, "Distinct raw types: {:?}", type_raws);

    if !size_hist.is_empty() {
        let _ = writeln!(out, "Size histogram (top 10):");
        for (k, v) in size_hist.iter().take(10) {
            let _ = writeln!(out, "  size {:6} -> {:6}", k, v);
        }
    }

    if !per_frame_preview.is_empty() {
        let _ = writeln!(out, "First {} frames:", per_frame_preview.len().min(25));
        for line in per_frame_preview.iter().take(25) {
            let _ = writeln!(out, "{}", line);
        }
        if per_frame_preview.len() > 25 {
            let _ = writeln!(out, "... ({} more)", per_frame_preview.len() - 25);
        }
    }

    out
}

pub fn sniff_sendtables_enhanced(sendtables: &[u8]) -> String {
    use std::collections::{BTreeMap, HashSet};
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "SendTables bytes: {}", sendtables.len());
    let _ = writeln!(
        out,
        "First 64 bytes (hex):\n{}",
        hexdump_prefix(sendtables, 64)
    );

    match pb::CsvcMsgFlattenedSerializer::decode(sendtables) {
        Ok(fs) => {
            let _ = writeln!(
                out,
                "Direct decode: CsvcMsgFlattenedSerializer OK (symbols={}, serializers={}, fields={})",
                fs.symbols.len(),
                fs.serializers.len(),
                fs.fields.len()
            );
            return out;
        }
        Err(e) => {
            let _ = writeln!(out, "Direct decode failed: {}", e);
        }
    }

    // Try SVC framed
    {
        let mut r = Reader::new(sendtables);
        let mut frames = 0usize;
        let mut flat_ok = 0usize;
        let mut legacy_ok = 0usize;
        let mut types = HashSet::<i32>::new();
        let mut size_hist = BTreeMap::<usize, usize>::new();

        while r.bits_remaining_total() >= 6 {
            let ty = match r.read_ubit_var() {
                Ok(v) => v as i32,
                _ => break,
            };
            if r.bits_remaining_total() < 8 {
                break;
            }
            let sz = match r.read_var_u32() {
                Ok(v) => v as usize,
                _ => break,
            };
            let payload = match r.read_bytes(sz) {
                Ok(b) => b,
                _ => break,
            };

            frames += 1;
            types.insert(ty);
            *size_hist.entry(sz).or_insert(0) += 1;

            if pb::CsvcMsgFlattenedSerializer::decode(&*payload).is_ok() {
                flat_ok += 1;
            }
            if pb::CsvcMsgSendTable::decode(&*payload).is_ok() {
                legacy_ok += 1;
            }
        }

        let _ = writeln!(
            out,
            "SVC-framed scan: frames={} flat_ok={} legacy_ok={} types={:?}",
            frames, flat_ok, legacy_ok, types
        );
        if frames > 0 {
            let _ = writeln!(out, "SVC size histogram (top 10):");
            for (k, v) in size_hist.iter().take(10) {
                let _ = writeln!(out, "  size {:6} -> {:6}", k, v);
            }
        }
    }

    // Try protobuf length-delimited stream
    {
        let chunks = split_length_delimited_frames(sendtables);
        let _ = writeln!(out, "LD stream: chunks={}", chunks.len());
        let mut flat_ok = 0usize;
        let mut legacy_ok = 0usize;
        let mut first_tags = Vec::new();
        for (i, ch) in chunks.iter().take(20).enumerate() {
            let mut tags = Vec::new();
            if let Ok(fs) = pb::CsvcMsgFlattenedSerializer::decode(*ch) {
                flat_ok += 1;
                tags.push(format!(
                    "Flattened(sym={},ser={},fields={})",
                    fs.symbols.len(),
                    fs.serializers.len(),
                    fs.fields.len()
                ));
            }
            if let Ok(st) = pb::CsvcMsgSendTable::decode(*ch) {
                legacy_ok += 1;
                let name = st.net_table_name.as_deref().unwrap_or("");
                tags.push(format!(
                    "SendTable('{}', props={}, end={})",
                    name,
                    st.props.len(),
                    st.is_end.unwrap_or(false)
                ));
            }
            let tag = if tags.is_empty() {
                "-".to_string()
            } else {
                tags.join(" | ")
            };
            first_tags.push(format!("#{:03} {}", i + 1, tag));
        }
        let _ = writeln!(out, "LD: flat_ok={} legacy_ok={}", flat_ok, legacy_ok);
        for line in first_tags {
            let _ = writeln!(out, "{}", line);
        }
    }

    out
}

// Decode a protobuf varint (up to 32-bit length) and return (len, bytes_consumed).
fn read_pb_varint_len(buf: &[u8]) -> Option<(usize, usize)> {
    let mut val: u64 = 0;
    let mut shift = 0;
    for (i, &b) in buf.iter().enumerate() {
        let part = (b & 0x7F) as u64;
        val |= part << shift;
        if (b & 0x80) == 0 {
            // Success
            return usize::try_from(val).ok().map(|len| (len, i + 1));
        }
        shift += 7;
        if shift > 35 {
            // sanity for 32-bit lengths
            return None;
        }
    }
    None
}

// Split a buffer into protobuf length-delimited frames: [len-varint][len bytes]...
fn split_length_delimited_frames(mut buf: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    while !buf.is_empty() {
        // parse length
        let (len, used) = match read_pb_varint_len(buf) {
            Some(x) => x,
            None => break, // stop on malformed tail
        };
        buf = &buf[used..];
        if buf.len() < len {
            break;
        } // incomplete last frame: stop
        frames.push(&buf[..len]);
        buf = &buf[len..];
    }
    frames
}

/// One flattened field in on-wire order.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,                    // symbols[var_name_sym] or legacy var_name
    pub var_type: String,                // symbols[var_type_sym] or legacy type string
    pub encoder: String,                 // best-effort encoder name / serializer name
    pub bit_count: i32,                  // optional width
    pub low: Option<f32>,                // optional low clamp
    pub high: Option<f32>,               // optional high clamp
    pub flags: u32,                      // field flags when present
    pub array_len: i32,                  // legacy arrays; 0 when scalar
    pub priority: i32,                   // legacy priority; 0 otherwise
    pub polymorphic: Vec<(String, i32)>, // (serializer_name, version) for poly fields
}

/// Per-class serializer / table.
#[derive(Debug, Clone)]
pub struct Serializer {
    pub name: String,          // class/table name
    pub version: i32,          // 0 for legacy sendtables
    pub class_id: Option<u16>, // from CDemoClassInfo.{table_name|network_name}
    pub fields: Vec<Field>,    // on-wire order for decode
}

/// Lookups for later decode passes.
#[derive(Debug, Default)]
pub struct SerializerRegistry {
    pub by_class: HashMap<u16, Serializer>,
    pub by_name: HashMap<String, Serializer>,
}

impl SerializerRegistry {
    pub fn get_by_class(&self, id: u16) -> Option<&Serializer> {
        self.by_class.get(&id)
    }
    pub fn get_by_name(&self, name: &str) -> Option<&Serializer> {
        self.by_name.get(name)
    }
}

/// Parse SendTables from the `CDemoSendTables.data` bytes.
/// Supports:
///  - Direct flattened blob (CsvcMsgFlattenedSerializer)
///  - Framed flattened (scan [UBitVar type][varU32 size][payload], decode as flattened)
///  - Framed legacy sendtables (scan, collect CsvcMsgSendTable until is_end)
pub fn parse_sendtables(
    sendtables: &[u8],
    class_info: &pb::CDemoClassInfo,
) -> Result<SerializerRegistry, ParserError> {
    // PATH A — direct flattened blob
    if let Ok(fs) = pb::CsvcMsgFlattenedSerializer::decode(sendtables) {
        return build_registry_from_flattened(fs, class_info);
    }

    // PATH B — SVC framed mini-stream
    let mut r = Reader::new(sendtables);
    let mut flattened: Option<pb::CsvcMsgFlattenedSerializer> = None;
    let mut legacy_tables: Vec<pb::CsvcMsgSendTable> = Vec::new();
    let mut saw_legacy_end = false;

    loop {
        if r.bits_remaining_total() < 6 {
            break;
        }
        let _ty = match r.read_ubit_var() {
            Ok(t) => t as i32,
            Err(ReadError::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        if r.bits_remaining_total() < 8 {
            break;
        }
        let size = match r.read_var_u32() {
            Ok(sz) => sz as usize,
            Err(_) => break,
        };
        let payload = match r.read_bytes(size) {
            Ok(b) => b,
            Err(ReadError::Eof) => break,
            Err(e) => return Err(e.into()),
        };

        if flattened.is_none()
            && let Ok(fs) = pb::CsvcMsgFlattenedSerializer::decode(&*payload)
        {
            flattened = Some(fs);
            continue;
        }

        if let Ok(st) = pb::CsvcMsgSendTable::decode(&*payload) {
            if st.is_end.unwrap_or(false) {
                saw_legacy_end = true;
            } else {
                legacy_tables.push(st);
            }
            continue;
        }
    }

    if let Some(fs) = flattened {
        return build_registry_from_flattened(fs, class_info);
    }
    if saw_legacy_end && !legacy_tables.is_empty() {
        return build_registry_from_legacy(legacy_tables, class_info);
    }

    // PATH C — protobuf length-delimited stream (no SVC headers)
    let mut ld_flattened: Option<pb::CsvcMsgFlattenedSerializer> = None;
    let mut ld_legacy_tables: Vec<pb::CsvcMsgSendTable> = Vec::new();
    let mut ld_saw_legacy_end = false;

    let frames = split_length_delimited_frames(sendtables);
    for chunk in frames {
        if ld_flattened.is_none()
            && let Ok(fs) = pb::CsvcMsgFlattenedSerializer::decode(chunk)
        {
            ld_flattened = Some(fs);
            continue;
        }
        if let Ok(st) = pb::CsvcMsgSendTable::decode(chunk) {
            if st.is_end.unwrap_or(false) {
                ld_saw_legacy_end = true;
            } else {
                ld_legacy_tables.push(st);
            }
            continue;
        }
        // unknown chunk type -> ignore
    }

    if let Some(fs) = ld_flattened {
        return build_registry_from_flattened(fs, class_info);
    }
    if ld_saw_legacy_end && !ld_legacy_tables.is_empty() {
        return build_registry_from_legacy(ld_legacy_tables, class_info);
    }

    // Diagnostics
    let report = sniff_sendtables_enhanced(sendtables);
    Err(ParserError::Decode(format!(
        "no SendTables recognized: neither CsvcMsgFlattenedSerializer nor legacy CsvcMsgSendTable frames were found\n{}",
        report
    )))
}

/* ---------- builders ---------- */

fn build_registry_from_flattened(
    fs: pb::CsvcMsgFlattenedSerializer,
    class_info: &pb::CDemoClassInfo,
) -> Result<SerializerRegistry, ParserError> {
    // class id mapping — prefer table_name, fallback to network_name
    let (class_id_by_table, class_id_by_net) = class_maps(class_info);

    // symbol resolver
    let sym = |i: Option<i32>| -> String {
        i.and_then(|x| usize::try_from(x).ok())
            .and_then(|u| fs.symbols.get(u).cloned())
            .unwrap_or_default()
    };

    // fields referenced by index
    let all_fields: Vec<pb::ProtoFlattenedSerializerFieldT> = fs.fields;

    let mut reg = SerializerRegistry::default();

    for s in fs.serializers {
        let name = sym(s.serializer_name_sym);
        let version = s.serializer_version.unwrap_or(0);

        let mut fields_out = Vec::with_capacity(s.fields_index.len());
        for idx in s.fields_index {
            let ui = match usize::try_from(idx) {
                Ok(u) => u,
                Err(_) => continue,
            };
            let f = match all_fields.get(ui) {
                Some(f) => f,
                None => continue,
            };

            // Prefer most specific encoder name: var_encoder_sym > var_serializer_sym > field_serializer_name_sym
            let encoder = {
                let e1 = sym(f.var_encoder_sym);
                if !e1.is_empty() {
                    e1
                } else {
                    let e2 = sym(f.var_serializer_sym);
                    if !e2.is_empty() {
                        e2
                    } else {
                        sym(f.field_serializer_name_sym)
                    }
                }
            };

            let polymorphic = f
                .polymorphic_types
                .iter()
                .map(|p| {
                    (
                        sym(p.polymorphic_field_serializer_name_sym),
                        p.polymorphic_field_serializer_version.unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>();

            fields_out.push(Field {
                name: sym(f.var_name_sym),
                var_type: sym(f.var_type_sym),
                encoder,
                bit_count: f.bit_count.unwrap_or(0),
                low: f.low_value,
                high: f.high_value,
                flags: f.encode_flags.unwrap_or(0) as u32,
                array_len: 0,
                priority: 0,
                polymorphic,
            });
        }

        let class_id = class_id_by_table
            .get(&name)
            .copied()
            .or_else(|| class_id_by_net.get(&name).copied());

        let ser = Serializer {
            name: name.clone(),
            version,
            class_id,
            fields: fields_out,
        };

        reg.by_name.insert(name.clone(), ser.clone());
        if let Some(id) = ser.class_id {
            reg.by_class.insert(id, ser);
        }
    }

    Ok(reg)
}

fn build_registry_from_legacy(
    tables: Vec<pb::CsvcMsgSendTable>,
    class_info: &pb::CDemoClassInfo,
) -> Result<SerializerRegistry, ParserError> {
    let (class_id_by_table, class_id_by_net) = class_maps(class_info);

    // Coalesce by table name; legacy arrives as multiple frames.
    let mut by_name: HashMap<String, Vec<pb::csvc_msg_send_table::SendpropT>> = HashMap::new();
    for t in tables {
        let name = t.net_table_name.unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        by_name.entry(name).or_default().extend(t.props.into_iter());
    }

    let mut reg = SerializerRegistry::default();

    for (name, props) in by_name {
        let mut fields = Vec::with_capacity(props.len());
        for p in props {
            let (var_type, encoder) =
                legacy_type_and_encoder(p.r#type.unwrap_or(0), p.dt_name.as_deref());

            fields.push(Field {
                name: p.var_name.unwrap_or_default(),
                var_type: var_type.to_string(),
                encoder,
                bit_count: p.num_bits.unwrap_or(0),
                low: p.low_value,
                high: p.high_value,
                flags: p.flags.unwrap_or(0) as u32,
                array_len: p.num_elements.unwrap_or(0),
                priority: p.priority.unwrap_or(0),
                polymorphic: Vec::new(),
            });
        }

        let class_id = class_id_by_table
            .get(&name)
            .copied()
            .or_else(|| class_id_by_net.get(&name).copied());

        let ser = Serializer {
            name: name.clone(),
            version: 0,
            class_id,
            fields,
        };

        reg.by_name.insert(name.clone(), ser.clone());
        if let Some(id) = ser.class_id {
            reg.by_class.insert(id, ser);
        }
    }

    Ok(reg)
}

/* ---------- helpers ---------- */

fn class_maps(class_info: &pb::CDemoClassInfo) -> (HashMap<String, u16>, HashMap<String, u16>) {
    let mut by_table = HashMap::new();
    let mut by_net = HashMap::new();
    for c in &class_info.classes {
        if let Some(id) = c.class_id
            && id >= 0
        {
            if let Some(tn) = &c.table_name {
                by_table.insert(tn.clone(), id as u16);
            }
            if let Some(nn) = &c.network_name {
                by_net.insert(nn.clone(), id as u16);
            }
        }
    }
    (by_table, by_net)
}

fn legacy_type_and_encoder(ty: i32, dt_name: Option<&str>) -> (&'static str, String) {
    // Common sendprop type codes; tweak if your generated protos expose an enum.
    match ty {
        0 => ("int", "int".into()),
        1 => ("float", "float".into()),
        2 => ("vector", "Vector".into()),
        3 => ("string", "string".into()),
        4 => ("array", "array".into()),
        5 => ("datatable", dt_name.unwrap_or("").to_string()),
        6 => ("int64", "int64".into()),
        7 => ("vectorxy", "VectorXY".into()),
        _ => ("unknown", format!("unknown({ty})")),
    }
}
