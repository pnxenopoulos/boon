//! Calculated hero stats from replay state, boon-data and explicit equations.
//!
//! Clip size is magazine capacity, not remaining rounds or unlimited-ammo state.
mod batch;
mod catalog;
mod inputs;
mod replay;
use crate::{
    Parser,
    rulesets::{self, Rule},
};
pub use batch::{StatBatch, StatBatchResult};
pub use catalog::StatCatalog;
pub use inputs::abilities as ability_stats;
pub use replay::StatReplay;
use serde::Serialize;
use std::collections::HashMap;

/// An error resolving data, replay state or a calculation.
#[derive(Debug, thiserror::Error)]
pub enum CalculationError {
    #[error(transparent)]
    Data(#[from] crate::data::DataError),
    #[error(transparent)]
    Parse(#[from] crate::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Stats with implemented calculation rules. This is not a network enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeroStat {
    ClipSize,
    BulletVelocity,
    MeleeDistance,
    ReloadTime,
    FireRate,
    FalloffStart,
    FalloffEnd,
    LightMeleeDamage,
    HeavyMeleeDamage,
    SlideDistance,
    BulletEvasion,
    DebuffResist,
    WeaponDamage,
    BulletResist,
    SpiritResist,
    MeleeResist,
    BulletLifesteal,
    SpiritLifesteal,
    MeleeLifesteal,
    GravityScale,
    Stamina,
    StaminaCooldown,
    DashSpeed,
    DashDuration,
    AirDashSpeed,
    AirDashDuration,
    MoveSpeed,
    SprintSpeed,
}

impl HeroStat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClipSize => "clip_size",
            Self::BulletVelocity => "bullet_velocity",
            Self::MeleeDistance => "melee_distance",
            Self::ReloadTime => "reload_time",
            Self::FireRate => "fire_rate",
            Self::FalloffStart => "falloff_start",
            Self::FalloffEnd => "falloff_end",
            Self::LightMeleeDamage => "light_melee_damage",
            Self::HeavyMeleeDamage => "heavy_melee_damage",
            Self::SlideDistance => "slide_distance",
            Self::BulletEvasion => "bullet_evasion",
            Self::DebuffResist => "debuff_resist",
            Self::WeaponDamage => "weapon_damage",
            Self::MeleeResist => "melee_resist",
            Self::SpiritResist => "spirit_resist",
            Self::BulletResist => "bullet_resist",
            Self::MeleeLifesteal => "melee_lifesteal",
            Self::SpiritLifesteal => "spirit_lifesteal",
            Self::BulletLifesteal => "bullet_lifesteal",
            Self::GravityScale => "gravity_scale",
            Self::Stamina => "stamina",
            Self::StaminaCooldown => "stamina_cooldown",
            Self::DashSpeed => "dash_speed",
            Self::DashDuration => "dash_duration",
            Self::AirDashSpeed => "air_dash_speed",
            Self::AirDashDuration => "air_dash_duration",
            Self::MoveSpeed => "move_speed",
            Self::SprintSpeed => "sprint_speed",
        }
    }

    pub const fn unit(self) -> &'static str {
        match self {
            Self::ClipSize => "rounds",
            Self::BulletVelocity
            | Self::DashSpeed
            | Self::AirDashSpeed
            | Self::MoveSpeed
            | Self::SprintSpeed => "m/s",
            Self::MeleeDistance
            | Self::FireRate
            | Self::SlideDistance
            | Self::BulletEvasion
            | Self::BulletResist
            | Self::SpiritResist
            | Self::MeleeResist
            | Self::WeaponDamage
            | Self::DebuffResist
            | Self::BulletLifesteal
            | Self::SpiritLifesteal
            | Self::MeleeLifesteal => "%",
            Self::ReloadTime
            | Self::StaminaCooldown
            | Self::DashDuration
            | Self::AirDashDuration => "s",
            Self::Stamina => "points",
            Self::FalloffStart | Self::FalloffEnd => "m",
            Self::LightMeleeDamage | Self::HeavyMeleeDamage => "damage",
            Self::GravityScale => "multiplier",
        }
    }

    pub const fn rule(self) -> Rule {
        match self {
            Self::ClipSize => rulesets::clip_size::V1,
            Self::BulletVelocity => rulesets::bullet_velocity::V1,
            Self::MeleeDistance => rulesets::melee_distance::V1,
            Self::ReloadTime => rulesets::reload_time::V1,
            Self::FireRate => rulesets::fire_rate::V1,
            Self::FalloffStart => rulesets::falloff_range::START_V1,
            Self::FalloffEnd => rulesets::falloff_range::END_V1,
            Self::LightMeleeDamage => rulesets::melee_damage::LIGHT_V1,
            Self::HeavyMeleeDamage => rulesets::melee_damage::HEAVY_V1,
            Self::SlideDistance => rulesets::slide_distance::V1,
            Self::BulletEvasion => rulesets::bullet_evasion::V1,
            Self::DebuffResist => rulesets::debuff_resist::V1,
            Self::WeaponDamage => rulesets::weapon_damage::V1,
            Self::MeleeResist => rulesets::melee_resist::V1,
            Self::SpiritResist => rulesets::spirit_resist::V1,
            Self::BulletResist => rulesets::bullet_resist::V1,
            Self::MeleeLifesteal => rulesets::melee_lifesteal::V1,
            Self::SpiritLifesteal => rulesets::spirit_lifesteal::V1,
            Self::BulletLifesteal => rulesets::bullet_lifesteal::V1,
            Self::GravityScale => rulesets::gravity_scale::V1,
            Self::Stamina => rulesets::stamina::V1,
            Self::StaminaCooldown => rulesets::stamina_cooldown::V1,
            Self::DashSpeed => rulesets::dash_speed::V1,
            Self::DashDuration => rulesets::dash_duration::V1,
            Self::AirDashSpeed => rulesets::air_dash_speed::V1,
            Self::AirDashDuration => rulesets::air_dash_duration::V1,
            Self::MoveSpeed => rulesets::move_speed::V1,
            Self::SprintSpeed => rulesets::sprint_speed::V1,
        }
    }
}

impl std::str::FromStr for HeroStat {
    type Err = CalculationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "clip_size" => Ok(Self::ClipSize),
            "bullet_velocity" => Ok(Self::BulletVelocity),
            "melee_distance" => Ok(Self::MeleeDistance),
            "reload_time" => Ok(Self::ReloadTime),
            "fire_rate" => Ok(Self::FireRate),
            "falloff_start" => Ok(Self::FalloffStart),
            "falloff_end" => Ok(Self::FalloffEnd),
            "light_melee_damage" => Ok(Self::LightMeleeDamage),
            "heavy_melee_damage" => Ok(Self::HeavyMeleeDamage),
            "slide_distance" => Ok(Self::SlideDistance),
            "bullet_evasion" => Ok(Self::BulletEvasion),
            "debuff_resist" => Ok(Self::DebuffResist),
            "weapon_damage" => Ok(Self::WeaponDamage),
            "melee_resist" => Ok(Self::MeleeResist),
            "spirit_resist" => Ok(Self::SpiritResist),
            "bullet_resist" => Ok(Self::BulletResist),
            "melee_lifesteal" => Ok(Self::MeleeLifesteal),
            "spirit_lifesteal" => Ok(Self::SpiritLifesteal),
            "bullet_lifesteal" => Ok(Self::BulletLifesteal),
            "gravity_scale" => Ok(Self::GravityScale),
            "stamina" => Ok(Self::Stamina),
            "stamina_cooldown" => Ok(Self::StaminaCooldown),
            "dash_speed" => Ok(Self::DashSpeed),
            "dash_duration" => Ok(Self::DashDuration),
            "air_dash_speed" => Ok(Self::AirDashSpeed),
            "air_dash_duration" => Ok(Self::AirDashDuration),
            "move_speed" => Ok(Self::MoveSpeed),
            "sprint_speed" => Ok(Self::SprintSpeed),
            _ => Err(CalculationError::Invalid(format!(
                "unsupported hero stat {value}"
            ))),
        }
    }
}

/// Which effects enter the calculation. Stat units and equations do not change.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatMode {
    /// Include supported active effects as well as baseline inputs.
    #[default]
    Current,
    /// Include hero values, owned passive effects and permanent recorded changes.
    Baseline,
}

impl std::str::FromStr for StatMode {
    type Err = CalculationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "current" => Ok(Self::Current),
            "baseline" => Ok(Self::Baseline),
            _ => Err(CalculationError::Invalid(format!(
                "unsupported stat mode {value}; use current or baseline"
            ))),
        }
    }
}

/// Selected equations. Rules are independent of catalog client versions.
#[derive(Clone, Debug, Default)]
pub struct Ruleset {
    rules: HashMap<HeroStat, Rule>,
}
impl Ruleset {
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn with(mut self, rule: Rule) -> Self {
        self.rules.insert(rule.stat, rule);
        self
    }
}

/// Select ticks and players without loading complete per-tick datasets.
#[derive(Clone, Debug)]
pub struct HeroStatQuery {
    ticks: Vec<i32>,
    stats: Vec<HeroStat>,
    mode: StatMode,
    steam_ids: Option<Vec<u64>>,
    heroes: Option<Vec<i64>>,
    explain: bool,
    strict: bool,
}
impl HeroStatQuery {
    pub fn new(
        ticks: impl IntoIterator<Item = i32>,
        stats: impl IntoIterator<Item = HeroStat>,
    ) -> Self {
        let mut stats: Vec<_> = stats.into_iter().collect();
        stats.sort_unstable();
        stats.dedup();
        Self {
            ticks: ticks.into_iter().collect(),
            stats,
            mode: StatMode::default(),
            steam_ids: None,
            heroes: None,
            explain: false,
            strict: true,
        }
    }
    /// Select baseline or current effects, including inputs to spirit scaling.
    #[must_use]
    pub fn mode(mut self, mode: StatMode) -> Self {
        self.mode = mode;
        self
    }
    #[must_use]
    pub fn steam_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.steam_ids = Some(ids.into_iter().collect());
        self
    }
    #[must_use]
    pub fn heroes(mut self, heroes: impl IntoIterator<Item = i64>) -> Self {
        self.heroes = Some(heroes.into_iter().collect());
        self
    }
    #[must_use]
    pub fn explain(mut self, explain: bool) -> Self {
        self.explain = explain;
        self
    }
    /// If false, unresolved rows have null values and an explanatory diagnostic.
    /// Skipped modifiers and inferred activation produce partial rows with either strict setting.
    #[must_use]
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}

/// One calculated stat; `value` is null if its inputs could not be resolved.
#[derive(Clone, Debug, Serialize)]
pub struct StatRow {
    pub mode: StatMode,
    pub tick: i32,
    /// Recorded Steam account ID; absent for players without an account.
    pub steam_id: Option<u64>,
    pub hero_id: i64,
    pub stat: HeroStat,
    pub value: Option<f64>,
    pub unit: &'static str,
    pub ruleset: &'static str,
    /// `calculated`, `partial` (skipped/unmapped inputs or inferred activation), or `unresolved`.
    pub status: &'static str,
    pub diagnostic: Option<String>,
}

/// One resolved input, including intermediate spirit-power inputs when needed.
#[derive(Clone, Debug, Serialize)]
pub struct Contribution {
    pub mode: StatMode,
    pub tick: i32,
    /// Recorded Steam account ID; absent for players without an account.
    pub steam_id: Option<u64>,
    pub hero_id: i64,
    /// Calculated stat this input contributes to, including intermediate inputs.
    pub stat: HeroStat,
    pub input: String,
    pub kind: &'static str,
    pub value: f64,
    pub source: String,
    pub definition_path: String,
    pub modifier_serial: Option<u32>,
}

/// Provenance sufficient to identify the selected catalogs and equation.
#[derive(Clone, Debug, Serialize)]
pub struct CalculationMetadata {
    pub mode: StatMode,
    pub data_version: String,
    pub snapshot_version: String,
    pub source_commit: String,
    pub rulesets: Vec<Rule>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatResult {
    pub values: Vec<StatRow>,
    pub contributions: Vec<Contribution>,
    pub metadata: CalculationMetadata,
}

impl StatResult {
    fn prepare(
        query: &HeroStatQuery,
        catalog: &StatCatalog,
        rules: &Ruleset,
    ) -> Result<Self, CalculationError> {
        validate_ticks(&query.ticks)?;
        if query.stats.is_empty() {
            return Err(CalculationError::Invalid(
                "provide at least one stat".into(),
            ));
        }
        for stat in &query.stats {
            if rules.rules.get(stat) != Some(&stat.rule()) {
                return Err(CalculationError::Invalid(format!(
                    "select {} for {}",
                    stat.rule().id,
                    stat.as_str()
                )));
            }
        }
        Ok(Self {
            values: Vec::new(),
            contributions: Vec::new(),
            metadata: CalculationMetadata {
                mode: query.mode,
                data_version: catalog.data_version.clone(),
                snapshot_version: catalog.snapshot_version.clone(),
                source_commit: catalog.source_commit.clone(),
                rulesets: query.stats.iter().map(|stat| stat.rule()).collect(),
            },
        })
    }

    fn finish(&mut self) {
        self.values
            .sort_by_key(|row| (row.tick, row.steam_id, row.hero_id, row.stat));
    }
}

fn validate_ticks(ticks: &[i32]) -> Result<(), CalculationError> {
    if ticks.is_empty() || ticks.iter().any(|tick| *tick < 0) {
        return Err(CalculationError::Invalid(
            "provide nonnegative ticks".into(),
        ));
    }
    Ok(())
}

impl Parser {
    /// Calculate selected hero stats after each requested demo tick.
    ///
    /// # Errors
    /// Invalid queries and missing ticks always fail. With strict mode (default),
    /// missing/ambiguous stat inputs also fail. Otherwise those rows are null.
    /// Skipped/unmapped inputs and inferred activation produce partial rows with diagnostics.
    pub fn calculate_hero_stats(
        &self,
        query: &HeroStatQuery,
        catalog: &StatCatalog,
        rules: &Ruleset,
    ) -> Result<StatResult, CalculationError> {
        let mut result = StatResult::prepare(query, catalog, rules)?;
        self.visit_stat_ticks(&query.ticks, catalog, |ctx, modifiers| {
            inputs::collect(ctx, query, catalog, modifiers, &mut result)
        })?;
        result.finish();
        Ok(result)
    }
    pub(crate) fn visit_stat_ticks(
        &self,
        requested: &[i32],
        catalog: &StatCatalog,
        visit: impl FnMut(
            &crate::Context,
            &crate::EffectiveModifierState,
        ) -> Result<(), CalculationError>,
    ) -> Result<(), CalculationError> {
        self.visit_stat_ticks_with_replay(requested, catalog, None, visit)
    }

    pub(crate) fn visit_stat_ticks_with_replay(
        &self,
        requested: &[i32],
        catalog: &StatCatalog,
        replay: Option<&StatReplay>,
        mut visit: impl FnMut(
            &crate::Context,
            &crate::EffectiveModifierState,
        ) -> Result<(), CalculationError>,
    ) -> Result<(), CalculationError> {
        if let Some(replay) = replay {
            replay.validate(self, catalog)?;
        }
        validate_ticks(requested)?;
        let mut ticks = requested.to_vec();
        ticks.sort_unstable();
        ticks.dedup();
        let available = self.distinct_ticks()?;
        for tick in &ticks {
            if available.binary_search(tick).is_err() {
                return Err(CalculationError::Invalid(format!(
                    "demo has no tick {tick}"
                )));
            }
        }
        let initial = self.parse_init()?;
        let classes = initial.serializers().iter().map(|(name, _)| name).collect();
        let mut failure = None;
        let mut seen = std::collections::HashSet::new();
        let end = ticks
            .last()
            .and_then(|t| t.checked_add(1))
            .ok_or_else(|| CalculationError::Invalid("tick is too large".into()))?;
        let checkpoint = replay.and_then(|r| r.before(ticks[0]));
        let mut modifiers = checkpoint.map_or_else(
            || crate::EffectiveModifierState::with_catalog(catalog),
            |c| c.modifiers.clone(),
        );
        let clock = crate::ModifierClock::resolve(&initial);
        if checkpoint.is_none() {
            modifiers.rebuild(&initial, clock.game_time(&initial));
        }
        self.stat_checkpoint(checkpoint.map(|c| &c.playback), end - 1, &classes, |ctx| {
            modifiers.update(ctx, clock.game_time(ctx));
            if failure.is_some()
                || ticks.binary_search(&ctx.tick()).is_err()
                || !seen.insert(ctx.tick())
            {
                return;
            }
            if let Err(error) = visit(ctx, &modifiers) {
                failure = Some(error);
            }
        })?;
        if let Some(error) = failure {
            return Err(error);
        }
        if seen.len() != ticks.len() {
            return Err(CalculationError::Invalid(
                "requested ticks were not decoded".into(),
            ));
        }
        Ok(())
    }
}
