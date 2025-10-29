// src/string_table/parse.rs
use crate::string_table::{StringTable, StringTableEntry, StringTables};
use boon_proto::generated as pb;

/// Build a StringTables snapshot from a CDemoFullPacket (if it contains a string_table).
pub fn from_full_packet(full: &pb::CDemoFullPacket) -> Option<StringTables> {
    full.string_table.as_ref().map(from_cdemo_string_tables)
}

/// Convert CDemoStringTables into our in-memory model.
pub fn from_cdemo_string_tables(st: &pb::CDemoStringTables) -> StringTables {
    let mut out = StringTables::default();

    for table in &st.tables {
        let name = table.table_name.clone().unwrap_or_default();
        let flags = table.table_flags.unwrap_or(0);

        let mut entries = Vec::with_capacity(table.items.len());
        for it in &table.items {
            let key = it.str.clone().unwrap_or_default();
            let user_data = it.data.clone().unwrap_or_default();
            entries.push(Some(StringTableEntry { key, user_data }));
        }

        // (Optional) If you ever want clientside items, mirror the loop above for items_clientside.

        let id = out.by_id.len();
        out.by_name.insert(name.clone(), id);
        out.by_id.push(StringTable { name, flags, entries });
    }

    out
}
