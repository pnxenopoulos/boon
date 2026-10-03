//! Shared remaining-factor equation. Callers supply stat-specific error labels.
use crate::hero_stats::CalculationError;

pub(super) fn combine(
    total: &mut f64,
    percent: f64,
    invalid: &'static str,
    overflow: &'static str,
) -> Result<(), CalculationError> {
    if !percent.is_finite() || percent > 100.0 {
        return Err(CalculationError::Invalid(invalid.into()));
    }
    // Equivalent to 100 * (1 - product(1 - each source / 100)). Keep percentage
    // points to preserve small and single contributions without an allocation.
    *total += percent * (1.0 - *total / 100.0);
    if !total.is_finite() {
        return Err(CalculationError::Invalid(overflow.into()));
    }
    Ok(())
}
