use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::murmur_hash2;

#[derive(Clone, Debug, Default)]
struct Property {
    value: f32,
    property_type: String,
    flags: String,
    spirit_scale: f32,
    complete: bool,
}

#[derive(Clone, Debug, Default)]
struct Upgrade {
    property: String,
    value: f32,
    scale: f32,
    complete: bool,
}

#[derive(Clone, Debug, Default)]
struct Modifier {
    class_name: String,
    subclass_name: String,
    properties: Vec<String>,
}

#[derive(Clone, Debug)]
struct Ability {
    name: String,
    is_item: bool,
    properties: HashMap<String, Property>,
    upgrades: [Vec<Upgrade>; 3],
    modifiers: Vec<Modifier>,
}

impl Ability {
    fn new(name: String) -> Self {
        Self {
            name,
            is_item: false,
            properties: HashMap::new(),
            upgrades: std::array::from_fn(|_| Vec::new()),
            modifiers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Scope {
    name: Option<String>,
    class_name: Option<String>,
    subclass_name: Option<String>,
    value: Option<f32>,
    property_type: Option<String>,
    flags: Option<String>,
    spirit_scale: Option<f32>,
    scaling_stat: Option<String>,
    property_name: Option<String>,
    bonus: Option<f32>,
    upgrade_type: Option<String>,
    scale_filter: Option<String>,
    auto_properties: Vec<String>,
    tier: Option<usize>,
    complete: bool,
}

#[derive(Clone, Debug)]
struct ArrayScope {
    name: String,
    depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GeneratedEffect {
    stat: &'static str,
    operation: &'static str,
    value_bits: u32,
    scale_bits: u32,
    upgrade_values: [u32; 3],
    upgrade_scales: [u32; 3],
    complete: bool,
}

impl GeneratedEffect {
    fn value(self) -> f32 {
        f32::from_bits(self.value_bits)
    }

    fn scale(self) -> f32 {
        f32::from_bits(self.scale_bits)
    }
}

fn assignment(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim().trim_matches('"'), value.trim()))
}

fn parse_number(value: &str) -> Option<f32> {
    let value = value.trim().trim_matches('"');
    let end = value
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()?;
    value[..end].parse().ok()
}

fn stat_mapping(property_type: &str) -> Option<(&'static str, &'static str)> {
    match property_type {
        "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST" => {
            Some(("BulletResist", "Resistance"))
        }
        "MODIFIER_VALUE_TECH_RESIST" => Some(("SpiritResist", "Resistance")),
        "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION" => {
            Some(("BulletResist", "Reduction"))
        }
        "MODIFIER_VALUE_TECH_RESIST_REDUCTION" => {
            Some(("SpiritResist", "Reduction"))
        }
        "MODIFIER_VALUE_TECH_POWER" => Some(("SpiritPower", "Add")),
        "MODIFIER_VALUE_FIRE_RATE" => Some(("FireRateBonus", "Add")),
        "MODIFIER_VALUE_FIRE_RATE_SLOW" => Some(("FireRateBonus", "Reduction")),
        "MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE"
        | "MODIFIER_VALUE_BULLET_DAMAGE_INCREASE" => {
            Some(("WeaponDamageBonus", "Add"))
        }
        "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE" => {
            Some(("CooldownReduction", "Add"))
        }
        "MODIFIER_VALUE_STATUS_RESISTANCE" => Some(("StatusResist", "Add")),
        "MODIFIER_VALUE_BULLET_LIFESTEAL" => Some(("BulletLifesteal", "Add")),
        "MODIFIER_VALUE_TECH_LIFESTEAL" => Some(("SpiritLifesteal", "Add")),
        _ => None,
    }
}

fn effect_for(ability: &Ability, property_name: &str) -> Option<GeneratedEffect> {
    let property = ability.properties.get(property_name)?;
    let (stat, operation) = stat_mapping(&property.property_type)?;
    let mut values = [0.0f32; 3];
    let mut scales = [0.0f32; 3];
    let mut complete = property.complete;

    for (tier, upgrades) in ability.upgrades.iter().enumerate() {
        for upgrade in upgrades
            .iter()
            .filter(|upgrade| upgrade.property == property_name)
        {
            values[tier] += upgrade.value;
            scales[tier] += upgrade.scale;
            complete &= upgrade.complete;
        }
    }

    let effect = GeneratedEffect {
        stat,
        operation,
        value_bits: property.value.to_bits(),
        scale_bits: property.spirit_scale.to_bits(),
        upgrade_values: values.map(f32::to_bits),
        upgrade_scales: scales.map(f32::to_bits),
        complete,
    };
    (effect.value() != 0.0
        || effect.scale() != 0.0
        || values.iter().any(|value| *value != 0.0)
        || scales.iter().any(|value| *value != 0.0))
    .then_some(effect)
}

fn parse_abilities(content: &str) -> Vec<Ability> {
    let mut scopes: Vec<Scope> = Vec::new();
    let mut arrays: Vec<ArrayScope> = Vec::new();
    let mut pending: Option<String> = None;
    let mut ability: Option<Ability> = None;
    let mut abilities = Vec::new();
    let mut next_tier = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[" {
            if let Some(name) = pending.take() {
                arrays.push(ArrayScope {
                    name,
                    depth: scopes.len(),
                });
            }
            continue;
        }
        if matches!(trimmed, "]" | "],") {
            arrays.pop();
            continue;
        }
        if trimmed == "{" {
            let name = pending.take().or_else(|| {
                arrays
                    .last()
                    .filter(|array| array.depth == scopes.len())
                    .map(|array| array.name.clone())
            });
            if scopes.len() == 1 {
                if let Some(name) = name.clone() {
                    ability = Some(Ability::new(name));
                    next_tier = 0;
                }
            }
            let inherited_tier = scopes.last().and_then(|scope| scope.tier);
            let tier = if name.as_deref() == Some("m_vecAbilityUpgrades") {
                let tier = next_tier.min(2);
                next_tier += 1;
                Some(tier)
            } else {
                inherited_tier
            };
            scopes.push(Scope {
                name,
                tier,
                complete: true,
                ..Default::default()
            });
            continue;
        }
        if matches!(trimmed, "}" | "},") {
            let Some(mut scope) = scopes.pop() else {
                continue;
            };

            if scopes
                .last()
                .and_then(|scope| scope.name.as_deref())
                == Some("m_mapAbilityProperties")
            {
                if let (Some(current), Some(name), Some(property_type)) = (
                    ability.as_mut(),
                    scope.name.take(),
                    scope.property_type.take(),
                ) {
                    current.properties.insert(
                        name,
                        Property {
                            value: scope.value.unwrap_or(0.0),
                            property_type,
                            flags: scope.flags.unwrap_or_default(),
                            spirit_scale: scope.spirit_scale.unwrap_or(0.0),
                            complete: scope.complete,
                        },
                    );
                }
            } else if scope.name.as_deref() == Some("m_vecPropertyUpgrades") {
                if let (Some(current), Some(tier), Some(property)) =
                    (ability.as_mut(), scope.tier, scope.property_name)
                {
                    let add_to_scale =
                        scope.upgrade_type.as_deref() == Some("EAddToScale");
                    let scale_supported = !add_to_scale
                        || scope.scale_filter.as_deref() == Some("ETechPower");
                    current.upgrades[tier].push(Upgrade {
                        property,
                        value: if add_to_scale {
                            0.0
                        } else {
                            scope.bonus.unwrap_or(0.0)
                        },
                        scale: if add_to_scale {
                            scope.bonus.unwrap_or(0.0)
                        } else {
                            0.0
                        },
                        complete: scale_supported,
                    });
                }
            }

            if scope
                .class_name
                .as_deref()
                .is_some_and(|class| class.starts_with("modifier_"))
                && !scope.auto_properties.is_empty()
            {
                if let Some(current) = ability.as_mut() {
                    current.modifiers.push(Modifier {
                        class_name: scope.class_name.clone().unwrap_or_default(),
                        subclass_name: scope.subclass_name.clone().unwrap_or_default(),
                        properties: scope.auto_properties.clone(),
                    });
                }
            }

            if let Some(parent) = scopes.last_mut() {
                if scope.spirit_scale.is_some() {
                    parent.spirit_scale = scope.spirit_scale;
                    let supported = scope.scaling_stat.as_deref().is_none_or(|stat| {
                        matches!(stat, "ETechPower" | "MODIFIER_VALUE_TECH_POWER")
                    });
                    parent.complete &= supported;
                }
            }

            if scopes.len() == 1 {
                if let Some(current) = ability.take() {
                    abilities.push(current);
                }
            }
            continue;
        }

        if let Some(array) = arrays.last()
            && array.name == "m_vecAutoRegisterModifierValueFromAbilityPropertyName"
            && !trimmed.contains('=')
        {
            let property = trimmed.trim_end_matches(',').trim_matches('"');
            if !property.is_empty() {
                if let Some(scope) = scopes.iter_mut().rev().find(|scope| {
                    scope
                        .class_name
                        .as_deref()
                        .is_some_and(|class| class.starts_with("modifier_"))
                }) {
                    scope.auto_properties.push(property.to_string());
                }
            }
            continue;
        }

        let Some((key, value)) = assignment(trimmed) else {
            continue;
        };
        if value.is_empty() {
            pending = Some(key.to_string());
            continue;
        }
        if value.ends_with(':') {
            pending = Some(key.to_string());
        }

        if scopes.len() == 2 && key == "m_eAbilityType" {
            if let Some(current) = ability.as_mut() {
                current.is_item = value.trim_matches('"') == "EAbilityType_Item";
            }
        }

        if let Some(scope) = scopes.last_mut() {
            match key {
                "_class" => scope.class_name = Some(value.trim_matches('"').to_string()),
                "_my_subclass_name" => {
                    scope.subclass_name = Some(value.trim_matches('"').to_string())
                }
                "m_strValue" => scope.value = parse_number(value),
                "m_eProvidedPropertyType" => {
                    scope.property_type = Some(value.trim_matches('"').to_string())
                }
                "m_eStatsUsageFlags" => {
                    scope.flags = Some(value.trim_matches('"').to_string())
                }
                "m_flStatScale" | "flScale" => scope.spirit_scale = parse_number(value),
                "m_eSpecificStatScaleType" | "eScalingStat" => {
                    scope.scaling_stat = Some(value.trim_matches('"').to_string())
                }
                "m_strPropertyName" => {
                    scope.property_name = Some(value.trim_matches('"').to_string())
                }
                "m_strBonus" => scope.bonus = parse_number(value),
                "m_eUpgradeType" => {
                    scope.upgrade_type = Some(value.trim_matches('"').to_string())
                }
                "m_eScaleStatFilter" => {
                    scope.scale_filter = Some(value.trim_matches('"').to_string())
                }
                _ => {}
            }
        }
    }
    abilities
}

fn write_effect(out: &mut String, effect: GeneratedEffect) {
    let values = effect.upgrade_values.map(f32::from_bits);
    let scales = effect.upgrade_scales.map(f32::from_bits);
    writeln!(
        out,
        "            StatEffect {{ stat: StatId::{}, operation: StatOperation::{}, base_value: {:?}, spirit_power_scale: {:?}, upgrade_values: {:?}, upgrade_scales: {:?}, complete: {} }},",
        effect.stat,
        effect.operation,
        effect.value(),
        effect.scale(),
        values,
        scales,
        effect.complete,
    )
    .unwrap();
}

pub fn generate(content: &str, output: &Path, today: &str) {
    let abilities = parse_abilities(content);
    let mut items: BTreeMap<u32, Vec<GeneratedEffect>> = BTreeMap::new();
    let mut modifiers: BTreeMap<(u32, u32), Vec<GeneratedEffect>> = BTreeMap::new();

    for ability in &abilities {
        let ability_id = murmur_hash2(ability.name.as_bytes());
        if ability.is_item {
            for (name, property) in &ability.properties {
                if property.flags.contains("ConditionallyApplied") {
                    continue;
                }
                if let Some(effect) = effect_for(ability, name) {
                    items.entry(ability_id).or_default().push(effect);
                }
            }
        }

        for modifier in &ability.modifiers {
            let mut effects: Vec<_> = modifier
                .properties
                .iter()
                .filter_map(|property| effect_for(ability, property))
                .collect();
            effects.sort();
            effects.dedup();
            if effects.is_empty() {
                continue;
            }
            let mut ids = vec![murmur_hash2(modifier.class_name.as_bytes())];
            if !modifier.subclass_name.is_empty() {
                ids.push(murmur_hash2(modifier.subclass_name.as_bytes()));
            }
            ids.sort_unstable();
            ids.dedup();
            for modifier_id in ids {
                modifiers
                    .entry((ability_id, modifier_id))
                    .or_default()
                    .extend(effects.iter().copied());
            }
        }
    }

    for effects in items.values_mut() {
        effects.sort();
        effects.dedup();
    }
    for effects in modifiers.values_mut() {
        effects.sort();
        effects.dedup();
    }

    // A concrete modifier token can change between game builds. Falling back
    // by ability is safe only when every stat-bearing modifier on that ability
    // registers the exact same effects.
    let mut fallback: BTreeMap<u32, Option<Vec<GeneratedEffect>>> = BTreeMap::new();
    for ((ability_id, _), effects) in &modifiers {
        fallback
            .entry(*ability_id)
            .and_modify(|candidate| {
                if candidate.as_ref() != Some(effects) {
                    *candidate = None;
                }
            })
            .or_insert_with(|| Some(effects.clone()));
    }
    let mut out = String::new();
    writeln!(
        out,
        "//! Auto-generated from Deadlock abilities.vdata.\n//!\n//! Last updated: {today}\n"
    )
    .unwrap();
    writeln!(out, "use crate::stats::{{StatEffect, StatId, StatOperation}};\n").unwrap();
    writeln!(
        out,
        "pub fn item_stat_effects(ability_id: u32) -> &'static [StatEffect] {{\n    match ability_id {{"
    )
    .unwrap();
    for (ability_id, effects) in &items {
        writeln!(out, "        {ability_id} => &[").unwrap();
        for &effect in effects {
            write_effect(&mut out, effect);
        }
        writeln!(out, "        ],").unwrap();
    }
    writeln!(out, "        _ => &[],\n    }}\n}}\n").unwrap();

    writeln!(
        out,
        "pub fn modifier_stat_effects(ability_id: u32, modifier_id: u32) -> &'static [StatEffect] {{\n    match (ability_id, modifier_id) {{"
    )
    .unwrap();
    for ((ability_id, modifier_id), effects) in &modifiers {
        writeln!(out, "        ({ability_id}, {modifier_id}) => &[").unwrap();
        for &effect in effects {
            write_effect(&mut out, effect);
        }
        writeln!(out, "        ],").unwrap();
    }
    writeln!(out, "        _ => match ability_id {{").unwrap();
    for (ability_id, effects) in fallback {
        if let Some(effects) = effects {
            writeln!(out, "            {ability_id} => &[").unwrap();
            for effect in effects {
                write_effect(&mut out, effect);
            }
            writeln!(out, "            ],").unwrap();
        }
    }
    writeln!(out, "            _ => &[],\n        }},\n    }}\n}}").unwrap();

    fs::write(output, out).expect("failed to write generated stat catalog");
    eprintln!(
        "Wrote {} with {} item and {} modifier effect groups",
        output.display(),
        items.len(),
        modifiers.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_item_and_modifier_effects() {
        let data = r#"
root =
{
    item_test =
    {
        m_eAbilityType = "EAbilityType_Item"
        m_mapAbilityProperties =
        {
            Resist =
            {
                m_strValue = "10"
                m_eProvidedPropertyType = "MODIFIER_VALUE_TECH_RESIST"
            }
        }
        m_Buff = subclass:
        {
            _class = "modifier_test"
            _my_subclass_name = "test"
            m_vecAutoRegisterModifierValueFromAbilityPropertyName =
            [
                "Resist",
            ]
        }
    }
}
"#;
        let abilities = parse_abilities(data);
        assert_eq!(abilities.len(), 1);
        let effect = effect_for(&abilities[0], "Resist").unwrap();
        assert_eq!(effect.stat, "SpiritResist");
        assert_eq!(effect.value(), 10.0);
        assert_eq!(abilities[0].modifiers.len(), 1);
    }
}
