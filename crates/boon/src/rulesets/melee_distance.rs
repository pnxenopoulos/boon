//! Heavy-melee travel bonus, expressed in percentage points.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::MeleeDistance,
    id: "melee_distance.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Return the sum of resolved percentage bonuses, without rounding or clamping.
/// Zero means no bonus; 50 means +50%, not 0.5 or a distance in metres.
///
/// # Errors
/// Returns an error if the total is not finite.
pub fn calculate(percent: f64) -> Result<f64, CalculationError> {
    if !percent.is_finite() {
        return Err(CalculationError::Invalid(
            "nonfinite melee-distance bonus".into(),
        ));
    }
    Ok(percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_percentage_points_and_rejects_nonfinite_totals() {
        for percent in [0.0, 37.5, -10.0, 225.0] {
            assert_eq!(calculate(percent).unwrap(), percent);
        }
        for percent in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(calculate(percent).is_err());
        }
    }
}
