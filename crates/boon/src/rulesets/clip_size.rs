//! Magazine capacity: flat bonuses first, additive percentages second.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::ClipSize,
    id: "clip_size.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Calculate `ceil((base + flat) * (1 + percent / 100))`.
///
/// Percent uses percentage points: `19.0` means +19%.
/// Returns an error for nonfinite inputs, negative capacity, or overflow.
pub fn calculate(base: f64, flat: f64, percent: f64) -> Result<u32, CalculationError> {
    if ![base, flat, percent].iter().all(|v| v.is_finite())
        || base < 0.0
        || base + flat < 0.0
        || percent < -100.0
    {
        return Err(CalculationError::Invalid("invalid clip-size inputs".into()));
    }
    // Divide once after multiplying. This also avoids turning exact results
    // such as 50 * 1.10 into 55.00000000000001 before ceil.
    let value = ((base + flat) * (100.0 + percent) / 100.0).ceil();
    if !value.is_finite() || value > f64::from(u32::MAX) {
        return Err(CalculationError::Invalid("clip size exceeds u32".into()));
    }
    Ok(value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_then_additive_percent_then_ceil() {
        assert_eq!(calculate(20.0, 10.0, 15.0 + 4.0).unwrap(), 36);
        assert_eq!(calculate(50.0, 0.0, 10.0).unwrap(), 55);
        assert_eq!(calculate(9.0, 0.0, 0.0).unwrap(), 9);
        assert_eq!(calculate(20.0, 0.0, -10.0).unwrap(), 18);
    }
    #[test]
    fn invalid_inputs_are_errors() {
        for args in [
            (f64::NAN, 0.0, 0.0),
            (20.0, -21.0, 0.0),
            (20.0, 0.0, -101.0),
            (f64::MAX, 0.0, 100.0),
        ] {
            assert!(calculate(args.0, args.1, args.2).is_err());
        }
    }
}
