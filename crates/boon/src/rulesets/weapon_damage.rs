//! Global weapon-damage bonus in percentage points, before per-hit effects.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::WeaponDamage,
    id: "weapon_damage.v1",
    version: 1,
    documented_on: "2026-09-28",
};

#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    percent: f64,
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        let total = self.percent + percent;
        if !percent.is_finite() || !total.is_finite() {
            return Err(CalculationError::Invalid(
                "invalid weapon-damage bonus".into(),
            ));
        }
        self.percent = total;
        Ok(())
    }

    pub(crate) fn calculate(self) -> f64 {
        self.percent
    }
}

/// Sum global weapon-damage bonuses in percentage points without rounding.
/// This is the bonus used in a damage multiplier, not damage per projectile.
/// Base damage, flat post-scale damage, crits, falloff, and targets are separate.
///
/// # Errors
/// Rejects nonfinite sources and arithmetic overflow.
pub fn calculate(sources: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    for percent in sources {
        modifiers.add(percent)?;
    }
    Ok(modifiers.calculate())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_signed_percentages_without_multiplying_or_rounding() {
        assert_eq!(calculate([]).unwrap(), 0.0);
        assert_eq!(calculate([20.0, 15.0, 4.0]).unwrap(), 39.0);
        assert_eq!(calculate([12.25, -5.0, 1.5]).unwrap(), 8.75);
        assert_eq!(calculate([130.0]).unwrap(), 130.0);
    }

    #[test]
    fn rejects_nonfinite_sources_and_overflow() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(calculate([value]).is_err());
        }
        assert!(calculate([f64::MAX, f64::MAX]).is_err());
    }
}
