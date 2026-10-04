//! End a known modifier application when its recorded states disappear.
use std::collections::HashMap;

use boon_proto::proto::CModifierTableEntry;

use super::StatCatalog;
use crate::{
    Context, EffectiveModifierState, ModifierChange, modifier_state::ModifierApplication,
    player_states::StateEvidenceKeys,
};

type ModifierId = (u32, Option<u32>);
type StateMask = Vec<(usize, u32)>;

pub(super) struct ModifierLifetimes {
    keys: Option<StateEvidenceKeys>,
    masks: HashMap<ModifierId, StateMask>,
    candidates: HashMap<u32, (u32, ModifierId)>,
    observed: HashMap<u32, ModifierApplication>,
    evidence: HashMap<u32, Option<Vec<u32>>>,
}

impl ModifierLifetimes {
    pub(super) fn new(
        ctx: &Context,
        catalog: &StatCatalog,
        state: &EffectiveModifierState,
    ) -> Self {
        let mut reader = Self {
            keys: ctx
                .serializers()
                .get("CCitadelPlayerPawn")
                .and_then(StateEvidenceKeys::resolve),
            masks: HashMap::new(),
            candidates: HashMap::new(),
            observed: HashMap::new(),
            evidence: HashMap::new(),
        };
        for entry in state.entries().values() {
            reader.track(catalog, entry);
        }
        reader
    }

    fn track(&mut self, catalog: &StatCatalog, entry: &CModifierTableEntry) {
        let (Some(serial), Some(parent), Some(id)) =
            (entry.serial_number, entry.parent, entry.modifier_subclass)
        else {
            return;
        };
        let id = (id, entry.ability_subclass);
        let mask = self.masks.entry(id).or_insert_with(|| {
            let Some(source) = catalog.modifier(id.0, id.1).ok() else {
                return Vec::new();
            };
            let Some(names) = source.definition["m_nEnabledStateMask"].as_str() else {
                return Vec::new();
            };
            names
                .split('|')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| {
                    let index = *catalog.modifier_states.get(name)?;
                    Some((index as usize / 32, 1 << (index % 32)))
                })
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default()
        });
        if mask.is_empty() {
            self.candidates.remove(&serial);
            self.observed.remove(&serial);
        } else {
            self.candidates.insert(serial, (parent, id));
        }
    }

    pub(super) fn update(
        &mut self,
        ctx: &Context,
        catalog: &StatCatalog,
        state: &mut EffectiveModifierState,
        changes: &[ModifierChange],
    ) {
        if self.keys.is_none() {
            return;
        }
        for change in changes {
            if let Some(entry) = state.entries().get(&change.serial) {
                self.track(catalog, entry);
            } else {
                self.candidates.remove(&change.serial);
                self.observed.remove(&change.serial);
            }
        }
        self.evidence.clear();
        self.candidates.retain(|&serial, &mut (parent, id)| {
            let Some(entry) = state.entries().get(&serial) else {
                self.observed.remove(&serial);
                return false;
            };
            let evidence = self.evidence.entry(parent).or_insert_with(|| {
                let pawn = ctx.entities().get_by_handle(parent)?;
                (pawn.class_name.as_ref() == "CCitadelPlayerPawn")
                    .then(|| self.keys.as_ref()?.read(pawn))
                    .flatten()
            });
            let Some(words) = evidence else { return true };
            let present = self.masks[&id]
                .iter()
                .try_fold(false, |present, &(index, bits)| {
                    Some(present || words.get(index)? & bits != 0)
                });
            let application = ModifierApplication::from(entry);
            if observe(&mut self.observed, serial, application, present) {
                state.end_application(serial);
                false
            } else {
                true
            }
        });
    }
}

fn observe(
    observed: &mut HashMap<u32, ModifierApplication>,
    serial: u32,
    application: ModifierApplication,
    present: Option<bool>,
) -> bool {
    if observed.get(&serial).is_some_and(|old| *old != application) {
        observed.remove(&serial);
    }
    match present {
        Some(true) => {
            observed.insert(serial, application);
            false
        }
        // Treat the catalog's declared states as required for this application.
        // Expiry is inferred, not a removal message; overlapping state providers
        // can hide it. Require a present-to-absent transition because a new
        // application's first tick may precede its mask. Missing evidence cannot end it.
        Some(false) => observed.remove(&serial).is_some(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_needs_observed_states_and_tracks_each_application_separately() {
        let first = ModifierApplication::from(&CModifierTableEntry {
            last_applied_time: Some(10.0),
            ..Default::default()
        });
        let refreshed = ModifierApplication::from(&CModifierTableEntry {
            last_applied_time: Some(20.0),
            ..Default::default()
        });
        let mut observed = HashMap::new();
        assert!(!observe(&mut observed, 1, first, Some(false)));
        assert!(!observe(&mut observed, 1, first, Some(true)));
        assert!(!observe(&mut observed, 2, first, Some(true)));
        assert!(!observe(&mut observed, 1, first, None));
        assert!(observe(&mut observed, 1, first, Some(false)));
        assert!(observed.contains_key(&2));
        assert!(!observe(&mut observed, 2, refreshed, Some(false)));
    }
}
