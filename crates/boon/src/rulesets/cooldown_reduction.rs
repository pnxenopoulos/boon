//! Version 1: 100 * (1 - product(1 - each percentage / 100)).
use crate::ability_stats::{AbilityRule, AbilityStat};
use crate::hero_stats::CalculationError;
pub const V1: AbilityRule = AbilityStat::CooldownReduction.rule();
/// Combine source percentages without intermediate rounding.
/// # Errors
/// Rejects nonfinite values, percentages above 100, and overflow.
pub fn calculate(sources: impl IntoIterator<Item = f64>) -> Result<f64, CalculationError> {
    super::ability_percent::calculate(sources)
}
