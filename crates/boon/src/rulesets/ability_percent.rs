//! Shared complement-product accumulator for ability percentage rules.
use crate::hero_stats::CalculationError;
pub(super) fn calculate(sources: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut total = 0.0;
    for value in sources {
        if !value.is_finite() || value > 100.0 {
            return Err(CalculationError::Invalid(
                "invalid ability percentage".into(),
            ));
        }
        total += value * (1.0 - total / 100.0);
        if !total.is_finite() {
            return Err(CalculationError::Invalid(
                "ability percentage overflow".into(),
            ));
        }
    }
    Ok(total)
}
