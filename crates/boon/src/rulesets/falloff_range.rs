//! Primary-weapon damage falloff endpoints with one percentage range adjustment.
use super::{METERS_PER_SOURCE_UNIT, Rule};
use crate::hero_stats::{CalculationError, HeroStat};

pub const START_V1: Rule = Rule {
    stat: HeroStat::FalloffStart,
    id: "falloff_start.v1",
    version: 1,
    documented_on: "2026-09-27",
};
pub const END_V1: Rule = Rule {
    stat: HeroStat::FalloffEnd,
    id: "falloff_end.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Scale both base distances by `1 + percent / 100` and convert to metres.
/// Percent uses percentage points. End is maximum falloff, not maximum bullet travel.
/// The resolver rejects multiple nonzero bonuses until their stacking is verified.
///
/// # Errors
/// Returns an error for missing-range sentinels, unordered or nonfinite distances,
/// percentages below -100, or overflow.
pub fn calculate(start: f64, end: f64, percent: f64) -> Result<(f64, f64), CalculationError> {
    if !start.is_finite()
        || !end.is_finite()
        || !percent.is_finite()
        || start < 0.0
        || end < start
        || percent < -100.0
    {
        return Err(CalculationError::Invalid(
            "invalid or unsupported falloff-range inputs".into(),
        ));
    }
    // Assumption to verify against replay UI: the range percentage scales both
    // endpoints equally. VData provides the inputs, not the engine equation.
    let scale = METERS_PER_SOURCE_UNIT * (1.0 + percent / 100.0);
    let value = (start * scale, end * scale);
    if !value.0.is_finite() || !value.1.is_finite() {
        return Err(CalculationError::Invalid("falloff range overflow".into()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_both_endpoints_without_rounding_or_a_fixed_distance_shift() {
        for (percent, expected) in [
            (0.0, (20.0, 50.0)),
            (20.0, (24.0, 60.0)),
            (-20.0, (16.0, 40.0)),
            (-100.0, (0.0, 0.0)),
        ] {
            let (start, end) = calculate(
                20.0 / METERS_PER_SOURCE_UNIT,
                50.0 / METERS_PER_SOURCE_UNIT,
                percent,
            )
            .unwrap();
            assert!((start - expected.0).abs() < 1e-10);
            assert!((end - expected.1).abs() < 1e-10);
        }
        assert_eq!(calculate(0.0, 0.0, 0.0).unwrap(), (0.0, 0.0));
    }

    #[test]
    fn rejects_sentinels_invalid_ranges_and_overflow() {
        for (start, end, percent) in [
            (-1.0, -1.0, 0.0),
            (20.0, 10.0, 0.0),
            (f64::NAN, 10.0, 0.0),
            (1.0, f64::INFINITY, 0.0),
            (1.0, 2.0, -101.0),
            (1.0, 2.0, f64::NAN),
            (f64::MAX, f64::MAX, f64::MAX),
        ] {
            assert!(calculate(start, end, percent).is_err());
        }
    }
}
