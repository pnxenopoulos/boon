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

/// Effective modifier state for gameplay and derived-stat consumers.
///
/// ModifierState intentionally mirrors the replicated ActiveModifiers table.
/// Valve can leave an active row in that table after its finite duration ends
/// and remove the row later as a bookkeeping operation. Such a row is useful
/// as raw state, but it must not continue to affect derived stats.
///
/// This type keeps the raw, merged state separate from the effective state.
/// That separation is important. A later refresh can be a partial protobuf
/// delta that needs fields from the stale raw row. Deleting the raw row when
/// its timer ends would make that refresh incomplete.
#[derive(Clone, Debug, Default)]
pub struct EffectiveModifierState {
    raw: ModifierState,
    effective_by_serial: HashMap<u32, CModifierTableEntry>,
}

impl EffectiveModifierState {
    /// Modifiers that currently have a gameplay effect.
    pub fn entries(&self) -> &HashMap<u32, CModifierTableEntry> {
        &self.effective_by_serial
    }

    /// Apply this tick's table deltas, then end modifiers whose deadlines pass.
    ///
    /// game_time must use the same Source 2 GameTime_t domain as
    /// CModifierTableEntry::last_applied_time. Callers normally obtain it from
    /// a replicated entity's m_flSimulationTime. Do not pass the HUD match
    /// clock: it has a different origin.
    ///
    /// None disables time-based expiry for this tick. Explicit removals, slot
    /// reuse, and aura exits still apply. This fallback keeps older demos useful
    /// when they do not replicate a compatible clock.
    pub fn update(&mut self, ctx: &Context, game_time: Option<f32>) -> Vec<ModifierChange> {
        let raw_changes = self.raw.update(ctx);
        self.reconcile(raw_changes, game_time)
    }

    /// Rebuild effective state from a complete string-table snapshot.
    ///
    /// A keyframe can contain an already-expired raw row. Filter it during the
    /// rebuild so a segmented parse or a seek gives the same result as a parse
    /// from the start.
    pub fn rebuild(&mut self, ctx: &Context, game_time: Option<f32>) {
        self.raw.rebuild(ctx);
        self.effective_by_serial = self
            .raw
            .entries()
            .iter()
            .filter(|(_, entry)| modifier_is_effective_at(entry, game_time))
            .map(|(&serial, entry)| (serial, entry.clone()))
            .collect();
    }

    /// Clear the raw and effective views.
    pub fn clear(&mut self) {
        self.raw.clear();
        self.effective_by_serial.clear();
    }

    fn reconcile(
        &mut self,
        raw_changes: Vec<ModifierChange>,
        game_time: Option<f32>,
    ) -> Vec<ModifierChange> {
        let mut changes = Vec::with_capacity(raw_changes.len());

        for change in raw_changes {
            let serial = change.serial;
            if change.kind == ModifierChangeKind::Removed {
                // Explicit removal, dispel, owner cleanup, and slot reuse take
                // precedence over the duration deadline. If the timer already
                // ended the effect, suppress this later table cleanup.
                if let Some(entry) = self.effective_by_serial.remove(&serial) {
                    changes.push(ModifierChange {
                        kind: ModifierChangeKind::Removed,
                        serial,
                        entry,
                    });
                }
                continue;
            }

            if modifier_is_effective_at(&change.entry, game_time) {
                // A finite modifier can use the same serial for a refresh. If
                // its old lifetime ended, this is a new effective application
                // even though the raw table calls it a change.
                let kind = if self
                    .effective_by_serial
                    .insert(serial, change.entry.clone())
                    .is_some()
                {
                    ModifierChangeKind::Changed
                } else {
                    ModifierChangeKind::Applied
                };
                changes.push(ModifierChange {
                    kind,
                    serial,
                    entry: change.entry,
                });
            } else if let Some(entry) = self.effective_by_serial.remove(&serial) {
                // Aura exit and a shortened deadline are effective removals
                // even when the replicated row stays present. A later aura
                // entry or refresh becomes a new application.
                changes.push(ModifierChange {
                    kind: ModifierChangeKind::Removed,
                    serial,
                    entry,
                });
            }
        }

        // Most ticks do not change the string table. Timed expiry must still
        // run on every tick. Sort serials so event order does not depend on
        // HashMap iteration order.
        let mut expired: Vec<_> = self
            .effective_by_serial
            .iter()
            .filter(|(_, entry)| !modifier_is_effective_at(entry, game_time))
            .map(|(&serial, _)| serial)
            .collect();
        expired.sort_unstable();
        for serial in expired {
            if let Some(entry) = self.effective_by_serial.remove(&serial) {
                changes.push(ModifierChange {
                    kind: ModifierChangeKind::Removed,
                    serial,
                    entry,
                });
            }
        }

        changes
    }
}

/// Return whether a replicated modifier still has a gameplay effect.
///
/// This function combines only universal rules available in each table row:
///
/// - an aura is inactive when Valve reports that its owner is out of range;
/// - a positive finite duration ends at last_applied_time + duration; and
/// - a negative duration, zero duration, or incomplete timestamp remains
///   active until another replicated transition ends it.
///
/// Zero is not an immediate expiry. Deadlock uses zero-duration rows for
/// modifiers whose lifetime another system controls. A player death is also
/// not a universal rule because some persistent modifiers survive death.
/// Consumers must use modifier-specific metadata for death cleanup.
pub fn modifier_is_effective_at(entry: &CModifierTableEntry, game_time: Option<f32>) -> bool {
    if entry.in_aura_range == Some(false) {
        return false;
    }

    let (Some(now), Some(applied), Some(duration)) = (
        game_time.filter(|value| value.is_finite()),
        entry.last_applied_time.filter(|value| value.is_finite()),
        entry.duration.filter(|value| value.is_finite()),
    ) else {
        return true;
    };

    // Only a positive duration is a self-contained deadline. Negative values
    // mean indefinite, while zero is used by externally controlled modifiers.
    duration <= 0.0 || now < applied + duration
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

    #[test]
    fn finite_modifier_ends_at_its_game_time_deadline() {
        let entry = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..active(7)
        };
        assert!(modifier_is_effective_at(&entry, Some(14.999)));
        assert!(!modifier_is_effective_at(&entry, Some(15.0)));
    }

    #[test]
    fn indefinite_zero_and_incomplete_durations_need_a_replication_transition() {
        for entry in [
            CModifierTableEntry {
                last_applied_time: Some(10.0),
                duration: Some(-1.0),
                ..active(7)
            },
            CModifierTableEntry {
                last_applied_time: Some(10.0),
                duration: Some(0.0),
                ..active(8)
            },
            CModifierTableEntry {
                last_applied_time: None,
                duration: Some(5.0),
                ..active(9)
            },
        ] {
            assert!(modifier_is_effective_at(&entry, Some(100.0)));
        }
    }

    #[test]
    fn aura_exit_ends_an_effect_before_its_deadline() {
        let entry = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            in_aura_range: Some(false),
            ..active(7)
        };
        assert!(!modifier_is_effective_at(&entry, Some(11.0)));
    }

    #[test]
    fn effective_state_expires_but_keeps_raw_state_for_a_refresh() {
        let mut state = EffectiveModifierState::default();
        let initial = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..active(7)
        };
        let applied = state.raw.apply_delta(3, initial);
        let changes = state.reconcile(applied, Some(10.0));
        assert_eq!(changes[0].kind, ModifierChangeKind::Applied);

        let expired = state.reconcile(Vec::new(), Some(15.0));
        assert_eq!(expired[0].kind, ModifierChangeKind::Removed);
        assert!(state.entries().is_empty());
        assert!(state.raw.get(7).is_some());

        let refreshed = state.raw.apply_delta(
            3,
            CModifierTableEntry {
                serial_number: Some(7),
                last_applied_time: Some(20.0),
                ..Default::default()
            },
        );
        let changes = state.reconcile(refreshed, Some(20.0));
        assert_eq!(changes[0].kind, ModifierChangeKind::Applied);
        assert_eq!(
            state.entries().get(&7).unwrap().modifier_subclass,
            Some(100)
        );
    }

    #[test]
    fn explicit_removal_wins_and_late_cleanup_is_not_duplicated() {
        let mut state = EffectiveModifierState::default();
        let initial = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..active(7)
        };
        let applied = state.raw.apply_delta(3, initial);
        state.reconcile(applied, Some(10.0));

        let removed = state.raw.apply_delta(
            9,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(7),
                ..Default::default()
            },
        );
        let changes = state.reconcile(removed, Some(12.0));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ModifierChangeKind::Removed);
        assert!(state.reconcile(Vec::new(), Some(15.0)).is_empty());
    }
}
