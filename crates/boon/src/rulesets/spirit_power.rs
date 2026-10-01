//! Multiply the complete flat spirit total by each independent percentage bonus.
use crate::hero_stats::CalculationError;

#[derive(Debug)]
pub(crate) struct Modifiers {
    multiplier: f64,
}

impl Default for Modifiers {
    fn default() -> Self {
        Self { multiplier: 1.0 }
    }
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        if !percent.is_finite() || percent < -100.0 {
            return Err(CalculationError::Invalid(
                "invalid spirit-power percentage".into(),
            ));
        }
        self.multiplier *= 1.0 + percent / 100.0;
        if !self.multiplier.is_finite() {
            return Err(CalculationError::Invalid(
                "spirit-power multiplier overflow".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn calculate(self, flat: f64) -> Result<f64, CalculationError> {
        let total = flat * self.multiplier;
        if !flat.is_finite() || !total.is_finite() {
            return Err(CalculationError::Invalid(
                "invalid spirit-power total".into(),
            ));
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calculate(flat: f64, sources: &[f64]) -> Result<f64, CalculationError> {
        let mut modifiers = Modifiers::default();
        for &percent in sources {
            modifiers.add(percent)?;
        }
        modifiers.calculate(flat)
    }

    #[test]
    fn multiplies_independent_bonuses_after_all_flat_sources() {
        assert_eq!(calculate(100.0, &[]).unwrap(), 100.0);
        assert_eq!(calculate(100.0, &[0.0]).unwrap(), 100.0);
        assert!((calculate(143.5, &[15.0]).unwrap() - 165.025).abs() < 1e-12);
        assert_eq!(calculate(100.0, &[20.0, 30.0]).unwrap(), 156.0);
        assert!((calculate(100.0, &[15.0, -15.0]).unwrap() - 97.75).abs() < 1e-12);
        assert_eq!(calculate(100.0, &[-100.0, 20.0]).unwrap(), 0.0);
    }

    #[test]
    fn rejects_invalid_sources_and_overflow() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -100.1] {
            assert!(calculate(100.0, &[value]).is_err());
            assert!(calculate(100.0, &[-100.0, value]).is_err());
        }
        assert!(calculate(f64::NAN, &[]).is_err());
        assert!(calculate(f64::MAX, &[100.0]).is_err());
        assert!(calculate(0.0, &[f64::MAX, f64::MAX]).is_err());
    }
}
