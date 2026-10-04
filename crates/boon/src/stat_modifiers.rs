//! Catalog-backed decoding for recorded stat-viewer modifier values.

use std::{collections::HashMap, fs, path::Path};

use serde::Deserialize;

use crate::data::{DataError, Result};

/// Canonical stat represented by a player controller's stat-viewer vector.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum StatModifierKind {
    Health,
    SpiritPower,
    FireRate,
    WeaponDamage,
    CooldownReduction,
    Ammo,
    BulletResist,
    SpiritResist,
}

impl StatModifierKind {
    pub const COUNT: usize = 8;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Health,
        Self::SpiritPower,
        Self::FireRate,
        Self::WeaponDamage,
        Self::CooldownReduction,
        Self::Ammo,
        Self::BulletResist,
        Self::SpiritResist,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::SpiritPower => "spirit_power",
            Self::FireRate => "fire_rate",
            Self::WeaponDamage => "weapon_damage",
            Self::CooldownReduction => "cooldown_reduction",
            Self::Ammo => "ammo",
            Self::BulletResist => "bullet_resist",
            Self::SpiritResist => "spirit_resist",
        }
    }
}

/// Canonical meaning of one raw `EModifierValue` entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodedStatModifierValue {
    pub kind: StatModifierKind,
    /// Multiplier that converts the raw value to Boon's signed value.
    /// Most entries add a value.
    /// Resistance reduction entries use `-1.0`.
    pub value_scale: f32,
}

/// Stat-viewer enum IDs from one boon-data version. Reuse across ticks.
#[derive(Debug)]
pub struct StatModifierTypes(HashMap<u32, DecodedStatModifierValue>);

impl StatModifierTypes {
    /// Read an installed catalog, downloading it if needed.
    ///
    /// # Errors
    /// Returns an error for unavailable versions or invalid catalogs.
    pub fn load(version: Option<&str>) -> Result<Self> {
        Self::from_directory(&crate::data::catalog_dir(version)?)
    }

    /// Read enum definitions from a local abilities.json.
    ///
    /// # Errors
    /// Returns an error if the file or its enum definitions are missing or invalid.
    pub fn from_directory(directory: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        struct Catalog {
            modifier_value_types: HashMap<u32, String>,
        }
        let catalog: Catalog = serde_json::from_slice(&fs::read(directory.join("abilities.json"))?)
            .map_err(|error| DataError::Invalid(format!(
                "abilities.json lacks modifier enum definitions: {error}; use `boon get VERSION --force`"
            )))?;
        if catalog.modifier_value_types.is_empty() {
            return Err(DataError::Invalid(
                "abilities.json has no modifier enum definitions; use `boon get VERSION --force`"
                    .into(),
            ));
        }
        Ok(Self(
            catalog
                .modifier_value_types
                .iter()
                .filter_map(|(&id, name)| {
                    let (kind, value_scale) = match name.as_str() {
                        "MODIFIER_VALUE_HEALTH_MAX" => (StatModifierKind::Health, 1.0),
                        "MODIFIER_VALUE_TECH_POWER" => (StatModifierKind::SpiritPower, 1.0),
                        "MODIFIER_VALUE_FIRE_RATE" => (StatModifierKind::FireRate, 1.0),
                        "MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE" => {
                            (StatModifierKind::WeaponDamage, 1.0)
                        }
                        "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE" => {
                            (StatModifierKind::CooldownReduction, 1.0)
                        }
                        "MODIFIER_VALUE_AMMO_CLIP_SIZE" => (StatModifierKind::Ammo, 1.0),
                        "MODIFIER_VALUE_TECH_RESIST" => (StatModifierKind::SpiritResist, 1.0),
                        "MODIFIER_VALUE_TECH_RESIST_REDUCTION" => {
                            (StatModifierKind::SpiritResist, -1.0)
                        }
                        "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST" => {
                            (StatModifierKind::BulletResist, 1.0)
                        }
                        "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION" => {
                            (StatModifierKind::BulletResist, -1.0)
                        }
                        _ => return None,
                    };
                    Some((id, DecodedStatModifierValue { kind, value_scale }))
                })
                .collect(),
        ))
    }

    /// Resolve one recorded numeric type using this catalog's enum definitions.
    pub fn decode(&self, value_type: u32) -> Option<DecodedStatModifierValue> {
        self.0.get(&value_type).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_catalog_ids_and_does_not_guess_unknown_types() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("abilities.json"),
            r#"{
            "modifier_value_types": {
                "900": "MODIFIER_VALUE_TECH_POWER",
                "51": "MODIFIER_VALUE_HEALTH_REGEN_PER_SECOND",
                "901": "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION"
            }
        }"#,
        )
        .unwrap();
        let types = StatModifierTypes::from_directory(folder.path()).unwrap();
        assert_eq!(
            types.decode(900).unwrap().kind,
            StatModifierKind::SpiritPower
        );
        assert_eq!(types.decode(901).unwrap().value_scale, -1.0);
        assert_eq!(types.decode(51), None);
        assert_eq!(types.decode(158), None);
    }

    #[test]
    fn requires_enum_definitions() {
        let folder = tempfile::tempdir().unwrap();
        for content in [r#"{}"#, r#"{"modifier_value_types":{}}"#] {
            fs::write(folder.path().join("abilities.json"), content).unwrap();
            assert!(
                StatModifierTypes::from_directory(folder.path())
                    .unwrap_err()
                    .to_string()
                    .contains("boon get")
            );
        }
    }
}
