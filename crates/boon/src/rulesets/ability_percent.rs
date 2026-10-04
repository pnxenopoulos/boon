//! Shared equation for ability bonus percentages.
use crate::hero_stats::CalculationError;

/// Combine source percentages without rounding intermediate values.
///
/// # Errors
/// Rejects nonfinite values, percentages above 100, and overflow.
pub fn calculate(sources: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut total = 0.0;
    for percent in sources {
        super::percentage::combine(
            &mut total,
            percent,
            "invalid ability percentage",
            "ability percentage overflow",
        )?;
    }
    Ok(total)
}
