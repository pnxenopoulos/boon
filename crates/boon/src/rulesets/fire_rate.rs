//! UI fire-rate modifier: additive bonuses and multiplicative slows.
use super::Rule;
use crate::hero_stats::CalculationError;

pub const V1: Rule = Rule {
    stat: crate::hero_stats::HeroStat::FireRate,
    id: "fire_rate.v1",
    version: 1,
    documented_on: "2026-09-27",
};

/// Accumulate individual signed percentage changes without allocating a list.
#[derive(Debug)]
pub(crate) struct Modifiers {
    bonuses: f64,
    remaining: f64,
}

impl Default for Modifiers {
    fn default() -> Self {
        Self {
            bonuses: 0.0,
            remaining: 1.0,
        }
    }
}

impl Modifiers {
    pub(crate) fn add(&mut self, percent: f64) -> Result<(), CalculationError> {
        if !percent.is_finite() || percent < -100.0 {
            return Err(CalculationError::Invalid(
                "invalid fire-rate adjustment".into(),
            ));
        }
        if percent < 0.0 {
            self.remaining *= 1.0 + percent / 100.0;
        } else {
            self.bonuses += percent;
        }
        Ok(())
    }

    pub(crate) fn calculate(self) -> Result<f64, CalculationError> {
        let value = self.bonuses - 100.0 * (1.0 - self.remaining);
        if !value.is_finite() {
            return Err(CalculationError::Invalid("fire rate overflow".into()));
        }
        Ok(value.max(-50.0))
    }
}

/// Return the UI modifier in percentage points, without rounding.
/// Positive inputs are bonuses; negative inputs are individual slows.
///
/// # Errors
/// Rejects nonfinite inputs, slows greater than 100%, and overflow.
pub fn calculate(changes: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    let mut modifiers = Modifiers::default();
    for change in changes {
        modifiers.add(change)?;
    }
    modifiers.calculate()
}

/// Convert the calculated UI percentage to a multiplier on base fire rate.
/// A negative modifier lengthens cycle time instead of reducing rate directly.
///
/// # Errors
/// Rejects nonfinite modifiers or values below the -50% cap.
pub fn rate_multiplier(percent: f64) -> Result<f64, CalculationError> {
    if !percent.is_finite() || percent < -50.0 {
        return Err(CalculationError::Invalid(
            "invalid calculated fire-rate modifier".into(),
        ));
    }
    Ok(if percent >= 0.0 {
        1.0 + percent / 100.0
    } else {
        1.0 / (1.0 - percent / 100.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_bonuses_and_individual_slows_then_caps() {
        assert_eq!(calculate([]).unwrap(), 0.0);
        assert_eq!(calculate([18.0, 20.0]).unwrap(), 38.0);
        assert!((calculate([-20.0, -30.0]).unwrap() + 44.0).abs() < 1e-12);
        assert!((calculate([18.0, 20.0, -20.0, -30.0]).unwrap() + 6.0).abs() < 1e-12);
        assert_eq!(calculate([-70.0, -20.0]).unwrap(), -50.0);
        // Cap only after adding bonuses, not before.
        assert!((calculate([40.0, -70.0, -20.0]).unwrap() + 36.0).abs() < 1e-12);
        assert_eq!(calculate([150.0]).unwrap(), 150.0);
    }

    #[test]
    fn negative_modifiers_lengthen_cycle_time() {
        for (percent, expected) in [
            (0.0, 1.0),
            (50.0, 1.5),
            (-10.0, 1.0 / 1.1),
            (-25.0, 0.8),
            (-50.0, 2.0 / 3.0),
        ] {
            assert!((rate_multiplier(percent).unwrap() - expected).abs() < 1e-12);
        }
        assert!(rate_multiplier(-51.0).is_err());
        assert!(rate_multiplier(f64::NAN).is_err());
    }

    #[test]
    fn rejects_invalid_inputs() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -101.0] {
            assert!(calculate([value]).is_err());
        }
        assert!(calculate([f64::MAX, f64::MAX]).is_err());
    }
}
