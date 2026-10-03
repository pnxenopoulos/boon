//! Version 1: 100 * (1 - product(1 - each percentage / 100)).
pub use super::ability_percent::calculate;
use crate::ability_stats::{AbilityRule, AbilityStat};
pub const V1: AbilityRule = AbilityStat::CooldownReduction.rule();
