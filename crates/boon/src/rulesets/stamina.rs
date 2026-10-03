//! Maximum stamina capacity, in points.
use super::Rule;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::Stamina,
    id: "stamina.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Add flat bonuses to base capacity, without rounding.
/// Free dashes do not change maximum capacity.
///
/// # Errors
/// Rejects nonfinite inputs, negative capacity, and overflow.
pub fn calculate(base: f64, flat: f64) -> Result<f64, crate::hero_stats::CalculationError> {
    let capacity = base + flat;
    if !base.is_finite()
        || base < 0.0
        || !flat.is_finite()
        || !capacity.is_finite()
        || capacity < 0.0
    {
        return Err(crate::hero_stats::CalculationError::Invalid(
            "invalid stamina capacity".into(),
        ));
    }
    Ok(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_capacity_without_rounding() {
        assert_eq!(calculate(4.0, 2.5).unwrap(), 6.5);
        assert_eq!(calculate(4.0, -1.0).unwrap(), 3.0);
        assert_eq!(calculate(0.0, 0.0).unwrap(), 0.0);
        for (base, flat) in [
            (f64::NAN, 0.0),
            (-1.0, 2.0),
            (1.0, -2.0),
            (1.0, f64::INFINITY),
            (f64::MAX, f64::MAX),
        ] {
            assert!(calculate(base, flat).is_err());
        }
    }
}
