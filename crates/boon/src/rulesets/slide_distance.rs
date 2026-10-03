//! Slide-distance bonus in percentage points; individual factors multiply.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::SlideDistance,
    id: "slide_distance.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Accumulate percentage factors without allocating a list of contributions.
#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    percent: f64,
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        if !percent.is_finite() || percent < -100.0 {
            return Err(CalculationError::Invalid(
                "invalid slide-distance bonus".into(),
            ));
        }
        // Equivalent to multiplying (1 + each bonus / 100). Keep percentage
        // points to avoid cancellation for a small or single contribution.
        self.percent += percent + self.percent * (percent / 100.0);
        Ok(())
    }

    pub(crate) fn calculate(self) -> Result<f64, CalculationError> {
        if !self.percent.is_finite() {
            return Err(CalculationError::Invalid("slide-distance overflow".into()));
        }
        Ok(self.percent)
    }
}

/// Return 100 * (product(1 + each bonus / 100) - 1), without rounding.
/// This is a bonus percentage, not travel in metres.
///
/// # Errors
/// Rejects nonfinite inputs, bonuses below -100%, and overflow.
pub fn calculate(bonuses: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    for percent in bonuses {
        modifiers.add(percent)?;
    }
    modifiers.calculate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_independent_percentage_factors() {
        assert_eq!(calculate([]).unwrap(), 0.0);
        assert_eq!(calculate([37.5]).unwrap(), 37.5);
        assert_eq!(calculate([35.0, 50.0]).unwrap(), 102.5);
        assert_eq!(calculate([50.0, 35.0]).unwrap(), 102.5);
        assert_eq!(calculate([50.0, -20.0]).unwrap(), 20.0);
        assert_eq!(calculate([50.0, -100.0]).unwrap(), -100.0);
    }

    #[test]
    fn rejects_invalid_inputs_and_overflow() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -100.1] {
            assert!(calculate([value]).is_err());
        }
        assert!(calculate([f64::MAX, f64::MAX]).is_err());
    }
}
