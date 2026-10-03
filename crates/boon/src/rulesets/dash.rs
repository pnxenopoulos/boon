//! Shared validation for ordinary ground and air dashes.
use crate::hero_stats::CalculationError;

pub(super) fn duration(seconds: f64) -> Result<f64, CalculationError> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(CalculationError::Invalid("invalid dash duration".into()));
    }
    Ok(seconds)
}

pub(super) fn speed(distance: f64, seconds: f64, percent: f64) -> Result<f64, CalculationError> {
    duration(seconds)?;
    if !distance.is_finite() || distance < 0.0 || !percent.is_finite() || percent < -100.0 {
        return Err(CalculationError::Invalid("invalid dash distance".into()));
    }
    let speed = distance * (1.0 + percent / 100.0) / seconds;
    if !speed.is_finite() {
        return Err(CalculationError::Invalid("dash speed overflow".into()));
    }
    Ok(speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_changes_speed_without_changing_duration() {
        assert_eq!(duration(0.8).unwrap(), 0.8);
        assert_eq!(speed(12.0, 0.8, 0.0).unwrap(), 15.0);
        assert!((speed(12.0, 0.8, 20.0).unwrap() - 18.0).abs() < 1e-12);
        assert!((speed(12.0, 0.8, -20.0).unwrap() - 12.0).abs() < 1e-12);
        assert_eq!(speed(12.0, 0.8, -100.0).unwrap(), 0.0);
    }

    #[test]
    fn rejects_invalid_inputs() {
        for (distance, seconds, percent) in [
            (-1.0, 0.8, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, -1.0, 0.0),
            (f64::NAN, 1.0, 0.0),
            (1.0, f64::INFINITY, 0.0),
            (1.0, 0.8, -101.0),
            (1.0, 0.8, f64::NAN),
            (f64::MAX, 0.1, 100.0),
        ] {
            assert!(speed(distance, seconds, percent).is_err());
        }
    }
}
