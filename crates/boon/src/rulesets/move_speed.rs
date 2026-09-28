//! Nominal movement speed with diminishing flat bonuses, in metres per second.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::MoveSpeed,
    id: "move_speed.v1",
    version: 1,
    documented_on: "2026-09-28",
};

// The normalization constant belongs to the supplied equation, not hero or item
// balance data. A change to this equation requires a new ruleset version.
const BONUS_REFERENCE_SPEED: f64 = 12.0;

#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    effective: f64,
    count: usize,
    negative: bool,
}

impl Modifiers {
    pub(crate) fn add(&mut self, bonus: f64) -> Result<(), CalculationError> {
        if !bonus.is_finite() || bonus > BONUS_REFERENCE_SPEED {
            return Err(CalculationError::Invalid(
                "unsupported move-speed bonus".into(),
            ));
        }
        if bonus == 0.0 {
            return Ok(());
        }
        // The supplied diminishing-return rule describes bonuses. It does not
        // establish how flat penalties interact with other movement sources.
        if self.count > 0 && (self.negative || bonus < 0.0) {
            return Err(CalculationError::Invalid("combining move-speed penalties with other flat adjustments is not supported by move_speed.v1".into()));
        }
        self.effective += bonus * (1.0 - self.effective / BONUS_REFERENCE_SPEED);
        self.count += 1;
        self.negative |= bonus < 0.0;
        Ok(())
    }

    pub(crate) fn calculate(self, base: f64, percent: f64) -> Result<f64, CalculationError> {
        let speed = (base + self.effective) * (1.0 + percent / 100.0);
        if !base.is_finite()
            || base < 0.0
            || !percent.is_finite()
            || percent < -100.0
            || !speed.is_finite()
            || speed < 0.0
        {
            return Err(CalculationError::Invalid(
                "invalid move-speed inputs".into(),
            ));
        }
        Ok(speed)
    }
}

/// Return `(base + 12 * (1 - product(1 - bonus / 12))) * (1 + percent / 100)`.
/// The percentage applies only to movement speed, not the extra sprint component.
/// No firing, crouching, slow, speed-limit or sprint-ramp state is applied.
///
/// # Errors
/// Rejects invalid or nonfinite values, bonuses above 12 m/s, overlapping flat
/// penalties, negative final speed, and overflow. `percent` represents one effect.
pub fn calculate(
    base: f64,
    bonuses: impl IntoIterator<Item = f64>,
    percent: f64,
) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    for bonus in bonuses {
        modifiers.add(bonus)?;
    }
    modifiers.calculate(base, percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_diminishing_bonuses_from_the_supplied_examples() {
        assert!((calculate(6.4, [2.0, 3.0], 0.0).unwrap() - 10.9).abs() < 1e-12);
        assert!((calculate(6.4, [3.0, 2.0], 0.0).unwrap() - 10.9).abs() < 1e-12);
        assert_eq!(calculate(6.4, [], 0.0).unwrap(), 6.4);
        assert_eq!(calculate(6.4, [-1.5], 0.0).unwrap(), 4.9);
        assert!((calculate(6.4, [2.0, 3.0], 70.0).unwrap() - 18.53).abs() < 1e-12);
        assert_eq!(calculate(6.4, [12.0, 3.0], 0.0).unwrap(), 18.4);
        assert!(calculate(6.4, [1.0, -1.5], 0.0).is_err());
        assert!(calculate(6.4, [-1.5, 1.0], 0.0).is_err());
    }

    #[test]
    fn rejects_invalid_values() {
        for (base, bonus, percent) in [
            (f64::NAN, 0.0, 0.0),
            (-1.0, 0.0, 0.0),
            (1.0, f64::INFINITY, 0.0),
            (1.0, 13.0, 0.0),
            (1.0, -2.0, 0.0),
            (1.0, 0.0, -101.0),
            (1.0, 0.0, f64::INFINITY),
            (f64::MAX, 0.0, 100.0),
        ] {
            assert!(calculate(base, [bonus], percent).is_err());
        }
    }
}
