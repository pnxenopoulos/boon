//! Additional nominal sprint speed, in metres per second.
use super::Rule;
use crate::hero_stats::{CalculationError, HeroStat};

pub const V1: Rule = Rule {
    stat: HeroStat::SprintSpeed,
    id: "sprint_speed.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Add all sprint bonuses to base sprint speed. This is the extra sprint
/// component, not movement plus sprint. It excludes eligibility and ramp-up.
///
/// # Errors
/// Rejects nonfinite inputs, negative base or final speed, and overflow.
pub fn calculate(base: f64, bonus: f64) -> Result<f64, CalculationError> {
    let speed = base + bonus;
    if !base.is_finite() || base < 0.0 || !bonus.is_finite() || !speed.is_finite() || speed < 0.0 {
        return Err(CalculationError::Invalid(
            "invalid sprint-speed inputs".into(),
        ));
    }
    Ok(speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_bonuses_add_without_diminishing_returns() {
        assert!((calculate(1.6, 2.0 + 5.0).unwrap() - 8.6).abs() < 1e-12);
        let movement = super::super::move_speed::calculate(6.4, [2.0, 3.0], 0.0).unwrap();
        assert!((movement + calculate(1.6, 2.0 + 1.5).unwrap() - 16.5).abs() < 1e-12);
        assert_eq!(calculate(1.0, -1.0).unwrap(), 0.0);
        for (base, bonus) in [
            (f64::NAN, 0.0),
            (-1.0, 2.0),
            (1.0, -2.0),
            (1.0, f64::INFINITY),
            (f64::MAX, f64::MAX),
        ] {
            assert!(calculate(base, bonus).is_err());
        }
    }
}
