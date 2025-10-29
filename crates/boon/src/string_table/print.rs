// src/string_table/print.rs
use crate::string_table::schema::NetSchemaContext;
use crate::string_table::{StringTableEntry, StringTables};
use crate::string_table::update::AppliedChange;

pub fn print_tables(tables: &StringTables, schema: Option<&NetSchemaContext>) {
    for (id, tbl) in tables.by_id.iter().enumerate() {
        println!("StringTable #{id}: '{}' (flags={})", tbl.name, tbl.flags);
        let is_instancebaseline = tbl.name.eq_ignore_ascii_case("instancebaseline");
        for (idx, opt) in tbl.entries.iter().enumerate() {
            match opt {
                Some(e) => print_row(idx, e, is_instancebaseline, schema),
                None => println!("  [{idx:4}] <empty>"),
            }
        }
        println!();
    }
}

pub fn print_changes(
    table_name: &str,
    changes: &[AppliedChange],
    schema: Option<&NetSchemaContext>,
) {
    let is_instancebaseline = table_name.eq_ignore_ascii_case("instancebaseline");
    for ch in changes {
        match &ch.entry {
            Some(e) => print_row(ch.index, e, is_instancebaseline, schema),
            None => println!("  [{:4}] <deleted>", ch.index),
        }
    }
}

fn print_row(
    idx: usize,
    e: &StringTableEntry,
    is_instancebaseline: bool,
    schema: Option<&NetSchemaContext>,
) {
    if is_instancebaseline {
        if let Ok(cid) = e.key.parse::<u32>() {
            if let Some(sc) = schema {
                if let Some(nm) = sc.name_of(cid) {
                    println!("  [{idx:4}] class_id={} ({nm})  baseline_len={}", cid, e.user_data.len());
                    return;
                }
            }
            println!("  [{idx:4}] class_id={}  baseline_len={}", cid, e.user_data.len());
            return;
        }
        println!("  [{idx:4}] key={:?}  baseline_len={}", e.key, e.user_data.len());
    } else {
        println!("  [{idx:4}] key={:?}  data_len={}", e.key, e.user_data.len());
    }
}
