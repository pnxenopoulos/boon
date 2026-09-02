//! State tracking for Deadlock's `ActiveModifiers` string table.
//!
//! String-table entries are protobuf changes. An update can contain only changed
//! fields. [`ModifierState`] merges the changes. It handles slot reuse and
//! explicit removals. It can rebuild state from a keyframe snapshot.

use std::collections::HashMap;

use boon_proto::proto::CModifierTableEntry;
use prost::Message;

use crate::Context;

/// The lifecycle transition produced by a modifier-table delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModifierChangeKind {
    Applied,
    Changed,
    Removed,
}

impl ModifierChangeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

/// One modifier lifecycle change.
#[derive(Clone, Debug, PartialEq)]
pub struct ModifierChange {
    pub kind: ModifierChangeKind,
    pub serial: u32,
    /// Complete state after an apply or change.
    /// A removal contains the last complete state.
    pub entry: CModifierTableEntry,
}

/// Complete live state for the `ActiveModifiers` string table.
#[derive(Clone, Debug, Default)]
pub struct ModifierState {
    by_serial: HashMap<u32, CModifierTableEntry>,
    index_to_serial: HashMap<usize, u32>,
}

impl ModifierState {
    /// Current live modifiers, keyed by their runtime serial number.
    pub fn entries(&self) -> &HashMap<u32, CModifierTableEntry> {
        &self.by_serial
    }

    /// A live modifier by serial number.
    pub fn get(&self, serial: u32) -> Option<&CModifierTableEntry> {
        self.by_serial.get(&serial)
    }

    /// Clear all tracked state.
    pub fn clear(&mut self) {
        self.by_serial.clear();
        self.index_to_serial.clear();
    }

    /// Apply the entries touched by this tick's string-table delta.
    pub fn update(&mut self, ctx: &Context) -> Vec<ModifierChange> {
        let Some(table) = ctx.string_tables().find_table("ActiveModifiers") else {
            return Vec::new();
        };
        let mut changes = Vec::new();
        for &index in table.dirty_indices() {
            let Some(data) = table
                .entries()
                .get(index)
                .and_then(|entry| entry.user_data.as_deref())
                .filter(|data| !data.is_empty())
            else {
                continue;
            };
            if let Ok(delta) = CModifierTableEntry::decode(data) {
                changes.extend(self.apply_delta(index, delta));
            }
        }
        changes
    }

    /// Build live state from a complete string-table snapshot.
    ///
    /// ActiveModifiers stores apply, update, and remove rows in table order.
    /// A reused slot contains its newest value. Processing the table produces
    /// the same active serial set as processing each change from signon.
    pub fn rebuild(&mut self, ctx: &Context) {
        self.clear();
        let Some(table) = ctx.string_tables().find_table("ActiveModifiers") else {
            return;
        };
        for (index, table_entry) in table.entries().iter().enumerate() {
            let Some(data) = table_entry
                .user_data
                .as_deref()
                .filter(|data| !data.is_empty())
            else {
                continue;
            };
            if let Ok(delta) = CModifierTableEntry::decode(data) {
                self.apply_delta(index, delta);
            }
        }
    }

    /// Merge one decoded string-table delta.
    pub fn apply_delta(&mut self, index: usize, delta: CModifierTableEntry) -> Vec<ModifierChange> {
        let Some(serial) = delta.serial_number else {
            return Vec::new();
        };
        let mut changes = Vec::with_capacity(2);

        // A new serial can overwrite a slot.
        // The table does not always contain a removal row for the old serial.
        if let Some(old_serial) = self.index_to_serial.get(&index).copied()
            && old_serial != serial
            && let Some(entry) = self.by_serial.remove(&old_serial)
        {
            changes.push(ModifierChange {
                kind: ModifierChangeKind::Removed,
                serial: old_serial,
                entry,
            });
        }

        if delta.entry_type.unwrap_or(1) == 2 {
            self.index_to_serial.remove(&index);
            if let Some(entry) = self.by_serial.remove(&serial) {
                changes.push(ModifierChange {
                    kind: ModifierChangeKind::Removed,
                    serial,
                    entry,
                });
            }
            return changes;
        }

        self.index_to_serial.insert(index, serial);
        match self.by_serial.entry(serial) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(delta.clone());
                changes.push(ModifierChange {
                    kind: ModifierChangeKind::Applied,
                    serial,
                    entry: delta,
                });
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                let previous = slot.get().clone();
                merge_entry(slot.get_mut(), delta);
                if *slot.get() != previous {
                    changes.push(ModifierChange {
                        kind: ModifierChangeKind::Changed,
                        serial,
                        entry: slot.get().clone(),
                    });
                }
            }
        }
        changes
    }
}

/// Merge present protobuf fields into an existing entry.
fn merge_entry(current: &mut CModifierTableEntry, delta: CModifierTableEntry) {
    macro_rules! merge {
        ($($field:ident),+ $(,)?) => {
            $(if delta.$field.is_some() {
                current.$field = delta.$field;
            })+
        };
    }

    merge!(
        entry_type,
        parent,
        serial_number,
        modifier_subclass,
        stack_count,
        max_stack_count,
        last_applied_time,
        duration,
        caster,
        ability,
        aura_provider_serial_number,
        aura_provider_ehandle,
        ability_subclass,
        in_aura_range,
        bool1,
        bool2,
        bool3,
        bool4,
        int1,
        int2,
        int3,
        int4,
        float1,
        float2,
        float3,
        float4,
        float5,
        float6,
        float7,
        float8,
        float9,
        float10,
        float11,
        float12,
        float13,
        float14,
        float15,
        float16,
        uint1,
        uint2,
        uint3,
        uint4,
        vec1,
        vec2,
        vec3,
        vec4,
        string1,
        string2,
        string3,
        string4,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(serial: u32) -> CModifierTableEntry {
        CModifierTableEntry {
            entry_type: Some(1),
            parent: Some(42),
            serial_number: Some(serial),
            modifier_subclass: Some(100),
            duration: Some(5.0),
            stack_count: Some(1),
            float1: Some(7.0),
            ..Default::default()
        }
    }

    #[test]
    fn merges_partial_updates() {
        let mut state = ModifierState::default();
        assert_eq!(
            state.apply_delta(3, active(7))[0].kind,
            ModifierChangeKind::Applied
        );

        let changes = state.apply_delta(
            3,
            CModifierTableEntry {
                serial_number: Some(7),
                stack_count: Some(2),
                ..Default::default()
            },
        );
        assert_eq!(changes[0].kind, ModifierChangeKind::Changed);
        let entry = state.get(7).unwrap();
        assert_eq!(entry.stack_count, Some(2));
        assert_eq!(entry.duration, Some(5.0));
        assert_eq!(entry.float1, Some(7.0));
    }

    #[test]
    fn removes_on_explicit_delta_and_slot_reuse() {
        let mut state = ModifierState::default();
        state.apply_delta(3, active(7));
        let removed = state.apply_delta(
            9,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(removed[0].kind, ModifierChangeKind::Removed);
        assert!(state.get(7).is_none());

        state.apply_delta(3, active(8));
        let changes = state.apply_delta(3, active(9));
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].kind, ModifierChangeKind::Removed);
        assert_eq!(changes[0].serial, 8);
        assert_eq!(changes[1].kind, ModifierChangeKind::Applied);
        assert_eq!(changes[1].serial, 9);
    }
}
