// src/string_table/update.rs
use crate::reader::Reader;
use crate::string_table::{StringTable, StringTableEntry, StringTables};
use boon_proto::generated as pb;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("unknown table id {0}")]
    BadTableId(usize),
    #[error("malformed update payload: {0}")]
    Malformed(&'static str),
}

const HISTORY_LIMIT: usize = 32;

#[derive(Debug, Clone)]
pub struct AppliedChange {
    pub index: usize,
    /// Final entry after the mutation (None would mean deletion if you add that later)
    pub entry: Option<StringTableEntry>,
}

/// Apply a Source-2 `svc_UpdateStringTable` and return the final state of each changed row.
pub fn apply_csvc_update_verbose(
    tables: &mut StringTables,
    msg: &pb::CsvcMsgUpdateStringTable,
) -> Result<Vec<AppliedChange>, UpdateError> {
    let table_id = msg.table_id.unwrap_or_default() as usize;
    let changes = msg.num_changed_entries.unwrap_or_default() as usize;
    let data = msg.string_data.as_deref().unwrap_or(&[]);

    let tbl: &mut StringTable = tables
        .by_id
        .get_mut(table_id)
        .ok_or(UpdateError::BadTableId(table_id))?;

    let mut r = Reader::new(data);
    let mut last_index: isize = -1;
    let mut history: Vec<String> = Vec::new();

    let mut out: Vec<AppliedChange> = Vec::with_capacity(changes);

    for _ in 0..changes {
        // 1) index prediction
        let sequential = r.read_bool();
        let index: usize = if sequential {
            (last_index + 1) as usize
        } else {
            r.read_var_u32_no_refill() as usize
        };
        last_index = index as isize;

        if index >= tbl.entries.len() {
            tbl.entries.resize(index + 1, None);
        }

        // 2) key (optional)
        let mut key: Option<String> = None;
        if r.read_bool() {
            let use_substr = r.read_bool();
            if use_substr {
                let hidx = r.read_var_u32_no_refill() as usize;
                let prefix_len = r.read_var_u32_no_refill() as usize;
                let suffix_len = r.read_var_u32_no_refill() as usize;

                let base = history.get(hidx).cloned().unwrap_or_default();
                let prefix = base.get(..prefix_len).unwrap_or("");
                let suffix_bytes = r.read_bytes(suffix_len as u32);
                let s = format!("{prefix}{}", String::from_utf8_lossy(&suffix_bytes));
                push_history(&mut history, &s);
                key = Some(s);
            } else {
                let len = r.read_var_u32_no_refill() as usize;
                let bytes = r.read_bytes(len as u32);
                let s = String::from_utf8_lossy(&bytes).into_owned();
                push_history(&mut history, &s);
                key = Some(s);
            }
        }

        // 3) user data (optional; usually varlen)
        let mut user_data: Option<Vec<u8>> = None;
        if r.read_bool() {
            let len = r.read_var_u32_no_refill() as usize;
            user_data = Some(r.read_bytes(len as u32));
        }

        // 4) apply (no explicit deletion handling yet)
        let (prev_key, prev_data) = match &tbl.entries[index] {
            Some(e) => (Some(e.key.clone()), Some(e.user_data.clone())),
            None => (None, None),
        };

        let final_key = key.or(prev_key).unwrap_or_default();
        let final_data = user_data.or(prev_data).unwrap_or_default();

        let final_entry = Some(StringTableEntry {
            key: final_key,
            user_data: final_data,
        });

        tbl.entries[index] = final_entry.clone();
        out.push(AppliedChange { index, entry: final_entry });
    }

    Ok(out)
}

fn push_history(hist: &mut Vec<String>, s: &str) {
    hist.insert(0, s.to_string());
    if hist.len() > HISTORY_LIMIT {
        hist.pop();
    }
}
