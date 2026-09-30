//! State tracking for Deadlock's `ActiveModifiers` string table.
//!
//! String-table entries are protobuf changes. An update can contain only changed
//! fields. [`ModifierState`] merges the changes. It handles slot reuse and
//! explicit removals. It can rebuild state from a keyframe snapshot.

use std::collections::HashMap;

use boon_proto::proto::CModifierTableEntry;
use prost::Message;

use crate::{Context, EntityContainer, FieldValue};

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
    /// Reused event slots can omit earlier applications at arbitrary ticks.
    /// A relay keyframe can also contain modifier state ahead of its entities.
    /// Use packet deltas from signon for calculations at an exact replay tick.
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
    pub fn apply_delta(
        &mut self,
        _index: usize,
        delta: CModifierTableEntry,
    ) -> Vec<ModifierChange> {
        let Some(serial) = delta.serial_number else {
            return Vec::new();
        };
        let mut changes = Vec::with_capacity(1);

        // Table slots carry events. Reusing a slot does not end the previous
        // serial's effect (for example, an equipped item's intrinsic modifier).
        // Only an explicit removal ends raw state; effective state also expires.
        if delta.entry_type.unwrap_or(1) == 2 {
            if let Some(entry) = self.by_serial.remove(&serial) {
                changes.push(ModifierChange {
                    kind: ModifierChangeKind::Removed,
                    serial,
                    entry,
                });
            }
            return changes;
        }

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
                if merge_entry(slot.get_mut(), delta) {
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

/// Cached field keys for the game clock used by modifier timestamps.
/// Resolve these keys once per replay, after its serializers are available.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModifierClock {
    simulation_time: Option<u64>,
    tick_base: Option<u64>,
    hero_pawn: Option<u64>,
    pawn: Option<u64>,
    total_paused_ticks: Option<u64>,
}

impl ModifierClock {
    pub fn resolve(ctx: &Context) -> Self {
        let controller = ctx.serializers().get("CCitadelPlayerController");
        Self {
            tick_base: controller.and_then(|s| s.resolve_field_key("m_nTickBase")),
            hero_pawn: controller.and_then(|s| s.resolve_field_key("m_hHeroPawn")),
            pawn: controller.and_then(|s| s.resolve_field_key("m_hPawn")),
            simulation_time: ctx
                .serializers()
                .get("CCitadelPlayerPawn")
                .and_then(|s| s.resolve_field_key("m_flSimulationTime")),
            total_paused_ticks: ctx
                .serializers()
                .get("CCitadelGameRulesProxy")
                .and_then(|s| s.resolve_field_key("m_pGameRules.m_nTotalPausedTicks")),
        }
    }

    /// Read simulation time minus accumulated pauses, in seconds.
    /// Uses pawn simulation time when present, or controller tick bases otherwise.
    /// Missing clock fields or a missing pause counter return `None`.
    pub fn game_time(&self, ctx: &Context) -> Option<f32> {
        self.time_from_entities(ctx.entities(), ctx.tick_interval())
    }

    fn time_from_entities(&self, entities: &EntityContainer, interval: f32) -> Option<f32> {
        let simulation_time = self
            .simulation_time
            .and_then(|key| {
                entities
                    .iter()
                    .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerPawn")
                    .filter_map(|(_, e)| match e.fields.get(&key)? {
                        FieldValue::F32(value) if value.is_finite() => Some(*value),
                        _ => None,
                    })
                    .max_by(f32::total_cmp)
            })
            .or_else(|| {
                // New demos omit pawn simulation time. Player controller tick bases
                // share the modifier clock's origin after accumulated pauses are removed.
                // Spectator controllers can be ahead: only use controllers with a hero pawn.
                entities
                    .iter()
                    .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
                    .filter(|(_, e)| {
                        [self.hero_pawn, self.pawn].into_iter().any(|key| {
                            e.get_handle(key)
                                .and_then(|handle| entities.get_by_handle(handle))
                                .is_some_and(|pawn| {
                                    pawn.class_name.as_ref() == "CCitadelPlayerPawn"
                                })
                        })
                    })
                    .filter_map(|(_, e)| u32::try_from(e.get_u64(self.tick_base)?).ok())
                    .max()
                    .map(|tick| tick as f32 * interval)
            })?;
        let (_, rules) = entities
            .iter()
            .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")?;
        let paused_ticks = u32::try_from(rules.get_u64(self.total_paused_ticks)?).ok()?;
        pause_adjusted_time(simulation_time, paused_ticks, interval)
    }
}

fn pause_adjusted_time(simulation_time: f32, paused_ticks: u32, interval: f32) -> Option<f32> {
    if !interval.is_finite() || interval <= 0.0 {
        return None;
    }
    let time = simulation_time - paused_ticks as f32 * interval;
    time.is_finite().then_some(time)
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
    ended_applications: HashMap<u32, ModifierApplication>,
}

/// Recorded identity of one application, separate from later payload changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModifierApplication(Option<u32>, Option<u32>);

impl From<&CModifierTableEntry> for ModifierApplication {
    fn from(entry: &CModifierTableEntry) -> Self {
        Self(
            entry.creation_time.map(f32::to_bits),
            entry.last_applied_time.map(f32::to_bits),
        )
    }
}

impl EffectiveModifierState {
    /// Modifiers that currently have a gameplay effect.
    pub fn entries(&self) -> &HashMap<u32, CModifierTableEntry> {
        &self.effective_by_serial
    }

    /// Apply this tick's table deltas, then end modifiers whose deadlines pass.
    ///
    /// game_time must use the same Source 2 GameTime_t domain as
    /// CModifierTableEntry::last_applied_time. Use [`ModifierClock::game_time`]
    /// to subtract accumulated pauses from entity simulation time. Do not pass
    /// the HUD match clock: it has a different origin.
    ///
    /// None disables time-based expiry for this tick. Explicit removals
    /// and aura exits still apply. This fallback keeps older demos useful
    /// when they do not replicate a compatible clock.
    pub fn update(&mut self, ctx: &Context, game_time: Option<f32>) -> Vec<ModifierChange> {
        let raw_changes = self.raw.update(ctx);
        self.reconcile(raw_changes, game_time)
    }

    /// Rebuild effective state from a complete string-table snapshot.
    ///
    /// Expired rows are filtered. This does not correct a relay snapshot whose
    /// capture time differs from the entity tick; exact stat queries replay
    /// packet deltas instead.
    pub fn rebuild(&mut self, ctx: &Context, game_time: Option<f32>) {
        self.ended_applications.clear();
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
        self.ended_applications.clear();
    }

    /// End effects using additional replay evidence, while keeping raw payloads.
    /// A changed stack/value cannot revive the same application. A new recorded
    /// application time can; it may need fields retained in the raw row.
    pub(crate) fn end_application(&mut self, serial: u32) {
        if let Some(entry) = self.effective_by_serial.remove(&serial) {
            self.ended_applications
                .insert(serial, ModifierApplication::from(&entry));
        }
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
                // A table refresh can move an unchanged serial to another slot.
                // Keep its lifetime when the final raw row is identical and active.
                if self.raw.get(serial) == Some(&change.entry)
                    && modifier_is_effective_at(&change.entry, game_time)
                {
                    continue;
                }
                self.ended_applications.remove(&serial);
                // Explicit removal, dispel, and owner cleanup take
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

            if self.ended_applications.get(&serial)
                == Some(&ModifierApplication::from(&change.entry))
            {
                continue;
            }
            self.ended_applications.remove(&serial);
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

/// Merge present fields and report whether the delta changes the stored values.
fn merge_entry(current: &mut CModifierTableEntry, delta: CModifierTableEntry) -> bool {
    let mut changed = false;
    macro_rules! merge {
        ($($field:ident),+ $(,)?) => {
            $(if let Some(value) = delta.$field {
                changed |= current.$field.as_ref() != Some(&value);
                current.$field = Some(value);
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
        creation_time,
        attributes,
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
    changed
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
    fn state_mask_expiry_persists_until_a_recorded_refresh() {
        let mut state = EffectiveModifierState::default();
        let mut entry = active(7);
        entry.duration = None;
        entry.creation_time = Some(10.0);
        entry.last_applied_time = Some(10.0);
        let changes = state.raw.apply_delta(0, entry);
        state.reconcile(changes, Some(10.0));
        state.end_application(7);
        assert!(state.entries().is_empty());
        // Moving an unchanged raw row between slots cannot restore the effect.
        let same = state.raw.get(7).unwrap().clone();
        let mut changes = state.raw.apply_delta(
            0,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(7),
                ..Default::default()
            },
        );
        changes.extend(state.raw.apply_delta(1, same));
        assert!(state.reconcile(changes, Some(11.0)).is_empty());
        let changes = state.raw.apply_delta(
            1,
            CModifierTableEntry {
                serial_number: Some(7),
                stack_count: Some(2),
                ..Default::default()
            },
        );
        assert!(state.reconcile(changes, Some(20.0)).is_empty());
        assert!(state.entries().is_empty());
        let changes = state.raw.apply_delta(
            2,
            CModifierTableEntry {
                serial_number: Some(7),
                last_applied_time: Some(20.0),
                ..Default::default()
            },
        );
        assert_eq!(
            state.reconcile(changes, Some(20.0))[0].kind,
            ModifierChangeKind::Applied
        );
        assert_eq!(state.entries()[&7].stack_count, Some(2));
        state.end_application(7);
        let changes = state.raw.apply_delta(
            3,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(7),
                ..Default::default()
            },
        );
        state.reconcile(changes, Some(21.0));
        assert!(state.ended_applications.is_empty());
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
    fn unchanged_deltas_preserve_owned_payloads_without_emitting_changes() {
        let mut state = ModifierState::default();
        let original = CModifierTableEntry {
            string1: Some("retained payload".into()),
            creation_time: Some(10.0),
            attributes: Some(32),
            int1: Some(12),
            ..active(7)
        };
        state.apply_delta(3, original.clone());

        for delta in [
            original.clone(),
            CModifierTableEntry {
                serial_number: Some(7),
                ..Default::default()
            },
        ] {
            assert!(state.apply_delta(3, delta).is_empty());
            assert_eq!(state.get(7), Some(&original));
        }

        let changes = state.apply_delta(
            3,
            CModifierTableEntry {
                serial_number: Some(7),
                string1: Some(String::new()),
                creation_time: Some(0.0),
                attributes: Some(0),
                int1: Some(0),
                ..Default::default()
            },
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ModifierChangeKind::Changed);
        assert_eq!(changes[0].entry.string1.as_deref(), Some(""));
        assert_eq!(changes[0].entry.int1, Some(0));
        assert_eq!(changes[0].entry.creation_time, Some(0.0));
        assert_eq!(changes[0].entry.attributes, Some(0));
        assert_eq!(changes[0].entry.float1, original.float1);
        assert_eq!(
            changes[0].entry.modifier_subclass,
            original.modifier_subclass
        );
    }

    #[test]
    fn slot_reuse_preserves_other_serials_until_explicit_removal() {
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
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ModifierChangeKind::Applied);
        assert_eq!(changes[0].serial, 9);
        assert!(state.get(8).is_some());
        let removed = state.apply_delta(
            3,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(8),
                ..Default::default()
            },
        );
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].serial, 8);
        assert!(state.get(9).is_some());
    }

    #[test]
    fn controller_clock_excludes_spectators_and_preserves_legacy_simulation_time() {
        let clock = ModifierClock {
            simulation_time: Some(0),
            tick_base: Some(1),
            total_paused_ticks: Some(2),
            hero_pawn: Some(3),
            pawn: Some(4),
        };
        let mut entities = EntityContainer::new();
        let entity = |id, class, fields: Vec<(u64, FieldValue)>| {
            crate::Entity::from_fields(id, 0, 0, class, true, fields.into_iter().collect()).unwrap()
        };
        entities
            .insert(entity(10, "CCitadelPlayerPawn", vec![]))
            .unwrap();
        entities
            .insert(entity(
                1,
                "CCitadelPlayerController",
                vec![(1, FieldValue::U32(1600)), (3, FieldValue::U32(10))],
            ))
            .unwrap();
        entities
            .insert(entity(
                2,
                "CCitadelPlayerController",
                vec![
                    (1, FieldValue::U32(9000)), // No hero pawn: spectator clock must not win.
                ],
            ))
            .unwrap();
        assert_eq!(clock.time_from_entities(&entities, 1.0 / 64.0), None);
        entities
            .insert(entity(
                20,
                "CCitadelGameRulesProxy",
                vec![(2, FieldValue::U32(640))],
            ))
            .unwrap();
        assert_eq!(clock.time_from_entities(&entities, 1.0 / 64.0), Some(15.0));
        // More paused ticks and simulation ticks leave game time unchanged.
        entities
            .insert(entity(
                1,
                "CCitadelPlayerController",
                vec![(1, FieldValue::I64(1664)), (4, FieldValue::U32(10))],
            ))
            .unwrap();
        entities
            .insert(entity(
                20,
                "CCitadelGameRulesProxy",
                vec![(2, FieldValue::I64(704))],
            ))
            .unwrap();
        assert_eq!(clock.time_from_entities(&entities, 1.0 / 64.0), Some(15.0));
        // Older demos retain their existing clock, even if a controller disagrees.
        entities
            .insert(entity(
                10,
                "CCitadelPlayerPawn",
                vec![(0, FieldValue::F32(25.0))],
            ))
            .unwrap();
        assert_eq!(clock.time_from_entities(&entities, 1.0 / 64.0), Some(14.0));
        assert_eq!(clock.time_from_entities(&entities, f32::NAN), None);
    }

    #[test]
    fn paused_time_does_not_expire_modifiers_early() {
        let entry = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..active(7)
        };
        // Ten seconds of pauses advance simulation time, but not the timer.
        let before = pause_adjusted_time(24.0, 640, 1.0 / 64.0);
        let deadline = pause_adjusted_time(25.0, 640, 1.0 / 64.0);
        assert_eq!(before, Some(14.0));
        assert!(modifier_is_effective_at(&entry, before));
        assert!(!modifier_is_effective_at(&entry, deadline));
        assert_eq!(pause_adjusted_time(14.0, 0, 1.0 / 64.0), before);
        assert_eq!(pause_adjusted_time(f32::NAN, 0, 1.0 / 64.0), None);
        assert_eq!(pause_adjusted_time(14.0, 0, 0.0), None);
        assert_eq!(pause_adjusted_time(14.0, 0, f32::NAN), None);
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
    fn table_refresh_preserves_an_unchanged_modifiers_deadline() {
        let mut state = EffectiveModifierState::default();
        let initial = CModifierTableEntry {
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..active(7)
        };
        let applied = state.raw.apply_delta(3, initial.clone());
        state.reconcile(applied, Some(10.0));

        let mut refresh = state.raw.apply_delta(3, active(8));
        refresh.extend(state.raw.apply_delta(4, initial.clone()));
        let changes = state.reconcile(refresh, Some(12.0));
        assert!(
            !changes
                .iter()
                .any(|change| change.serial == 7 && change.kind == ModifierChangeKind::Removed)
        );
        assert_eq!(state.entries().get(&7), Some(&initial));
        let expired = state.reconcile(Vec::new(), Some(15.0));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].serial, 7);
        assert_eq!(expired[0].kind, ModifierChangeKind::Removed);
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
