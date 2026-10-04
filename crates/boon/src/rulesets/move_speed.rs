//! Nominal movement speed with additive flat adjustments, in metres per second.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::MoveSpeed,
    id: "move_speed.v1",
    version: 1,
    documented_on: "2026-10-02",
};

#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    flat: f64,
}

impl Modifiers {
    pub(crate) fn add(&mut self, bonus: f64) -> Result<(), CalculationError> {
        let flat = self.flat + bonus;
        if !bonus.is_finite() || !flat.is_finite() {
            return Err(CalculationError::Invalid(
                "invalid move-speed adjustment".into(),
            ));
        }
        self.flat = flat;
        Ok(())
    }

    pub(crate) fn calculate(self, base: f64, percent: f64) -> Result<f64, CalculationError> {
        let speed = (base + self.flat) * (1.0 + percent / 100.0);
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

/// Return `(base + sum(flat_adjustments)) * (1 + percent / 100)`.
/// Flat bonuses and penalties add. The percentage applies only to movement
/// speed, not the extra sprint component. No firing, crouching, slow,
/// speed-limit or sprint-ramp state is applied.
///
/// # Errors
/// Rejects nonfinite inputs, negative base or final speed, and overflow.
/// `percent` represents one effect.
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
    fn adds_flat_adjustments_before_the_percentage() {
        assert!((calculate(6.7, [0.6, 0.3, 2.5, 1.25], 0.0).unwrap() - 11.35).abs() < 1e-12);
        assert!((calculate(6.4, [2.0, 3.0], 70.0).unwrap() - 19.38).abs() < 1e-12);
        assert!((calculate(6.4, [12.0, 3.0], 0.0).unwrap() - 21.4).abs() < 1e-12);
        assert_eq!(calculate(6.4, [], 0.0).unwrap(), 6.4);
        assert!((calculate(6.4, [1.0, -1.5], 0.0).unwrap() - 5.9).abs() < 1e-12);
    }

    #[test]
    fn rejects_invalid_values() {
        for (base, bonus, percent) in [
            (f64::NAN, 0.0, 0.0),
            (-1.0, 0.0, 0.0),
            (1.0, f64::INFINITY, 0.0),
            (1.0, f64::NAN, 0.0),
            (1.0, -2.0, 0.0),
            (1.0, 0.0, -101.0),
            (1.0, 0.0, f64::INFINITY),
            (f64::MAX, 0.0, 100.0),
        ] {
            assert!(calculate(base, [bonus], percent).is_err());
        }
        assert!(calculate(0.0, [f64::MAX, f64::MAX, -f64::MAX], 0.0).is_err());
    }
}
