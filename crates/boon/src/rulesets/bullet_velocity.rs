//! Nominal primary-weapon bullet speed with additive percentage bonuses.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::BulletVelocity,
    id: "bullet_velocity.v1",
    version: 1,
    documented_on: "2026-09-27",
};

pub use super::METERS_PER_SOURCE_UNIT;

/// Calculate `base_source_units_per_second * (1 + percent / 100) * 0.0254`.
/// Percent uses percentage points. The result is in metres per second, without rounding.
///
/// # Errors
/// Returns an error for nonfinite inputs, negative speeds, or overflow.
pub fn calculate(base: f64, percent: f64) -> Result<f64, CalculationError> {
    if !base.is_finite() || !percent.is_finite() || base < 0.0 || percent < -100.0 {
        return Err(CalculationError::Invalid(
            "invalid bullet-velocity inputs".into(),
        ));
    }
    let value = base * METERS_PER_SOURCE_UNIT * (1.0 + percent / 100.0);
    if !value.is_finite() {
        return Err(CalculationError::Invalid("bullet velocity overflow".into()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_units_and_adds_percentages_without_rounding() {
        assert!((calculate(8000.0, 0.0).unwrap() - 203.2).abs() < 1e-10);
        assert!((calculate(8000.0, 60.0 + 25.0).unwrap() - 375.92).abs() < 1e-10);
        assert!((calculate(1000.0, -20.0).unwrap() - 20.32).abs() < 1e-10);
        assert_eq!(calculate(8000.0, -100.0).unwrap(), 0.0);
    }

    #[test]
    fn rejects_invalid_inputs() {
        for (base, percent) in [
            (f64::NAN, 0.0),
            (-1.0, 0.0),
            (1.0, -101.0),
            (1.0, f64::INFINITY),
            (f64::MAX, f64::MAX),
        ] {
            assert!(calculate(base, percent).is_err());
        }
    }
}
