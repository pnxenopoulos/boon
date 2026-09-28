//! Debuff duration resistance in percentage points.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::DebuffResist,
    id: "debuff_resist.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Accumulate remaining-duration factors without allocating contributions.
#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    percent: f64,
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        if !percent.is_finite() || percent > 100.0 {
            return Err(CalculationError::Invalid(
                "invalid debuff resistance".into(),
            ));
        }
        // Equivalent to 100 * (1 - product(1 - each resistance / 100)).
        // Keep percentage points to preserve small and single contributions.
        self.percent += percent * (1.0 - self.percent / 100.0);
        if !self.percent.is_finite() {
            return Err(CalculationError::Invalid(
                "debuff-resistance overflow".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn calculate(self) -> f64 {
        self.percent
    }
}

/// Combine innate and modifier percentages as remaining-duration factors.
/// Negative resistance increases duration. The result is not rounded or
/// clamped to zero. This does not describe immunity or cleanse effects.
///
/// # Errors
/// Rejects nonfinite inputs, values above 100%, and arithmetic overflow.
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
    fn multiplies_remaining_durations_and_preserves_negative_resistance() {
        assert_eq!(calculate([]).unwrap(), 0.0);
        assert_eq!(calculate([30.0]).unwrap(), 30.0);
        assert_eq!(calculate([25.0, 25.0]).unwrap(), 43.75);
        assert_eq!(calculate([-8.0]).unwrap(), -8.0);
        assert_eq!(calculate([-8.0, 25.0]).unwrap(), 19.0);
        assert_eq!(calculate([25.0, -8.0]).unwrap(), 19.0);
        assert_eq!(calculate([100.0, -50.0]).unwrap(), 100.0);
        assert_eq!(calculate([1e-15]).unwrap(), 1e-15);
    }

    #[test]
    fn rejects_invalid_inputs_and_overflow() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 100.1] {
            assert!(calculate([value]).is_err());
        }
        assert!(calculate([-f64::MAX, -f64::MAX]).is_err());
    }
}
