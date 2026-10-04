//! Nominal reload duration, in seconds per magazine or per single round.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::ReloadTime,
    id: "reload_time.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Calculate `base_seconds * (1 + percent / 100)` without rounding.
/// A negative percentage reduces time. Combining adjustments requires a separate rule.
///
/// # Errors
/// Returns an error for nonfinite inputs, negative durations, or overflow.
pub fn calculate(base: f64, percent: f64) -> Result<f64, CalculationError> {
    if !base.is_finite() || !percent.is_finite() || base < 0.0 || percent < -100.0 {
        return Err(CalculationError::Invalid(
            "invalid reload-time inputs".into(),
        ));
    }
    let value = base * (1.0 + percent / 100.0);
    if !value.is_finite() {
        return Err(CalculationError::Invalid("reload time overflow".into()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_signed_time_adjustments_without_rounding() {
        for (base, percent, expected) in [
            (2.0, 0.0, 2.0),
            (2.0, -10.0, 1.8),
            (2.0, 25.0, 2.5),
            (0.35, -20.0, 0.28),
            (2.0, -100.0, 0.0),
        ] {
            assert!((calculate(base, percent).unwrap() - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn rejects_invalid_inputs() {
        for (base, percent) in [
            (f64::NAN, 0.0),
            (-1.0, 0.0),
            (1.0, -101.0),
            (1.0, f64::INFINITY),
            (f64::MAX, 100.0),
        ] {
            assert!(calculate(base, percent).is_err());
        }
    }
}
