//! Bullet-evasion chance in percentage points.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::BulletEvasion,
    id: "bullet_evasion.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Return one resolved chance. The resolver rejects multiple nonzero chances.
///
/// # Errors
/// Returns an error for nonfinite inputs, out-of-range values, or overflow.
pub fn calculate(percent: f64) -> Result<f64, CalculationError> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(CalculationError::Invalid(
            "invalid bullet_evasion inputs".into(),
        ));
    }
    let value = percent;
    if !value.is_finite() {
        return Err(CalculationError::Invalid("bullet_evasion overflow".into()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_percentage_points_and_rejects_invalid_inputs() {
        for value in [0.0, 37.5, 100.0] {
            assert_eq!(calculate(value).unwrap(), value);
        }
        for value in [f64::NAN, f64::INFINITY, -0.1, 100.1] {
            assert!(calculate(value).is_err());
        }
    }
}
