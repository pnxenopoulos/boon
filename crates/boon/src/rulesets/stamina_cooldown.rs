//! Seconds to recover one stamina point.
use super::Rule;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::StaminaCooldown,
    id: "stamina_cooldown.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Return the reciprocal of the adjusted regeneration rate.
/// V1 accepts a flat rate adjustment OR one percentage adjustment. The order
/// of mixed adjustments and percentage stacking has not been verified.
///
/// # Errors
/// Rejects mixed adjustments, nonfinite inputs, and nonpositive recovery rates.
pub fn calculate(
    base_per_second: f64,
    flat: f64,
    percent: f64,
) -> Result<f64, crate::hero_stats::CalculationError> {
    if flat != 0.0 && percent != 0.0 {
        return Err(crate::hero_stats::CalculationError::Invalid(
            "mixed stamina recovery adjustments are not supported by stamina_cooldown.v1".into(),
        ));
    }
    let rate = (base_per_second + flat) * (1.0 + percent / 100.0);
    let seconds = rate.recip();
    if !base_per_second.is_finite()
        || base_per_second < 0.0
        || !flat.is_finite()
        || !percent.is_finite()
        || percent <= -100.0
        || !rate.is_finite()
        || rate <= 0.0
        || !seconds.is_finite()
    {
        return Err(crate::hero_stats::CalculationError::Invalid(
            "invalid stamina recovery rate".into(),
        ));
    }
    Ok(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_percentage_changes_rate_not_time() {
        assert_eq!(calculate(0.2, 0.0, 0.0).unwrap(), 5.0);
        assert!((calculate(0.2, 0.0, -20.0).unwrap() - 6.25).abs() < 1e-12);
        assert_eq!(calculate(0.2, 0.0, 25.0).unwrap(), 4.0);
        assert_eq!(calculate(0.2, 0.05, 0.0).unwrap(), 4.0);
        assert!(calculate(0.2, 0.05, 25.0).is_err());
    }

    #[test]
    fn rejects_nonfinite_or_nonpositive_recovery() {
        for (base, flat, percent) in [
            (f64::NAN, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (-1.0, 2.0, 0.0),
            (0.2, -0.2, 0.0),
            (0.2, 0.0, -100.0),
            (0.2, 0.0, -101.0),
            (0.2, f64::INFINITY, 0.0),
            (0.2, 0.0, f64::INFINITY),
            (f64::MAX, 0.0, 100.0),
        ] {
            assert!(calculate(base, flat, percent).is_err());
        }
    }
}
