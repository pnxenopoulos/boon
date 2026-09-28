//! Melee lifesteal in percentage points.
use super::Rule;
pub use super::lifesteal::calculate;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::MeleeLifesteal,
    id: "melee_lifesteal.v1",
    version: 1,
    documented_on: "2026-09-28",
};
