//! Urn carrier transitions from the shared effective modifier stream.
use std::collections::{HashMap, HashSet};

use boon_proto::proto::CModifierTableEntry;

use crate::{
    ModifierChange, ModifierChangeKind,
    hero_stats::{CalculationError, StatCatalog},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrnEventKind {
    PickedUp,
    Dropped,
    Returned,
}

impl UrnEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PickedUp => "picked_up",
            Self::Dropped => "dropped",
            Self::Returned => "returned",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UrnChange {
    pub kind: UrnEventKind,
    /// Full carrier handle, including its entity serial.
    pub parent: u32,
}

#[derive(Debug)]
pub struct UrnState {
    ability_id: u32,
    holding_ids: HashSet<u32>,
    channel_ids: HashSet<u32>,
    holding: HashMap<u32, CModifierTableEntry>,
    channels: HashMap<u32, CModifierTableEntry>,
}

impl UrnState {
    /// Read carrier and delivery-channel modifier IDs from the catalog.
    pub fn with_catalog(catalog: &StatCatalog) -> Result<Self, CalculationError> {
        let (ability_id, holding_ids) =
            catalog.ability_modifier_ids("ability_golden_idol", "m_HoldingIdolModifier")?;
        let (_, channel_ids) =
            catalog.ability_modifier_ids("ability_golden_idol", "m_DropoffTimerModifier")?;
        Ok(Self {
            ability_id,
            holding_ids,
            channel_ids,
            holding: HashMap::new(),
            channels: HashMap::new(),
        })
    }

    /// Consume each tick's ordered effective changes once.
    ///
    /// A return requires carrier loss, a completed channel for the same full
    /// ability handle, and an observed delivery-point closure on this tick.
    /// Missing evidence leaves the event as a drop. Entering the delivery area
    /// alone is not a completed delivery.
    pub fn update(
        &mut self,
        changes: &[ModifierChange],
        game_time: Option<f32>,
        delivery_closed: bool,
    ) -> Vec<UrnChange> {
        let mut events = Vec::new();
        let mut losses = Vec::new();
        // Keep removed channels until the batch ends: their removal can precede
        // the carrier's removal in the same tick.
        let mut removed_channels = Vec::new();
        for change in changes {
            let entry = &change.entry;
            let removed = change.kind == ModifierChangeKind::Removed;
            let owner_matches = entry.ability_subclass == Some(self.ability_id);
            let holding = !removed
                && owner_matches
                && entry
                    .modifier_subclass
                    .is_some_and(|id| self.holding_ids.contains(&id));
            if self
                .holding
                .get(&change.serial)
                .is_some_and(|old| !holding || old.parent != entry.parent)
                && let Some(old) = self.holding.remove(&change.serial)
                && let Some(parent) = old.parent
                && !self
                    .holding
                    .values()
                    .any(|other| other.parent == Some(parent))
            {
                losses.push((events.len(), old));
                events.push(UrnChange {
                    kind: UrnEventKind::Dropped,
                    parent,
                });
            }
            if holding && let Some(parent) = entry.parent {
                let already_held = self
                    .holding
                    .values()
                    .any(|other| other.parent == Some(parent));
                self.holding.insert(change.serial, entry.clone());
                if !already_held {
                    events.push(UrnChange {
                        kind: UrnEventKind::PickedUp,
                        parent,
                    });
                }
            }
            if owner_matches
                && entry
                    .modifier_subclass
                    .is_some_and(|id| self.channel_ids.contains(&id))
            {
                if removed {
                    self.channels.remove(&change.serial);
                    removed_channels.push(entry);
                } else {
                    self.channels.insert(change.serial, entry.clone());
                }
            } else {
                self.channels.remove(&change.serial);
            }
        }
        if delivery_closed {
            for (index, carrier) in losses {
                if self
                    .channels
                    .values()
                    .chain(removed_channels.iter().copied())
                    .any(|channel| completed_channel(channel, &carrier, game_time))
                {
                    events[index].kind = UrnEventKind::Returned;
                }
            }
        }
        events
    }
}

fn completed_channel(
    channel: &CModifierTableEntry,
    carrier: &CModifierTableEntry,
    time: Option<f32>,
) -> bool {
    let (Some(time), Some(start), Some(duration), Some(held_since), Some(ability)) = (
        time,
        channel.last_applied_time.or(channel.creation_time),
        channel.duration,
        carrier.creation_time.or(carrier.last_applied_time),
        carrier.ability,
    ) else {
        return false;
    };
    channel.parent == carrier.parent
        && channel.ability == Some(ability)
        && time.is_finite()
        && start.is_finite()
        && held_since.is_finite()
        && duration.is_finite()
        && duration > 0.0
        && start >= held_since
        && time >= start + duration
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModifierState;
    use serde_json::json;

    fn state() -> UrnState {
        let file = |name, records| {
            serde_json::to_vec(&json!({
                "catalog":name,"client_version":"test","source_commit":"test","records":records
            }))
            .unwrap()
        };
        let empty = json!([]);
        let catalog = StatCatalog::from_json_bytes(
            &file("heroes", &empty),
            &file("abilities", &json!([{"record_key":"idol","definition_path":"/idol","definition":{},"stat_changes":[],"ability_id":3,"ability_name":"ability_golden_idol"}])),
            &file("modifiers", &json!([
                {"record_key":"held","definition_path":"/idol/m_HoldingIdolModifier","definition":{},"stat_changes":[],"ability_id":3,"modifier_id":11,"qualified_modifier_id":12},
                {"record_key":"channel","definition_path":"/idol/m_DropoffTimerModifier","definition":{},"stat_changes":[],"ability_id":3,"modifier_id":21,"qualified_modifier_id":22}
            ])),
            &file("misc", &empty),
        ).unwrap();
        UrnState::with_catalog(&catalog).unwrap()
    }

    fn entry(serial: u32, parent: u32, id: u32) -> CModifierTableEntry {
        CModifierTableEntry {
            serial_number: Some(serial),
            parent: Some(parent),
            modifier_subclass: Some(id),
            ability: Some(123),
            ability_subclass: Some(3),
            creation_time: Some(10.0),
            ..Default::default()
        }
    }

    #[test]
    fn slot_reuse_and_partial_updates_preserve_full_carrier_identity() {
        let mut state = state();
        let mut raw = ModifierState::default();
        let first = (9 << 14) | 2;
        let reused = (10 << 14) | 2;
        let changes = raw.apply_delta(0, entry(1, first, 12));
        assert_eq!(
            state.update(&changes, None, false),
            [UrnChange {
                kind: UrnEventKind::PickedUp,
                parent: first
            }]
        );
        // An unrelated write reuses the event slot; it cannot drop the Urn.
        let mut unrelated = entry(2, first, 12);
        unrelated.ability_subclass = Some(4);
        let changes = raw.apply_delta(0, unrelated);
        assert!(state.update(&changes, None, false).is_empty());
        let changes = raw.apply_delta(
            0,
            CModifierTableEntry {
                serial_number: Some(1),
                stack_count: Some(2),
                ..Default::default()
            },
        );
        assert!(state.update(&changes, None, false).is_empty());
        let changes = raw.apply_delta(0, entry(3, reused, 11));
        assert_eq!(state.update(&changes, None, false)[0].parent, reused);
        let changes = raw.apply_delta(
            0,
            CModifierTableEntry {
                serial_number: Some(1),
                entry_type: Some(2),
                ..Default::default()
            },
        );
        assert_eq!(
            state.update(&changes, None, false),
            [UrnChange {
                kind: UrnEventKind::Dropped,
                parent: first
            }]
        );
        assert!(state.holding.contains_key(&3));
    }

    #[test]
    fn returns_require_completed_matching_channel_and_point_closure() {
        for (time, closed, ability, expected) in [
            (Some(20.2), true, Some(123), UrnEventKind::Returned),
            (None, true, Some(123), UrnEventKind::Dropped),
            (Some(20.05), true, Some(123), UrnEventKind::Dropped),
            (Some(20.2), false, Some(123), UrnEventKind::Dropped),
            (Some(20.2), true, Some(124), UrnEventKind::Dropped),
            (Some(20.2), true, None, UrnEventKind::Dropped),
        ] {
            for channel_first in [false, true] {
                let mut state = state();
                let carrier = entry(1, 2, 12);
                let mut channel = entry(2, 2, 22);
                channel.last_applied_time = Some(20.0);
                channel.duration = Some(0.1);
                channel.ability = ability;
                let change = |kind, entry: CModifierTableEntry| ModifierChange {
                    kind,
                    serial: entry.serial_number.unwrap(),
                    entry,
                };
                state.update(
                    &[
                        change(ModifierChangeKind::Applied, carrier.clone()),
                        change(ModifierChangeKind::Applied, channel.clone()),
                    ],
                    Some(20.0),
                    false,
                );
                let mut removals = [
                    change(ModifierChangeKind::Removed, carrier),
                    change(ModifierChangeKind::Removed, channel),
                ];
                if channel_first {
                    removals.reverse();
                }
                assert_eq!(state.update(&removals, time, closed)[0].kind, expected);
            }
        }
    }

    #[test]
    fn same_tick_pickup_and_drop_keep_wire_order() {
        let mut raw = ModifierState::default();
        let mut changes = raw.apply_delta(0, entry(1, 2, 12));
        changes.extend(raw.apply_delta(
            0,
            CModifierTableEntry {
                serial_number: Some(1),
                entry_type: Some(2),
                ..Default::default()
            },
        ));
        let events = state().update(&changes, None, false);
        assert_eq!(
            events.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [UrnEventKind::PickedUp, UrnEventKind::Dropped]
        );
    }
}
