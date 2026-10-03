//! Shared multiplicative lifesteal equation, in percentage points.
use crate::hero_stats::CalculationError;

#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    percent: f64,
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(CalculationError::Invalid(
                "invalid lifesteal percentage".into(),
            ));
        }
        // Equivalent to 100 * (1 - product(1 - source / 100)), without a
        // temporary collection or loss of precision for small single sources.
        self.percent += percent * (1.0 - self.percent / 100.0);
        Ok(())
    }

    pub(crate) fn calculate(self) -> f64 {
        self.percent
    }
}

/// Combine independent lifesteal sources in percentage points, without rounding.
///
/// # Errors
/// Rejects nonfinite sources and percentages outside 0..=100.
pub fn calculate(sources: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    for percent in sources {
        modifiers.add(percent)?;
    }
    Ok(modifiers.calculate())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_independent_sources_without_display_rounding() {
        assert_eq!(calculate([]).unwrap(), 0.0);
        assert_eq!(calculate([13.0]).unwrap(), 13.0);
        assert!((calculate([22.0, 30.0]).unwrap() - 45.4).abs() < 1e-12);
        assert_eq!(calculate([25.0, 25.0]).unwrap(), 43.75);
        assert_eq!(calculate([100.0, 20.0]).unwrap(), 100.0);
        assert_eq!(calculate([1e-15]).unwrap(), 1e-15);
    }

    #[test]
    fn rejects_invalid_sources_even_after_full_lifesteal() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 100.1] {
            assert!(calculate([100.0, value]).is_err());
        }
    }
}
