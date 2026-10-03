//! Spirit resist in percentage points.
use super::Rule;
pub use super::resistance::calculate;
use crate::hero_stats::HeroStat;

pub const V1: Rule = Rule {
    stat: HeroStat::SpiritResist,
    id: "spirit_resist.v1",
    version: 1,
    documented_on: "2026-09-28",
};
