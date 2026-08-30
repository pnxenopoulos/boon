//! Native, persistent baseline, and tick-effective player stats.

use std::collections::HashMap;
use std::ops::{Index, IndexMut};

use boon_proto::proto::CModifierTableEntry;

use crate::{hero_resistance_stats, stat_catalog};

pub const STAT_COUNT: usize = 9;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StatId {
    BulletResist,
    SpiritResist,
    SpiritPower,
    FireRateBonus,
    WeaponDamageBonus,
    CooldownReduction,
    StatusResist,
    BulletLifesteal,
    SpiritLifesteal,
}

impl StatId {
    pub const ALL: [Self; STAT_COUNT] = [
        Self::BulletResist,
        Self::SpiritResist,
        Self::SpiritPower,
        Self::FireRateBonus,
        Self::WeaponDamageBonus,
        Self::CooldownReduction,
        Self::StatusResist,
        Self::BulletLifesteal,
        Self::SpiritLifesteal,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::BulletResist => "bullet_resist",
            Self::SpiritResist => "spirit_resist",
            Self::SpiritPower => "spirit_power",
            Self::FireRateBonus => "fire_rate_bonus",
            Self::WeaponDamageBonus => "weapon_damage_bonus",
            Self::CooldownReduction => "cooldown_reduction",
            Self::StatusResist => "status_resist",
            Self::BulletLifesteal => "bullet_lifesteal",
            Self::SpiritLifesteal => "spirit_lifesteal",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stat| stat.name() == name)
    }

    const fn bit(self) -> u16 {
        1 << self as u8
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StatMask(u16);

impl StatMask {
    pub const ALL: Self = Self((1 << STAT_COUNT) - 1);

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, stat: StatId) -> bool {
        self.0 & stat.bit() != 0
    }

    pub fn insert(&mut self, stat: StatId) {
        self.0 |= stat.bit();
    }

    pub fn remove(&mut self, stat: StatId) {
        self.0 &= !stat.bit();
    }

    pub fn iter(self) -> impl Iterator<Item = StatId> {
        StatId::ALL
            .into_iter()
            .filter(move |stat| self.contains(*stat))
    }
}

impl FromIterator<StatId> for StatMask {
    fn from_iter<T: IntoIterator<Item = StatId>>(iter: T) -> Self {
        let mut mask = Self::default();
        for stat in iter {
            mask.insert(stat);
        }
        mask
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatBlock {
    values: [f32; STAT_COUNT],
}

impl StatBlock {
    pub const fn values(&self) -> &[f32; STAT_COUNT] {
        &self.values
    }
}

impl Index<StatId> for StatBlock {
    type Output = f32;

    fn index(&self, stat: StatId) -> &Self::Output {
        &self.values[stat as usize]
    }
}

impl IndexMut<StatId> for StatBlock {
    fn index_mut(&mut self, stat: StatId) -> &mut Self::Output {
        &mut self.values[stat as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatOperation {
    Add,
    Resistance,
    Reduction,
}

impl StatOperation {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Resistance => "resistance",
            Self::Reduction => "reduction",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatEffect {
    pub stat: StatId,
    pub operation: StatOperation,
    pub base_value: f32,
    pub spirit_power_scale: f32,
    pub upgrade_values: [f32; 3],
    pub upgrade_scales: [f32; 3],
    pub complete: bool,
}

impl StatEffect {
    pub fn resolve(self, spirit_power: f32, ability_tier: u8) -> (f32, bool) {
        let tiers = usize::from(ability_tier.min(3));
        let value = self.base_value + self.upgrade_values[..tiers].iter().sum::<f32>();
        let scale = self.spirit_power_scale + self.upgrade_scales[..tiers].iter().sum::<f32>();
        (value + spirit_power * scale, self.complete)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatLayers {
    pub native: StatBlock,
    pub baseline: StatBlock,
    pub effective: StatBlock,
    pub complete: StatMask,
}

pub fn combine_resistance(current: f32, source: f32) -> f32 {
    100.0 - (100.0 - current) * (100.0 - source) / 100.0
}

fn apply(block: &mut StatBlock, effect: StatEffect, value: f32) {
    if !value.is_finite() {
        return;
    }
    match effect.operation {
        StatOperation::Add => block[effect.stat] += value,
        StatOperation::Resistance => {
            block[effect.stat] = combine_resistance(block[effect.stat], value);
        }
        StatOperation::Reduction => block[effect.stat] -= value,
    }
}

fn spirit_power(hero_id: i64, level: i64) -> f32 {
    let hero = hero_resistance_stats(hero_id);
    hero.base_spirit_power + level.saturating_sub(1) as f32 * hero.spirit_power_per_level
}

fn native_with_spirit(hero_id: i64, level: i64, spirit: f32) -> StatBlock {
    let hero = hero_resistance_stats(hero_id);
    let level_ups = level.saturating_sub(1) as f32;
    let mut block = StatBlock::default();
    block[StatId::SpiritPower] = spirit;
    block[StatId::BulletResist] = combine_resistance(
        0.0,
        hero.base_bullet_resist
            + level_ups * hero.bullet_resist_per_level
            + spirit * hero.bullet_resist_per_spirit_power,
    );
    block[StatId::SpiritResist] = combine_resistance(
        0.0,
        hero.base_spirit_resist
            + level_ups * hero.spirit_resist_per_level
            + spirit * hero.spirit_resist_per_spirit_power,
    );
    block
}

fn apply_item_effects(
    block: &mut StatBlock,
    upgrades: &[u32],
    spirit: f32,
    skip_spirit: bool,
    complete: &mut StatMask,
) {
    for &upgrade in upgrades {
        for &effect in stat_catalog::item_stat_effects(upgrade) {
            if skip_spirit && effect.stat == StatId::SpiritPower {
                continue;
            }
            let (value, ok) = effect.resolve(spirit, 0);
            apply(block, effect, value);
            if !ok {
                complete.remove(effect.stat);
            }
        }
    }
}

pub fn evaluate_player_stats<'a>(
    hero_id: i64,
    level: i64,
    upgrades: &[u32],
    ability_tiers: &HashMap<u32, u8>,
    active_modifiers: impl IntoIterator<Item = &'a CModifierTableEntry>,
) -> StatLayers {
    let modifiers: Vec<_> = active_modifiers
        .into_iter()
        .filter(|entry| entry.in_aura_range != Some(false))
        .collect();
    let mut complete = StatMask::ALL;
    let native_spirit = spirit_power(hero_id, level);
    let native = native_with_spirit(hero_id, level, native_spirit);

    let mut baseline_spirit = native_spirit;
    for &upgrade in upgrades {
        for &effect in stat_catalog::item_stat_effects(upgrade) {
            if effect.stat == StatId::SpiritPower {
                let (value, ok) = effect.resolve(baseline_spirit, 0);
                baseline_spirit += value;
                if !ok {
                    complete.remove(effect.stat);
                }
            }
        }
    }

    // Persistent item modifiers repeat unconditional properties that were
    // already applied from the purchased-upgrade catalog. Other permanent
    // modifiers are baseline sources; finite-duration modifiers are temporary.
    let persistent_modifiers = modifiers.iter().copied().filter(|entry| {
        entry.duration.unwrap_or(-1.0) < 0.0
            && !upgrades.contains(&entry.ability_subclass.unwrap_or(0))
    });
    let temporary = modifiers
        .iter()
        .copied()
        .filter(|entry| entry.duration.unwrap_or(-1.0) >= 0.0);

    let mut persistent_effects = Vec::new();
    for entry in persistent_modifiers {
        let ability = entry.ability_subclass.unwrap_or(0);
        let modifier = entry.modifier_subclass.unwrap_or(0);
        let tier = ability_tiers.get(&ability).copied().unwrap_or(0);
        for &effect in stat_catalog::modifier_stat_effects(ability, modifier) {
            persistent_effects.push((entry, effect, tier));
        }
    }
    let mut temporary_effects = Vec::new();
    for entry in temporary {
        let ability = entry.ability_subclass.unwrap_or(0);
        let modifier = entry.modifier_subclass.unwrap_or(0);
        let tier = ability_tiers.get(&ability).copied().unwrap_or(0);
        for &effect in stat_catalog::modifier_stat_effects(ability, modifier) {
            temporary_effects.push((entry, effect, tier));
        }
    }

    for &(entry, effect, tier) in &persistent_effects {
        if effect.stat == StatId::SpiritPower {
            let (value, ok) = effect.resolve(baseline_spirit, tier);
            baseline_spirit += value;
            if !ok || entry.stack_count.unwrap_or(0) > 1 {
                complete.remove(effect.stat);
            }
        }
    }

    let mut effective_spirit = baseline_spirit;
    for &(entry, effect, tier) in &temporary_effects {
        if effect.stat == StatId::SpiritPower {
            let (value, ok) = effect.resolve(effective_spirit, tier);
            // VData does not provide one universal stack rule. Apply one copy
            // and expose uncertainty instead of assuming linear scaling.
            effective_spirit += value;
            if !ok || entry.stack_count.unwrap_or(0) > 1 {
                complete.remove(effect.stat);
            }
        }
    }

    let mut baseline = native_with_spirit(hero_id, level, baseline_spirit);
    apply_item_effects(
        &mut baseline,
        upgrades,
        baseline_spirit,
        true,
        &mut complete,
    );
    for &(entry, effect, tier) in &persistent_effects {
        if effect.stat == StatId::SpiritPower {
            continue;
        }
        let (value, ok) = effect.resolve(baseline_spirit, tier);
        apply(&mut baseline, effect, value);
        if !ok || entry.stack_count.unwrap_or(0) > 1 {
            complete.remove(effect.stat);
        }
    }

    let mut effective = baseline;
    effective[StatId::SpiritPower] = effective_spirit;

    for &(entry, effect, tier) in &temporary_effects {
        if effect.stat == StatId::SpiritPower {
            continue;
        }
        let (value, ok) = effect.resolve(effective_spirit, tier);
        apply(&mut effective, effect, value);
        if !ok || entry.stack_count.unwrap_or(0) > 1 {
            complete.remove(effect.stat);
        }
    }

    StatLayers {
        native,
        baseline,
        effective,
        complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resistance_sources_compose_multiplicatively() {
        assert!((combine_resistance(18.0, 9.0) - 25.38).abs() < 1e-4);
        assert!((combine_resistance(10.0, 10.0) - 19.0).abs() < 1e-4);
    }

    #[test]
    fn parses_public_names() {
        for stat in StatId::ALL {
            assert_eq!(StatId::from_name(stat.name()), Some(stat));
        }
        assert_eq!(StatId::from_name("damage"), None);
    }

    fn plot_armor(stacks: i32, in_aura_range: Option<bool>) -> CModifierTableEntry {
        CModifierTableEntry {
            ability_subclass: Some(3_553_292_912),
            // This token comes from the older real-demo fixture and exercises
            // the guarded ability-level fallback generated for cross-build IDs.
            modifier_subclass: Some(1_397_769_555),
            duration: Some(5.0),
            stack_count: Some(stacks),
            in_aura_range,
            ..Default::default()
        }
    }

    #[test]
    fn temporary_effect_only_changes_effective_layer() {
        let modifier = plot_armor(1, None);
        let layers = evaluate_player_stats(67, 1, &[], &HashMap::new(), std::iter::once(&modifier));
        assert_eq!(layers.baseline[StatId::WeaponDamageBonus], 0.0);
        assert_eq!(layers.effective[StatId::WeaponDamageBonus], 25.0);
        assert!(layers.complete.contains(StatId::WeaponDamageBonus));
    }

    #[test]
    fn unknown_stack_rule_is_not_multiplied_and_is_incomplete() {
        let modifier = plot_armor(2, None);
        let layers = evaluate_player_stats(67, 1, &[], &HashMap::new(), std::iter::once(&modifier));
        assert_eq!(layers.effective[StatId::WeaponDamageBonus], 25.0);
        assert!(!layers.complete.contains(StatId::WeaponDamageBonus));
    }

    #[test]
    fn out_of_range_aura_does_not_apply() {
        let modifier = plot_armor(1, Some(false));
        let layers = evaluate_player_stats(67, 1, &[], &HashMap::new(), std::iter::once(&modifier));
        assert_eq!(layers.effective[StatId::WeaponDamageBonus], 0.0);
    }

    #[test]
    fn persistent_item_modifier_is_not_counted_twice() {
        let modifier = CModifierTableEntry {
            ability_subclass: Some(1_235_347_618),
            modifier_subclass: Some(2_312_238_751),
            duration: Some(-1.0),
            ..Default::default()
        };
        let layers = evaluate_player_stats(
            0,
            1,
            &[1_235_347_618],
            &HashMap::new(),
            std::iter::once(&modifier),
        );
        assert!((layers.baseline[StatId::BulletResist] - 18.0).abs() < 1e-4);
        assert_eq!(
            layers.baseline[StatId::BulletResist],
            layers.effective[StatId::BulletResist]
        );
    }
}
