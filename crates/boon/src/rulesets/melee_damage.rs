//! Nominal light and heavy melee damage, before target resistance and on-hit effects.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const LIGHT_V1: Rule = Rule {
    stat: HeroStat::LightMeleeDamage,
    id: "light_melee_damage.v1",
    version: 1,
    documented_on: "2026-09-27",
};
pub const HEAVY_V1: Rule = Rule {
    stat: HeroStat::HeavyMeleeDamage,
    id: "heavy_melee_damage.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Add boon growth and spirit scaling, then apply additive melee and half weapon bonuses.
/// `growth` is flat light-melee growth. Heavy growth uses the hero's base damage ratio.
/// Percentages use percentage points. The result is not rounded.
///
/// # Errors
/// Returns an error for nonfinite inputs, invalid base values, negative flat bonuses,
/// a combined damage factor below zero, or overflow.
pub fn calculate(
    base: f64,
    base_light: f64,
    growth: f64,
    spirit: f64,
    weapon_percent: f64,
    melee_percent: f64,
) -> Result<f64, CalculationError> {
    if [
        base,
        base_light,
        growth,
        spirit,
        weapon_percent,
        melee_percent,
    ]
    .iter()
    .any(|v| !v.is_finite())
        || base < 0.0
        || base_light <= 0.0
        || growth < 0.0
        || spirit < 0.0
    {
        return Err(CalculationError::Invalid(
            "invalid melee-damage inputs".into(),
        ));
    }
    // Engine semantics, not balance values: weapon bonuses apply at half strength.
    // Assumption to check in the viewer: heavy boon growth uses the unscaled base
    // heavy/light ratio, and spirit is added before percentage bonuses. VData
    // supplies these inputs but does not encode their order of operations.
    let factor = 1.0 + (weapon_percent * 0.5 + melee_percent) / 100.0;
    let value = (base + growth * (base / base_light) + spirit) * factor;
    if !value.is_finite() || factor < 0.0 {
        return Err(CalculationError::Invalid(
            "invalid melee-damage result".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_base_ratio_and_additive_bonuses_without_rounding() {
        // Deliberately use a nonstandard ratio and spirit value.
        assert!((calculate(40.0, 40.0, 6.0, 0.0, 50.0, 12.0).unwrap() - 63.02).abs() < 1e-10);
        assert!((calculate(100.0, 40.0, 6.0, 9.0, 50.0, 12.0).unwrap() - 169.88).abs() < 1e-10);
        assert_eq!(calculate(100.0, 40.0, 0.0, 0.0, 0.0, 0.0).unwrap(), 100.0);
        assert_eq!(calculate(100.0, 40.0, 0.0, 0.0, -20.0, 0.0).unwrap(), 90.0);
    }

    #[test]
    fn rejects_missing_bases_nonfinite_values_and_overflow() {
        for args in [
            [100.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            [-1.0, 40.0, 0.0, 0.0, 0.0, 0.0],
            [100.0, 40.0, -1.0, 0.0, 0.0, 0.0],
            [100.0, 40.0, 1.0, -1.0, 0.0, 0.0],
            [100.0, 40.0, 1.0, 0.0, f64::NAN, 0.0],
            [100.0, 40.0, 1.0, 0.0, 0.0, -101.0],
            [f64::MAX, 1.0, f64::MAX, 0.0, 0.0, 0.0],
        ] {
            let [base, light, growth, spirit, weapon, melee] = args;
            assert!(calculate(base, light, growth, spirit, weapon, melee).is_err());
        }
    }
}
