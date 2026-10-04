//! Nominal ordinary dash speed in metres per second.
use super::Rule;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::DashSpeed,
    id: "dash_speed.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Return nominal distance / duration, with one signed distance adjustment.
/// Positive percentages increase distance, with no change to duration.
///
/// # Errors
/// Rejects invalid inputs, zero duration, and overflow.
pub fn calculate(
    distance: f64,
    duration: f64,
    percent: f64,
) -> Result<f64, crate::hero_stats::CalculationError> {
    super::dash::speed(distance, duration, percent)
}
