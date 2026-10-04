//! Recorded player-pawn gravity scale.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::GravityScale,
    id: "gravity_scale.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Return the recorded pawn scale unchanged. Modifiers are not applied.
///
/// # Errors
/// Rejects nonfinite or negative recorded values.
pub fn calculate(recorded: f64) -> Result<f64, CalculationError> {
    if !recorded.is_finite() || recorded < 0.0 {
        return Err(CalculationError::Invalid(
            "invalid recorded gravity scale".into(),
        ));
    }
    Ok(recorded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_recorded_scale() {
        for value in [0.0, 0.725, 1.0, 1.35] {
            assert_eq!(calculate(value).unwrap(), value);
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
            assert!(calculate(value).is_err());
        }
    }
}
