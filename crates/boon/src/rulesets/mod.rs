//! Versioned equations for calculated hero stats.
pub mod air_dash_duration;
pub mod air_dash_speed;
pub mod bullet_evasion;
pub mod bullet_lifesteal;
pub mod bullet_resist;
pub mod bullet_velocity;
pub mod clip_size;
mod dash;
pub mod dash_duration;
pub mod dash_speed;
pub mod debuff_resist;
pub mod falloff_range;
pub mod fire_rate;
pub mod gravity_scale;
pub(crate) mod lifesteal;
pub mod melee_damage;
pub mod melee_distance;
pub mod melee_lifesteal;
pub mod melee_resist;
pub mod move_speed;
pub mod reload_time;
pub(crate) mod resistance;
pub mod slide_distance;
pub mod spirit_lifesteal;
pub mod spirit_resist;
pub mod sprint_speed;
pub mod stamina;
pub mod stamina_cooldown;
pub mod weapon_damage;

use crate::hero_stats::HeroStat;
use serde::Serialize;

/// Identity of an equation, independent of the boon-data client version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Rule {
    pub stat: HeroStat,
    pub id: &'static str,
    pub version: u32,
    /// Date this equation was documented in Boon; not a game patch date.
    pub documented_on: &'static str,
}

/// Source distances use inches. This conversion is independent of balance values.
pub const METERS_PER_SOURCE_UNIT: f64 = 0.0254;

pub mod cooldown_reduction;

pub mod item_cooldown_reduction;

pub mod duration_bonus;

pub mod range_bonus;

pub mod radius_bonus;

mod ability_percent;
