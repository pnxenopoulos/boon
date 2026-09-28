//! Resistance and reduction are separate multiplicative groups.
use crate::hero_stats::CalculationError;

#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    resistance: f64,
    reduction: f64,
}

fn combine(total: &mut f64, percent: f64) -> Result<(), CalculationError> {
    if !percent.is_finite() || percent > 100.0 {
        return Err(CalculationError::Invalid(
            "invalid resistance percentage".into(),
        ));
    }
    *total += percent * (1.0 - *total / 100.0);
    if !total.is_finite() {
        return Err(CalculationError::Invalid("resistance overflow".into()));
    }
    Ok(())
}

impl Modifiers {
    pub(crate) fn add_resistance(&mut self, percent: f64) -> Result<(), CalculationError> {
        combine(&mut self.resistance, percent)
    }

    /// Reductions use positive magnitudes in the equation.
    pub(crate) fn add_reduction(&mut self, percent: f64) -> Result<(), CalculationError> {
        if percent < 0.0 {
            return Err(CalculationError::Invalid(
                "negative resistance-reduction magnitude".into(),
            ));
        }
        combine(&mut self.reduction, percent)
    }

    pub(crate) fn calculate(self) -> Result<f64, CalculationError> {
        let value = self.resistance - self.reduction;
        if !value.is_finite() {
            return Err(CalculationError::Invalid("resistance overflow".into()));
        }
        Ok(value)
    }
}

/// Calculate resistance in percentage points without display rounding.
/// `innate` is base resistance plus additive boon and spirit growth. Other
/// resistance sources and positive reduction magnitudes each stack separately.
/// Negative resistance remains negative; this does not calculate damage taken.
///
/// # Errors
/// Rejects nonfinite inputs, sources above 100%, negative reduction magnitudes,
/// and arithmetic overflow.
pub fn calculate(
    innate: f64,
    sources: impl IntoIterator<Item = f64>,
    reductions: impl IntoIterator<Item = f64>,
) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    modifiers.add_resistance(innate)?;
    for percent in sources {
        modifiers.add_resistance(percent)?;
    }
    for percent in reductions {
        modifiers.add_reduction(percent)?;
    }
    modifiers.calculate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtracts_separately_stacked_reductions() {
        assert_eq!(calculate(0.0, [40.0, 20.0], []).unwrap(), 52.0);
        assert_eq!(calculate(0.0, [40.0, 20.0], [25.0, 20.0]).unwrap(), 12.0);
        assert_eq!(calculate(0.0, [10.0], [25.0, 20.0]).unwrap(), -30.0);
        assert_eq!(calculate(-15.0, [20.0], []).unwrap(), 8.0);
        assert_eq!(calculate(10.0 + 5.0 + 5.0, [25.0], []).unwrap(), 40.0);
        assert_eq!(calculate(0.0, [], []).unwrap(), 0.0);
        assert_eq!(calculate(0.0, [1e-15], []).unwrap(), 1e-15);
    }

    #[test]
    fn validates_both_groups_without_clamping() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 100.1] {
            assert!(calculate(value, [], []).is_err());
            assert!(calculate(0.0, [value], []).is_err());
            assert!(calculate(0.0, [], [value]).is_err());
        }
        assert!(calculate(0.0, [], [-10.0]).is_err());
        assert!(calculate(-f64::MAX, [-f64::MAX], []).is_err());
        assert_eq!(calculate(-20.0, [], [100.0]).unwrap(), -120.0);
    }
}
