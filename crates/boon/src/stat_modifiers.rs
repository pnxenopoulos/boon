//! Compatibility decoding for permanent stat-viewer modifier values.

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
    /// Multiplier that converts the raw accumulated value into Boon's signed
    /// canonical value. Most entries are additive; explicit resistance
    /// reduction entries use `-1.0`.
    pub value_scale: f32,
}

/// Decode the numeric `EModifierValue` stored in
/// `m_PlayerDataGlobal.m_vecStatViewerModifierValues[*].m_eValType`.
///
/// Demo send tables preserve the enum's numeric discriminant, but not its
/// symbolic member name. These discriminants are not stable wire IDs: Valve
/// renumbered `EModifierValue` between the captured build 10725 and build
/// 10854.
///
/// The first value in each alias pair was observed in build 10725; the second
/// was observed in build 10854:
///
/// | Canonical stat       | 10725 | 10854 |
/// |----------------------|------:|------:|
/// | health               |    31 |    43 |
/// | spirit power         |    51 |   158 |
/// | fire rate            |    79 |    91 |
/// | weapon damage        |    18 |    19 |
/// | cooldown reduction   |   109 |    98 |
/// | ammo                 |   172 |    63 |
///
/// These discriminants are pairwise disjoint, and this controller vector is
/// interpreted in the narrower context of permanent stat-viewer modifiers.
/// Accepting both observed aliases is therefore unambiguous and avoids
/// guessing the enum layout from an individual controller snapshot. Values
/// 32-35 were additionally observed in build 10854 as signed spirit/bullet
/// resistance entries; captured build-10725 vectors do not use them.
///
/// This is a compatibility alias table, not a claim that either numbering is
/// permanently stable. When supporting another build, verify the observed
/// `m_eValType` values. If Valve ever reuses one of these numbers for a
/// different canonical stat, this decoder must become build/layout-aware
/// instead of adding another unconditional alias.
pub const fn decode_stat_modifier_value_type(value_type: u32) -> Option<DecodedStatModifierValue> {
    let (kind, value_scale) = match value_type {
        31 | 43 => (StatModifierKind::Health, 1.0),
        51 | 158 => (StatModifierKind::SpiritPower, 1.0),
        79 | 91 => (StatModifierKind::FireRate, 1.0),
        18 | 19 => (StatModifierKind::WeaponDamage, 1.0),
        109 | 98 => (StatModifierKind::CooldownReduction, 1.0),
        172 | 63 => (StatModifierKind::Ammo, 1.0),
        32 => (StatModifierKind::SpiritResist, 1.0),
        33 => (StatModifierKind::SpiritResist, -1.0),
        34 => (StatModifierKind::BulletResist, 1.0),
        35 => (StatModifierKind::BulletResist, -1.0),
        _ => return None,
    };
    Some(DecodedStatModifierValue { kind, value_scale })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{StatModifierKind, decode_stat_modifier_value_type};

    const OBSERVED_ALIASES: &[(u32, StatModifierKind)] = &[
        (31, StatModifierKind::Health),
        (43, StatModifierKind::Health),
        (51, StatModifierKind::SpiritPower),
        (158, StatModifierKind::SpiritPower),
        (79, StatModifierKind::FireRate),
        (91, StatModifierKind::FireRate),
        (18, StatModifierKind::WeaponDamage),
        (19, StatModifierKind::WeaponDamage),
        (109, StatModifierKind::CooldownReduction),
        (98, StatModifierKind::CooldownReduction),
        (172, StatModifierKind::Ammo),
        (63, StatModifierKind::Ammo),
    ];

    #[test]
    fn maps_observed_10725_and_10854_aliases() {
        for &(value_type, expected) in OBSERVED_ALIASES {
            let decoded = decode_stat_modifier_value_type(value_type).unwrap();
            assert_eq!(decoded.kind, expected, "value type {value_type}");
            assert_eq!(decoded.value_scale, 1.0, "value type {value_type}");
        }
    }

    #[test]
    fn observed_value_types_do_not_collide() {
        let all_values = OBSERVED_ALIASES
            .iter()
            .map(|&(value_type, _)| value_type)
            .chain([32, 33, 34, 35]);
        let values: Vec<_> = all_values.collect();
        let unique: HashSet<_> = values.iter().copied().collect();
        assert_eq!(unique.len(), values.len());
    }

    #[test]
    fn maps_signed_resistance_entries() {
        for (value_type, kind, value_scale) in [
            (32, StatModifierKind::SpiritResist, 1.0),
            (33, StatModifierKind::SpiritResist, -1.0),
            (34, StatModifierKind::BulletResist, 1.0),
            (35, StatModifierKind::BulletResist, -1.0),
        ] {
            let decoded = decode_stat_modifier_value_type(value_type).unwrap();
            assert_eq!(decoded.kind, kind);
            assert_eq!(decoded.value_scale, value_scale);
        }
    }

    #[test]
    fn ignores_unknown_value_types() {
        for value_type in [0, 1, 255, u32::MAX] {
            assert_eq!(decode_stat_modifier_value_type(value_type), None);
        }
    }
}
