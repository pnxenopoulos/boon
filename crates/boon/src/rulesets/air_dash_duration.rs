//! Nominal ordinary dash duration in seconds.
use super::Rule;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::AirDashDuration,
    id: "air_dash_duration.v1",
    version: 1,
    documented_on: "2026-09-28",
};

/// Return the hero's catalog duration in seconds, without rounding.
///
/// # Errors
/// Rejects nonfinite or nonpositive durations.
pub fn calculate(seconds: f64) -> Result<f64, crate::hero_stats::CalculationError> {
    super::dash::duration(seconds)
}
