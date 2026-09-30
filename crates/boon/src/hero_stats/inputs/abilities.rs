//! Ability-scoped percentages and recorded item imbues.
//! Balance values and targeting filters come from the selected catalog.
use super::*;
use crate::Parser;
use serde::Serialize;

/// Percentage stats, separate from cooldown timers and individual VData properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityStat {
    CooldownReduction,
    ItemCooldownReduction,
    DurationBonus,
    RangeBonus,
    RadiusBonus,
}
impl AbilityStat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CooldownReduction => "cooldown_reduction",
            Self::ItemCooldownReduction => "item_cooldown_reduction",
            Self::DurationBonus => "duration_bonus",
            Self::RangeBonus => "range_bonus",
            Self::RadiusBonus => "radius_bonus",
        }
    }
    fn symbol(self) -> &'static str {
        match self {
            Self::CooldownReduction => "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE",
            Self::ItemCooldownReduction => "MODIFIER_VALUE_ITEM_COOLDOWN_REDUCTION_PERCENTAGE",
            Self::DurationBonus => "MODIFIER_VALUE_BONUS_ABILITY_DURATION_PERCENTAGE",
            Self::RangeBonus => "MODIFIER_VALUE_TECH_RANGE_PERCENT",
            Self::RadiusBonus => "MODIFIER_VALUE_TECH_RADIUS_PERCENT",
        }
    }
    fn from_symbol(symbol: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|stat| stat.symbol() == symbol)
            .or_else(|| {
                (symbol == "MODIFIER_VALUE_ULTIMATE_COOLDOWN_REDUCTION_PERCENTAGE")
                    .then_some(Self::CooldownReduction)
            })
    }
    pub const ALL: [Self; 5] = [
        Self::CooldownReduction,
        Self::ItemCooldownReduction,
        Self::DurationBonus,
        Self::RangeBonus,
        Self::RadiusBonus,
    ];
    pub const fn rule(self) -> AbilityRule {
        AbilityRule {
            stat: self,
            id: match self {
                Self::CooldownReduction => "cooldown_reduction.v1",
                Self::ItemCooldownReduction => "item_cooldown_reduction.v1",
                Self::DurationBonus => "duration_bonus.v1",
                Self::RangeBonus => "range_bonus.v1",
                Self::RadiusBonus => "radius_bonus.v1",
            },
            version: 1,
            documented_on: "2026-09-28",
        }
    }
}
impl std::str::FromStr for AbilityStat {
    type Err = CalculationError;
    fn from_str(value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|stat| stat.as_str() == value)
            .ok_or_else(|| invalid(format!("unsupported ability stat {value}")))
    }
}

/// Equation identity; independent of the catalog version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AbilityRule {
    pub stat: AbilityStat,
    pub id: &'static str,
    pub version: u32,
    pub documented_on: &'static str,
}
/// Explicit equation selection for the requested ability stats.
#[derive(Clone, Debug, Default)]
pub struct AbilityRuleset(BTreeMap<AbilityStat, AbilityRule>);
impl AbilityRuleset {
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn with(mut self, rule: AbilityRule) -> Self {
        self.0.insert(rule.stat, rule);
        self
    }
}

/// Select replay state after each requested tick. Select players by Steam ID.
#[derive(Clone, Debug)]
pub struct ImbueQuery {
    ticks: Vec<i32>,
    steam_ids: Option<Vec<u64>>,
}
impl ImbueQuery {
    pub fn new(ticks: impl IntoIterator<Item = i32>) -> Self {
        Self {
            ticks: ticks.into_iter().collect(),
            steam_ids: None,
        }
    }
    #[must_use]
    pub fn steam_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.steam_ids = Some(ids.into_iter().collect());
        self
    }
}
/// Defaults to the current hero's signature abilities. Explicit IDs can select items.
#[derive(Clone, Debug)]
pub struct AbilityStatQuery {
    selection: ImbueQuery,
    stats: Vec<AbilityStat>,
    mode: StatMode,
    abilities: Option<Vec<u32>>,
    include_items: bool,
    explain: bool,
    strict: bool,
}
impl AbilityStatQuery {
    pub fn new(
        ticks: impl IntoIterator<Item = i32>,
        stats: impl IntoIterator<Item = AbilityStat>,
    ) -> Self {
        let mut stats: Vec<_> = stats.into_iter().collect();
        stats.sort_unstable();
        stats.dedup();
        Self {
            selection: ImbueQuery::new(ticks),
            stats,
            mode: StatMode::default(),
            abilities: None,
            include_items: false,
            explain: false,
            strict: true,
        }
    }
    /// Select passive and permanent inputs, or include supported active effects.
    #[must_use]
    pub fn mode(mut self, mode: StatMode) -> Self {
        self.mode = mode;
        self
    }
    #[must_use]
    pub fn steam_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.selection = self.selection.steam_ids(ids);
        self
    }
    #[must_use]
    pub fn abilities(mut self, abilities: impl IntoIterator<Item = u32>) -> Self {
        self.abilities = Some(abilities.into_iter().collect());
        self
    }
    #[must_use]
    pub fn include_items(mut self, include: bool) -> Self {
        self.include_items = include;
        self
    }
    #[must_use]
    pub fn explain(mut self, explain: bool) -> Self {
        self.explain = explain;
        self
    }
    #[must_use]
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct AbilityMetadata {
    /// Selected calculation mode. Absent for recorded imbue queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<StatMode>,
    pub data_version: String,
    pub snapshot_version: String,
    pub source_commit: String,
    pub rulesets: Vec<AbilityRule>,
}
impl AbilityMetadata {
    fn new(catalog: &StatCatalog, stats: &[AbilityStat], mode: Option<StatMode>) -> Self {
        Self {
            mode,
            data_version: catalog.data_version.clone(),
            snapshot_version: catalog.snapshot_version.clone(),
            source_commit: catalog.source_commit.clone(),
            rulesets: stats.iter().map(|s| s.rule()).collect(),
        }
    }
}
/// A recorded relation survives even when either catalog lookup is unavailable.
#[derive(Clone, Debug, Serialize)]
pub struct ImbueBinding {
    pub tick: i32,
    /// Recorded Steam account ID; absent for players without an account.
    pub steam_id: Option<u64>,
    pub hero_id: i64,
    pub item_id: u32,
    pub item_name: Option<String>,
    pub ability_id: u32,
    pub ability_name: Option<String>,
    pub status: &'static str,
    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ImbueEffect {
    #[serde(flatten)]
    pub binding: ImbueBinding,
    pub stat: Option<String>,
    pub property_name: Option<String>,
    pub value: Option<f64>,
    pub unit: Option<&'static str>,
    pub apply_filter: String,
    pub source: String,
    pub definition_path: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ImbueResult {
    pub bindings: Vec<ImbueBinding>,
    pub effects: Vec<ImbueEffect>,
    pub metadata: AbilityMetadata,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectScope {
    Global,
    Imbued,
    Charged,
    Ultimate,
    NextCast,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectState {
    Active,
    Ready,
    Inactive,
    Unresolved,
}
#[derive(Clone, Debug, Serialize)]
pub struct AbilityContribution {
    pub mode: StatMode,
    pub tick: i32,
    /// Recorded Steam account ID; absent for players without an account.
    pub steam_id: Option<u64>,
    pub hero_id: i64,
    pub ability_id: u32,
    pub stat: AbilityStat,
    pub value: Option<f64>,
    pub source: String,
    pub source_ability_id: Option<u32>,
    pub property_name: Option<String>,
    pub definition_path: String,
    pub modifier_serial: Option<u32>,
    pub scope: EffectScope,
    pub state: EffectState,
    pub included: bool,
    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AbilityStatRow {
    pub mode: StatMode,
    pub tick: i32,
    /// Recorded Steam account ID; absent for players without an account.
    pub steam_id: Option<u64>,
    pub hero_id: i64,
    pub ability_id: u32,
    pub ability_name: Option<String>,
    pub stat: AbilityStat,
    pub value: Option<f64>,
    pub unit: &'static str,
    pub ruleset: &'static str,
    pub status: &'static str,
    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AbilityStatResult {
    pub values: Vec<AbilityStatRow>,
    pub contributions: Vec<AbilityContribution>,
    pub metadata: AbilityMetadata,
}

fn display(record: &Record) -> Option<String> {
    record
        .display_name
        .clone()
        .or_else(|| record.ability_name.clone())
}
fn players<'a>(ctx: &'a Context, selected: Option<&[u64]>) -> Result<Vec<(i64, &'a Entity)>> {
    let mut result = Vec::new();
    for (_, controller) in ctx
        .entities()
        .iter()
        .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
    {
        if selected.is_some_and(|ids| steam_id(ctx, controller).is_none_or(|id| !ids.contains(&id)))
        {
            continue;
        }
        let hero = required(ctx, controller, "m_PlayerDataGlobal.m_nHeroID")? as i64;
        if hero != 0 {
            result.push((hero, controller));
        }
    }
    if let Some(ids) = selected {
        for id in ids {
            if !result
                .iter()
                .any(|(_, controller)| steam_id(ctx, controller) == Some(*id))
            {
                return Err(invalid(format!(
                    "Steam ID {id} has no hero at tick {}",
                    ctx.tick()
                )));
            }
        }
    }
    result.sort_by_key(|(hero, controller)| (steam_id(ctx, controller), *hero));
    Ok(result)
}
fn bindings(ctx: &Context, controller: &Entity) -> Result<BTreeSet<(u32, u32)>> {
    let path = "m_PlayerDataGlobal.m_vecImbuements";
    let mut result = BTreeSet::new();
    for i in 0..count(ctx, controller, path)? {
        let prefix = format!("{path}.{i}");
        let source = id(ctx, controller, &format!("{prefix}.m_SourceItemID"))?;
        let targets = format!("{prefix}.m_vecImbuedAbilities");
        for j in 0..count(ctx, controller, &targets)? {
            result.insert((source, id(ctx, controller, &format!("{targets}.{j}"))?));
        }
    }
    Ok(result)
}
/// Recover filters from older JSON artifacts whose raw definition already has them.
pub(super) fn apply_filter<'a>(
    catalog: &'a StatCatalog,
    source: &'a Record,
    effect: &'a Value,
) -> Option<&'a str> {
    effect
        .get("apply_filter")
        .and_then(Value::as_str)
        .or_else(|| {
            let owner = source
                .ability_id
                .and_then(|id| catalog.abilities.get(&id))
                .unwrap_or(source);
            let name = effect["property_name"].as_str()?;
            owner.definition["m_mapAbilityProperties"][name]["m_eApplyFilter"].as_str()
        })
}
pub(super) fn scope(filter: Option<&str>) -> EffectScope {
    match filter {
        None | Some("" | "EApplyFilter_None") => EffectScope::Global,
        Some("EApplyFilter_OnlyIfImbued") => EffectScope::Imbued,
        Some("EApplyFilter_OnlyIfHasCharges") => EffectScope::Charged,
        _ => EffectScope::Unknown,
    }
}

impl Parser {
    /// Read item-to-ability selections and resolve their catalog effects.
    /// Missing effect definitions remain visible with diagnostics.
    /// # Errors
    /// Invalid ticks, players, or missing imbue fields return an error.
    pub fn imbues(&self, query: &ImbueQuery, catalog: &StatCatalog) -> Result<ImbueResult> {
        let mut result = ImbueResult {
            bindings: Vec::new(),
            effects: Vec::new(),
            metadata: AbilityMetadata::new(catalog, &[], None),
        };
        self.visit_stat_ticks(&query.ticks, catalog, |ctx, _| {
            for (hero, controller) in players(ctx, query.steam_ids.as_deref())? {
                let mut scratch = Vec::new();
                let resolver = resolver(ctx, catalog, controller, hero, &mut scratch);
                for (item_id, ability_id) in bindings(ctx, controller)? {
                    let item = catalog.abilities.get(&item_id);
                    let ability = catalog.abilities.get(&ability_id);
                    let missing = item.is_none() || ability.is_none();
                    let binding = ImbueBinding {
                        tick: ctx.tick(),
                        steam_id: resolver.steam_id,
                        hero_id: hero,
                        item_id,
                        item_name: item.and_then(display),
                        ability_id,
                        ability_name: ability.and_then(display),
                        status: if missing { "unresolved" } else { "recorded" },
                        diagnostic: missing.then(|| {
                            "imbue is recorded; catalog source or target is missing".into()
                        }),
                    };
                    result.bindings.push(binding.clone());
                    if let Some(item) = item {
                        for effect in &item.stat_changes {
                            let filter = apply_filter(catalog, item, effect);
                            if scope(filter) != EffectScope::Imbued {
                                continue;
                            }
                            let resolved = resolver.effect(effect, item, None);
                            let mut binding = binding.clone();
                            if let Err(error) = &resolved {
                                binding.status = "unresolved";
                                binding.diagnostic = Some(error.to_string());
                            }
                            result.effects.push(ImbueEffect {
                                binding,
                                stat: effect["stat"].as_str().map(str::to_owned),
                                property_name: effect["property_name"].as_str().map(str::to_owned),
                                value: resolved.ok(),
                                unit: effect["stat"]
                                    .as_str()
                                    .and_then(AbilityStat::from_symbol)
                                    .map(|_| "%"),
                                apply_filter: filter.unwrap_or_default().into(),
                                source: item.record_key.clone(),
                                definition_path: effect["definition_path"]
                                    .as_str()
                                    .unwrap_or(&item.definition_path)
                                    .into(),
                            });
                        }
                    }
                }
            }
            Ok(())
        })?;
        Ok(result)
    }

    /// Calculate per-ability percentage bonuses, not remaining timers or metres/seconds.
    /// # Errors
    /// Invalid queries always fail. With strict enabled, missing calculation inputs fail.
    pub fn calculate_ability_stats(
        &self,
        query: &AbilityStatQuery,
        catalog: &StatCatalog,
        rules: &AbilityRuleset,
    ) -> Result<AbilityStatResult> {
        if query.stats.is_empty() {
            return Err(invalid("provide at least one ability stat"));
        }
        for stat in &query.stats {
            if rules.0.get(stat) != Some(&stat.rule()) {
                return Err(invalid(format!("select {}", stat.rule().id)));
            }
        }
        let mut result = AbilityStatResult {
            values: Vec::new(),
            contributions: Vec::new(),
            metadata: AbilityMetadata::new(catalog, &query.stats, Some(query.mode)),
        };
        self.visit_stat_ticks(&query.selection.ticks, catalog, |ctx, modifiers| {
            for (hero, controller) in players(ctx, query.selection.steam_ids.as_deref())? {
                calculate_player(
                    ctx,
                    catalog,
                    modifiers,
                    controller,
                    hero,
                    query,
                    &mut result,
                )?;
            }
            Ok(())
        })?;
        if let Some(ids) = &query.abilities {
            for id in ids {
                if !result.values.iter().any(|row| row.ability_id == *id) {
                    return Err(invalid(format!(
                        "ability {id} is not owned by a selected player"
                    )));
                }
            }
        }
        Ok(result)
    }
}
fn resolver<'a, 'b>(
    ctx: &'a Context,
    catalog: &'a StatCatalog,
    controller: &'a Entity,
    hero_id: i64,
    contributions: &'b mut Vec<Contribution>,
) -> Resolver<'a, 'b> {
    Resolver {
        mode: super::StatMode::Current,
        ctx,
        catalog,
        controller,
        steam_id: steam_id(ctx, controller),
        hero_id,
        game_time: ModifierClock::resolve(ctx).game_time(ctx).map(f64::from),
        game_start: ctx
            .entities()
            .iter()
            .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
            .and_then(|(_, e)| field(ctx, e, "m_pGameRules.m_flGameStartTime")),
        explain: false,
        contributions,
        ignored_modifiers: BTreeMap::new(),
        inferred_bindings: BTreeSet::new(),
        unmapped_inputs: BTreeSet::new(),
    }
}

#[derive(Debug)]
struct Effect {
    stat: AbilityStat,
    source: String,
    source_ability_id: Option<u32>,
    property_name: Option<String>,
    path: String,
    serial: Option<u32>,
    value: Option<f64>,
    scope: EffectScope,
    targets: BTreeSet<u32>,
    excluded_targets: BTreeSet<u32>,
    state: EffectState,
    diagnostic: Option<String>,
    fatal: bool,
}
impl Effect {
    fn catalog(
        resolver: &Resolver<'_, '_>,
        source: &Record,
        definition: &Value,
        entry: Option<&CModifierTableEntry>,
        imbues: &BTreeSet<(u32, u32)>,
    ) -> Option<Self> {
        let symbol = definition["stat"].as_str()?;
        let stat = AbilityStat::from_symbol(symbol)?;
        if empty_declaration(source, definition) || !resolver.mode.includes_effect(definition) {
            return None;
        }
        let value = resolver.effect(definition, source, entry);
        let mut scope = scope(apply_filter(resolver.catalog, source, definition));
        if symbol == "MODIFIER_VALUE_ULTIMATE_COOLDOWN_REDUCTION_PERCENTAGE" {
            scope = EffectScope::Ultimate;
        }
        let targets = imbues
            .iter()
            .filter_map(|(item, target)| (Some(*item) == source.ability_id).then_some(*target))
            .collect();
        let corruption = resolver.corruption_diagnostic(definition, source, entry);
        let fatal = value.is_err() || corruption.is_err();
        let diagnostic = value
            .as_ref()
            .err()
            .map(ToString::to_string)
            .or_else(|| corruption.unwrap_or_else(|error| Some(error.to_string())));
        Some(Self {
            stat,
            source: source.record_key.clone(),
            source_ability_id: source.ability_id,
            property_name: definition["property_name"].as_str().map(str::to_owned),
            path: definition["definition_path"]
                .as_str()
                .unwrap_or(&source.definition_path)
                .into(),
            serial: entry.and_then(|e| e.serial_number),
            value: value.ok(),
            scope,
            targets,
            excluded_targets: BTreeSet::new(),
            state: if fatal {
                EffectState::Unresolved
            } else {
                EffectState::Active
            },
            fatal,
            diagnostic,
        })
    }
    fn conditional(&mut self, state: EffectState, message: impl Into<String>) {
        self.state = state;
        self.diagnostic = Some(message.into());
        self.fatal = false;
    }
}

/// A next-cast window is a semantic interpretation of these exact structural
/// fields, not a claim that VData defines engine activation logic. Keep the
/// inference in diagnostics and never spread a ready bonus across all abilities.
fn cast_modifiers<'a>(
    catalog: &'a StatCatalog,
    owner: &Record,
) -> Option<(&'a Record, &'a Record)> {
    for modifier in &catalog.modifiers {
        if !modifier.is_intrinsic_modifier_of(owner) {
            continue;
        }
        if modifier.definition.get("m_SurgeWindowModifier").is_none()
            || modifier
                .definition
                .get("m_AbilityWatcherModifier")
                .is_none()
        {
            continue;
        }
        let ready_path = format!("{}/m_SurgeWindowModifier", modifier.definition_path);
        let applied_path = format!("{}/m_AbilityWatcherModifier", modifier.definition_path);
        let find = |path: &str| {
            catalog
                .modifiers
                .iter()
                .find(|m| m.ability_id == owner.ability_id && m.definition_path == path)
        };
        if let (Some(ready), Some(applied)) = (find(&ready_path), find(&applied_path)) {
            return Some((ready, applied));
        }
    }
    None
}
fn active_record(entry: &CModifierTableEntry, record: &Record) -> bool {
    entry.modifier_subclass.is_some_and(|id| {
        Some(id) == record.modifier_id || Some(id) == record.qualified_modifier_id
    }) && entry
        .ability_subclass
        .is_none_or(|id| id == 0 || record.ability_id == Some(id))
}

fn inferred_state(
    entries: &[&CModifierTableEntry],
    modifier: &Record,
    has_clock: bool,
) -> EffectState {
    let mut state = EffectState::Inactive;
    for entry in entries
        .iter()
        .filter(|entry| active_record(entry, modifier))
    {
        if !has_clock
            || entry.last_applied_time.is_none()
            || !entry.duration.is_some_and(|d| d.is_finite() && d > 0.0)
        {
            return EffectState::Unresolved;
        }
        state = EffectState::Active;
    }
    state
}

/// Runtime values need their own role evidence; a target alone is not proof
/// of a persistent imbue. Keep ambiguous values unresolved so they cannot be
/// replaced by a plausible but unverified catalog default during deduplication.
#[derive(Debug, Eq, PartialEq)]
enum DynamicRole {
    Passive,
    Temporary,
    Unknown,
}

fn dynamic_role(catalog: &StatCatalog, owner: &Record, property: Option<&Value>) -> DynamicRole {
    let Some(property) = property else {
        return DynamicRole::Unknown;
    };
    if !StatMode::Baseline.includes_effect(property) {
        return DynamicRole::Temporary;
    }
    if let Some(keys) = property["modifier_keys"]
        .as_array()
        .filter(|keys| !keys.is_empty())
    {
        let mut intrinsic = false;
        let mut temporary = false;
        for key in keys {
            let Some(modifier) = catalog
                .modifiers
                .iter()
                .find(|modifier| key.as_str() == Some(&modifier.record_key))
            else {
                return DynamicRole::Unknown;
            };
            if modifier.is_intrinsic_modifier_of(owner) {
                intrinsic = true;
            } else {
                temporary = true;
            }
        }
        return match (intrinsic, temporary) {
            (true, false) => DynamicRole::Passive,
            (false, true) => DynamicRole::Temporary,
            _ => DynamicRole::Unknown,
        };
    }
    if property["usage_flags"]
        .as_str()
        .is_some_and(|flags| flags.contains("IntrinsicallyProvidedInAbility"))
    {
        DynamicRole::Passive
    } else if cast_modifiers(catalog, owner).is_some() {
        DynamicRole::Temporary
    } else {
        DynamicRole::Unknown
    }
}

fn dynamic_effects(resolver: &Resolver<'_, '_>, inputs: &PlayerInputs<'_>) -> Result<Vec<Effect>> {
    let path = "m_PlayerDataGlobal.m_vecDynamicAbilityValues";
    let mut result = Vec::new();
    for i in 0..count(resolver.ctx, resolver.controller, path)? {
        let prefix = format!("{path}.{i}");
        let owner_id = id(
            resolver.ctx,
            resolver.controller,
            &format!("{prefix}.m_SourceAbilityID"),
        )?;
        let kind = id(
            resolver.ctx,
            resolver.controller,
            &format!("{prefix}.m_eValType"),
        )?;
        let value = required(
            resolver.ctx,
            resolver.controller,
            &format!("{prefix}.m_flValue"),
        )?;
        let symbol = resolver.catalog.modifier_value_types.get(&kind).ok_or_else(|| invalid(format!("catalog has no network stat type {kind}; install a catalog with modifier_value_types using `boon get VERSION --force`")))?;
        let Some(stat) = AbilityStat::from_symbol(symbol) else {
            continue;
        };
        let owner = resolver
            .catalog
            .abilities
            .get(&owner_id)
            .ok_or_else(|| invalid(format!("missing dynamic ability source {owner_id}")))?;
        let vector = format!("{prefix}.m_vecImbuedAbilities");
        let targets: BTreeSet<_> = (0..count(resolver.ctx, resolver.controller, &vector)?)
            .map(|j| id(resolver.ctx, resolver.controller, &format!("{vector}.{j}")))
            .collect::<Result<_>>()?;
        let properties: Vec<_> = owner
            .stat_changes
            .iter()
            .filter(|e| e["stat"] == *symbol)
            .collect();
        let property = match properties.as_slice() {
            [one] => Some(*one),
            // A runtime stat type does not identify which of several same-stat
            // properties supplied it. Baseline must not guess a persistent role.
            _ if resolver.mode == StatMode::Baseline => None,
            _ => {
                let mut conditional = properties.iter().filter(|e| {
                    e["usage_flags"]
                        .as_str()
                        .is_some_and(|s| s.contains("ConditionallyApplied"))
                });
                let first = conditional.next().copied();
                if conditional.next().is_none() {
                    first
                } else {
                    None
                }
            }
        };
        let mut effect = Effect {
            stat,
            source: owner.record_key.clone(),
            source_ability_id: Some(owner_id),
            property_name: property
                .and_then(|e| e["property_name"].as_str())
                .map(str::to_owned),
            path: format!("{prefix}.m_flValue"),
            serial: None,
            value: Some(value),
            scope: if targets.is_empty() {
                property.map_or(EffectScope::Global, |p| {
                    scope(apply_filter(resolver.catalog, owner, p))
                })
            } else {
                EffectScope::Imbued
            },
            targets,
            excluded_targets: BTreeSet::new(),
            state: EffectState::Active,
            diagnostic: None,
            fatal: false,
        };
        if symbol == "MODIFIER_VALUE_ULTIMATE_COOLDOWN_REDUCTION_PERCENTAGE" {
            effect.scope = EffectScope::Ultimate;
        }
        if resolver.mode == StatMode::Baseline {
            if !inputs
                .owned
                .iter()
                .any(|record| record.ability_id == Some(owner_id))
            {
                continue;
            }
            match dynamic_role(resolver.catalog, owner, property) {
                DynamicRole::Temporary => continue,
                DynamicRole::Passive => {}
                DynamicRole::Unknown => effect.conditional(
                    EffectState::Unresolved,
                    "baseline excludes this dynamic value; its passive or active role is unknown",
                ),
            }
        }
        if properties.len() > 1 && property.is_none() {
            effect.conditional(
                EffectState::Unresolved,
                "dynamic source has several matching properties; cannot deduplicate safely",
            );
        }
        if resolver.mode == StatMode::Current
            && effect.targets.is_empty()
            && value != 0.0
            && let Some((ready, _)) = cast_modifiers(resolver.catalog, owner)
        {
            effect.scope = EffectScope::NextCast;
            effect.conditional(
                if inputs.active.iter().any(|e| active_record(e, ready)) {
                    EffectState::Ready
                } else {
                    EffectState::Unresolved
                },
                "inferred next-cast window; no recorded target ability",
            );
        }
        if result.iter().any(|prior: &Effect| {
            prior.source_ability_id == effect.source_ability_id
                && prior.stat == effect.stat
                && (prior.targets.is_empty()
                    || effect.targets.is_empty()
                    || !prior.targets.is_disjoint(&effect.targets))
        }) {
            effect.conditional(
                EffectState::Unresolved,
                "overlapping dynamic values for one source and stat",
            );
            for prior in &mut result {
                if prior.source_ability_id == effect.source_ability_id && prior.stat == effect.stat
                {
                    prior.conditional(
                        EffectState::Unresolved,
                        "overlapping dynamic values for one source and stat",
                    );
                }
            }
        }
        result.push(effect);
    }
    Ok(result)
}
fn replaced_by_runtime(effect: &mut Effect, dynamic: &[Effect]) -> bool {
    for recorded in dynamic.iter().filter(|d| {
        d.source_ability_id == effect.source_ability_id
            && d.stat == effect.stat
            && (d.property_name.is_none() || d.property_name == effect.property_name)
    }) {
        if recorded.targets.is_empty() {
            return true;
        }
        effect.excluded_targets.extend(&recorded.targets);
    }
    false
}

fn effects(
    resolver: &mut Resolver<'_, '_>,
    inputs: &PlayerInputs<'_>,
    imbues: &BTreeSet<(u32, u32)>,
) -> Result<Vec<Effect>> {
    let dynamic = dynamic_effects(resolver, inputs)?;
    catalog_effects(resolver, inputs, imbues, dynamic)
}

fn catalog_effects(
    resolver: &mut Resolver<'_, '_>,
    inputs: &PlayerInputs<'_>,
    imbues: &BTreeSet<(u32, u32)>,
    mut result: Vec<Effect>,
) -> Result<Vec<Effect>> {
    let dynamic_len = result.len();
    // Accumulated values already include repeated permanent pickups. Never
    // exponentiate a pickup count or add the pickup's live modifier again.
    for recorded in &inputs.permanent {
        let Some(stat) = resolver
            .recorded_stat(recorded)?
            .and_then(AbilityStat::from_symbol)
        else {
            continue;
        };
        let source = resolver.recorded_source(recorded.source_id);
        result.push(Effect {
            stat,
            source,
            source_ability_id: None,
            property_name: None,
            path: "m_PlayerDataGlobal.m_vecStatViewerModifierValues".into(),
            serial: None,
            value: Some(recorded.value),
            scope: EffectScope::Global,
            targets: BTreeSet::new(),
            excluded_targets: BTreeSet::new(),
            state: EffectState::Active,
            diagnostic: None,
            fatal: false,
        });
    }
    let mut bound = HashSet::new();
    let mut seen_properties = HashSet::new();
    for entry in &inputs.active {
        let Some(source) = entry
            .modifier_subclass
            .and_then(|id| resolver.modifier(id, entry.ability_subclass))
        else {
            continue;
        };
        if source
            .misc_id
            .and_then(|id| resolver.catalog.misc.get(&id))
            .is_some_and(|r| r.definition["m_bIsPermanentPickup"] == true)
        {
            continue;
        }
        for definition in &source.stat_changes {
            let path = definition["definition_path"]
                .as_str()
                .unwrap_or(&source.definition_path);
            if !seen_properties.insert((entry.serial_number, &source.record_key, path)) {
                continue;
            }
            let Some(mut effect) =
                Effect::catalog(resolver, source, definition, Some(entry), imbues)
            else {
                continue;
            };
            bound.insert((source.ability_id, effect.property_name.clone()));
            if replaced_by_runtime(&mut effect, &result[..dynamic_len]) {
                continue;
            }
            if entry.duration.is_some_and(|d| d > 0.0)
                && (resolver.game_time.is_none() || entry.last_applied_time.is_none())
            {
                effect.value = None;
                effect.state = EffectState::Unresolved;
                effect.fatal = true;
                effect.diagnostic = Some("cannot determine modifier expiry".into());
            }
            if let Some(counter) = definition["runtime_count"].as_str() {
                match resolver.runtime_count(source, entry, counter) {
                    Ok(count) => effect.value = effect.value.map(|value| value * count),
                    Err(error) => {
                        effect.value = None;
                        effect.state = EffectState::Unresolved;
                        effect.fatal = true;
                        effect.diagnostic = Some(error.to_string());
                    }
                }
            } else if entry.stack_count.is_some_and(|n| n > 1) {
                effect.conditional(
                    EffectState::Unresolved,
                    "modifier stack semantics are not defined",
                );
            }
            result.push(effect);
        }
    }
    for owner in &inputs.owned {
        for definition in &owner.stat_changes {
            let Some(mut effect) = Effect::catalog(resolver, owner, definition, None, imbues)
            else {
                continue;
            };
            if bound.contains(&(owner.ability_id, effect.property_name.clone()))
                || replaced_by_runtime(&mut effect, &result[..dynamic_len])
            {
                continue;
            }
            let has_binding = definition["modifier_keys"]
                .as_array()
                .is_some_and(|k| !k.is_empty());
            if has_binding {
                let missing_intrinsic = definition["modifier_keys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|key| {
                        resolver
                            .catalog
                            .modifiers
                            .iter()
                            .any(|m| m.record_key == key && m.is_intrinsic_modifier_of(owner))
                    });
                if missing_intrinsic && effect.value != Some(0.0) {
                    effect.conditional(
                        EffectState::Unresolved,
                        "owned source has no active intrinsic modifier",
                    );
                    result.push(effect);
                }
                continue;
            }
            let flags = definition["usage_flags"].as_str().unwrap_or_default();
            if effect.value == Some(0.0) {
                continue;
            }
            if !flags.contains("IntrinsicallyProvidedInAbility")
                && let Some((ready, applied)) = cast_modifiers(resolver.catalog, owner)
            {
                if resolver.mode == StatMode::Baseline {
                    continue;
                }
                effect.scope = EffectScope::NextCast;
                effect.conditional(if inputs.active.iter().any(|e|active_record(e,ready)) {EffectState::Ready}
                    else if inputs.active.iter().any(|e|active_record(e,applied)) {EffectState::Unresolved}
                    else {EffectState::Inactive}, "inferred next-cast activation; a dynamic ability value is required to identify an applied bonus");
            } else if flags.contains("ConditionallyApplied") {
                if let Some(modifier) = owner
                    .ability_id
                    .and_then(|id| resolver.catalog.conditional_modifier(id))
                {
                    effect.conditional(
                        inferred_state(&inputs.active, modifier, resolver.game_time.is_some()),
                        "inferred activation requires the owner's unique timed effect modifier",
                    );
                } else {
                    effect.conditional(
                        EffectState::Unresolved,
                        "conditional property has no activation binding",
                    );
                }
            } else if !flags.contains("IntrinsicallyProvidedInAbility") {
                effect.conditional(
                    EffectState::Unresolved,
                    "stat property has no modifier binding",
                );
            }
            result.push(effect);
        }
    }
    Ok(result)
}

struct Target<'a> {
    record: &'a Record,
    ultimate: bool,
    item: bool,
}
fn targets<'a>(
    inputs: &PlayerInputs<'a>,
    catalog: &'a StatCatalog,
    query: &AbilityStatQuery,
) -> Result<Vec<Target<'a>>> {
    let mut signature = BTreeMap::new();
    if let Some(bound) = inputs.hero.definition["m_mapBoundAbilities"].as_object() {
        for (slot, name) in bound {
            if slot.starts_with("ESlot_Signature_")
                && let Some(id) = name
                    .as_str()
                    .and_then(|name| catalog.ability_names.get(name))
            {
                signature.insert(*id, slot == "ESlot_Signature_4");
            }
        }
    }
    let mut result = Vec::new();
    for record in &inputs.owned {
        let Some(id) = record.ability_id else {
            continue;
        };
        let item = record.definition["m_eAbilityType"] == "EAbilityType_Item";
        if query.abilities.as_ref().map_or_else(
            || signature.contains_key(&id) || (query.include_items && item),
            |ids| ids.contains(&id),
        ) {
            result.push(Target {
                record,
                ultimate: signature.get(&id).copied().unwrap_or(false),
                item,
            });
        }
    }
    result.sort_by_key(|t| t.record.ability_id);
    // A missing requested ability must not silently disappear. Explicit IDs can
    // select an ability owned by any selected player, not necessarily every one.
    Ok(result)
}
fn eligible(stat: AbilityStat, target: &Target<'_>) -> bool {
    match stat {
        AbilityStat::CooldownReduction => !target.item,
        AbilityStat::ItemCooldownReduction => {
            target.item
                && target.record.definition["m_mapAbilityProperties"]["AbilityCooldown"]["m_subclassScaleFunction"]
                    ["$value"]["m_bFunctionDisabled"]
                    != true
        }
        _ => true,
    }
}

fn applies(effect: &Effect, target: &Target<'_>, resolver: &Resolver<'_, '_>) -> Result<bool> {
    if target
        .record
        .ability_id
        .is_some_and(|id| effect.excluded_targets.contains(&id))
    {
        return Ok(false);
    }
    match effect.scope {
        EffectScope::Global => Ok(true),
        EffectScope::Imbued => Ok(target
            .record
            .ability_id
            .is_some_and(|id| effect.targets.contains(&id))),
        EffectScope::Ultimate => Ok(target.ultimate),
        EffectScope::NextCast => Ok(false),
        EffectScope::Charged => {
            let property = &target.record.definition["m_mapAbilityProperties"]["AbilityCharges"];
            let def = json!({"stat":"", "property_name":"AbilityCharges", "value":number(&property["m_strValue"]),"definition_path":target.record.definition_path});
            // AbilityCharges' scaling encodes charge-count semantics rather than
            // spirit scaling. Only current mode can use a recorded maximum,
            // which may include temporary changes.
            if resolver.mode == StatMode::Current
                && let Ok(entity) = resolver.owned_ability(target.record)
                && let Some(value) = field(resolver.ctx, entity, "m_iMaxAbilityCharges")
            {
                return Ok(value > 0.0);
            }
            resolver
                .effect(&def, target.record, None)
                .map(|value| value > 0.0)
        }
        EffectScope::Unknown => Err(invalid("unsupported catalog apply filter")),
    }
}
#[allow(clippy::too_many_arguments)]
fn calculate_player(
    ctx: &Context,
    catalog: &StatCatalog,
    modifiers: &EffectiveModifierState,
    controller: &Entity,
    hero: i64,
    query: &AbilityStatQuery,
    result: &mut AbilityStatResult,
) -> Result<()> {
    let mut scratch = Vec::new();
    let mut resolver = resolver(ctx, catalog, controller, hero, &mut scratch);
    resolver.mode = query.mode;
    let mut inputs = match PlayerInputs::collect(ctx, catalog, controller, hero, modifiers) {
        Ok(inputs) => inputs,
        Err(error) if query.strict => return Err(error),
        Err(error) => {
            // Selection metadata can survive missing pawn inputs. Keep null rows
            // for known signature abilities rather than claiming zero bonuses.
            let hero_record = catalog
                .heroes
                .get(&hero)
                .ok_or_else(|| invalid(format!("missing hero {hero}")))?;
            let ids: BTreeSet<u32> = if let Some(ids) = &query.abilities {
                ids.iter().copied().collect()
            } else {
                hero_record.definition["m_mapBoundAbilities"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(slot, _)| slot.starts_with("ESlot_Signature_"))
                    .filter_map(|(_, name)| {
                        name.as_str()
                            .and_then(|n| catalog.ability_names.get(n))
                            .copied()
                    })
                    .collect()
            };
            for ability_id in ids {
                for &stat in &query.stats {
                    result.values.push(AbilityStatRow {
                        mode: query.mode,
                        tick: ctx.tick(),
                        steam_id: resolver.steam_id,
                        hero_id: hero,
                        ability_id,
                        ability_name: catalog.abilities.get(&ability_id).and_then(display),
                        stat,
                        value: None,
                        unit: "%",
                        ruleset: stat.rule().id,
                        status: "unresolved",
                        diagnostic: Some(error.to_string()),
                    });
                }
            }
            return Ok(());
        }
    };
    let mode_diagnostics = inputs.select_mode(catalog, query.mode);
    let imbues = bindings(ctx, controller)?;
    let targets = targets(&inputs, catalog, query)?;
    let resolved = effects(&mut resolver, &inputs, &imbues);
    for target in targets {
        let ability_id = target
            .record
            .ability_id
            .ok_or_else(|| invalid("target has no ability ID"))?;
        for &stat in &query.stats {
            let not_applicable = !eligible(stat, &target);
            let mut diagnostics = BTreeSet::new();
            let mut failure = None;
            let mut values = Vec::new();
            if !not_applicable {
                match &resolved {
                    Err(error) => failure = Some(error.to_string()),
                    Ok(effects) => {
                        for effect in effects.iter().filter(|e| e.stat == stat) {
                            let applies = applies(effect, &target, &resolver);
                            let included = applies.as_ref().copied().unwrap_or(false)
                                && effect.state == EffectState::Active;
                            let relevant = applies.as_ref().copied().unwrap_or(true)
                                || effect.scope == EffectScope::NextCast;
                            if !relevant {
                                continue;
                            }
                            if let Err(error) = applies {
                                failure = Some(error.to_string());
                            }
                            if effect.fatal {
                                failure = effect.diagnostic.clone();
                            }
                            if let Some(message) = &effect.diagnostic
                                && effect.state != EffectState::Inactive
                            {
                                diagnostics.insert(format!("{}: {message}", effect.source));
                            }
                            if effect.state == EffectState::Unresolved {
                                diagnostics
                                    .insert(format!("{}: unresolved contribution", effect.source));
                            }
                            if included && let Some(value) = effect.value {
                                values.push(value);
                            }
                            if query.explain {
                                result.contributions.push(AbilityContribution {
                                    mode: query.mode,
                                    tick: ctx.tick(),
                                    steam_id: resolver.steam_id,
                                    hero_id: hero,
                                    ability_id,
                                    stat,
                                    value: effect.value,
                                    source: effect.source.clone(),
                                    source_ability_id: effect.source_ability_id,
                                    property_name: effect.property_name.clone(),
                                    definition_path: effect.path.clone(),
                                    modifier_serial: effect.serial,
                                    scope: effect.scope,
                                    state: effect.state,
                                    included,
                                    diagnostic: effect.diagnostic.clone(),
                                });
                            }
                        }
                    }
                }
                diagnostics.extend(mode_diagnostics.iter().cloned());
                for message in resolver.ignored_modifiers.values() {
                    diagnostics.insert(message.clone());
                }
            }
            let value = if not_applicable || failure.is_some() {
                None
            } else {
                match match stat {
                    AbilityStat::CooldownReduction => {
                        crate::rulesets::cooldown_reduction::calculate(values)
                    }
                    AbilityStat::ItemCooldownReduction => {
                        crate::rulesets::item_cooldown_reduction::calculate(values)
                    }
                    AbilityStat::DurationBonus => {
                        crate::rulesets::duration_bonus::calculate(values)
                    }
                    AbilityStat::RangeBonus => crate::rulesets::range_bonus::calculate(values),
                    AbilityStat::RadiusBonus => crate::rulesets::radius_bonus::calculate(values),
                } {
                    Ok(v) => Some(v),
                    Err(e) => {
                        failure = Some(e.to_string());
                        None
                    }
                }
            };
            if let Some(error) = failure {
                let message = format!(
                    "tick {} player {} ability {ability_id} {}: {error}",
                    ctx.tick(),
                    resolver
                        .steam_id
                        .map_or_else(|| "without a Steam ID".into(), |id| id.to_string()),
                    stat.as_str()
                );
                if query.strict {
                    return Err(invalid(message));
                }
                diagnostics.insert(message);
            }
            result.values.push(AbilityStatRow {
                mode: query.mode,
                tick: ctx.tick(),
                steam_id: resolver.steam_id,
                hero_id: hero,
                ability_id,
                ability_name: display(target.record),
                stat,
                value,
                unit: "%",
                ruleset: stat.rule().id,
                status: if not_applicable {
                    "not_applicable"
                } else if value.is_none() {
                    "unresolved"
                } else if diagnostics.is_empty() {
                    "calculated"
                } else {
                    "partial"
                },
                diagnostic: (!diagnostics.is_empty())
                    .then(|| diagnostics.into_iter().collect::<Vec<_>>().join("; ")),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> StatCatalog {
        let directory = crate::hero_stats::catalog::tests::fixture();
        StatCatalog::from_directory(directory.path()).unwrap()
    }
    fn record(id: u32, definition: Value) -> Record {
        serde_json::from_value(json!({"record_key":format!("abilities#/{id}"),"definition_path":format!("/{id}"),"ability_id":id,"definition":definition,"stat_changes":[]})).unwrap()
    }
    fn bonus(scope: EffectScope, value: f64) -> Effect {
        Effect {
            stat: AbilityStat::RangeBonus,
            source: "test".into(),
            source_ability_id: Some(123),
            property_name: Some("Range".into()),
            path: "test".into(),
            serial: None,
            value: Some(value),
            scope,
            targets: if scope == EffectScope::Imbued {
                BTreeSet::from([123])
            } else {
                BTreeSet::new()
            },
            excluded_targets: BTreeSet::new(),
            state: EffectState::Active,
            diagnostic: None,
            fatal: false,
        }
    }
    #[test]
    fn global_and_imbued_sources_combine_only_on_selected_target() {
        let catalog = catalog();
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let mut scratch = Vec::new();
        let resolver = resolver(&ctx, &catalog, &controller, 999, &mut scratch);
        let a = record(123, json!({}));
        let b = record(456, json!({}));
        let effects = [
            bonus(EffectScope::Global, 10.0),
            bonus(EffectScope::Imbued, 20.0),
        ];
        for (record, expected) in [(&a, 28.0), (&b, 10.0)] {
            let target = Target {
                record,
                item: false,
                ultimate: false,
            };
            let values = effects
                .iter()
                .filter(|e| applies(e, &target, &resolver).unwrap())
                .filter_map(|e| e.value);
            assert_eq!(
                crate::rulesets::range_bonus::calculate(values).unwrap(),
                expected
            );
        }
        assert!(
            !applies(
                &bonus(EffectScope::NextCast, 12.0),
                &Target {
                    record: &a,
                    item: false,
                    ultimate: false
                },
                &resolver
            )
            .unwrap()
        );
        assert!(
            applies(
                &bonus(EffectScope::Unknown, 12.0),
                &Target {
                    record: &a,
                    item: false,
                    ultimate: false
                },
                &resolver
            )
            .is_err()
        );
    }
    #[test]
    fn charged_filter_uses_catalog_values_and_ultimate_is_separate() {
        let catalog = catalog();
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let mut scratch = Vec::new();
        let resolver = resolver(&ctx, &catalog, &controller, 999, &mut scratch);
        for (charges, expected) in [(0, false), (1, true), (3, true)] {
            let record = record(
                456,
                json!({"m_mapAbilityProperties":{"AbilityCharges":{"m_strValue":charges.to_string()}}}),
            );
            let target = Target {
                record: &record,
                item: false,
                ultimate: false,
            };
            assert_eq!(
                applies(&bonus(EffectScope::Charged, 9.0), &target, &resolver).unwrap(),
                expected
            );
            assert!(!applies(&bonus(EffectScope::Ultimate, 9.0), &target, &resolver).unwrap());
        }
    }
    #[test]
    fn raw_filters_work_for_old_catalogs_and_unknown_filters_are_not_global() {
        let catalog = catalog();
        let item = record(
            456,
            json!({"m_mapAbilityProperties":{"Range":{"m_eApplyFilter":"EApplyFilter_OnlyIfImbued"}}}),
        );
        let property = json!({"property_name":"Range"});
        assert_eq!(
            scope(apply_filter(&catalog, &item, &property)),
            EffectScope::Imbued
        );
        assert_eq!(scope(Some("future_filter")), EffectScope::Unknown);
    }
    #[test]
    fn item_cooldown_does_not_affect_hero_skills_or_disabled_scaling() {
        let ordinary = record(123, json!({}));
        let disabled = record(
            456,
            json!({"m_mapAbilityProperties":{"AbilityCooldown":{"m_subclassScaleFunction":{"$value":{"m_bFunctionDisabled":true}}}}}),
        );
        assert!(!eligible(
            AbilityStat::ItemCooldownReduction,
            &Target {
                record: &ordinary,
                item: false,
                ultimate: false
            }
        ));
        assert!(!eligible(
            AbilityStat::CooldownReduction,
            &Target {
                record: &ordinary,
                item: true,
                ultimate: false
            }
        ));
        assert!(!eligible(
            AbilityStat::ItemCooldownReduction,
            &Target {
                record: &disabled,
                item: true,
                ultimate: false
            }
        ));
        assert!(eligible(
            AbilityStat::RangeBonus,
            &Target {
                record: &disabled,
                item: true,
                ultimate: false
            }
        ));
    }
    #[test]
    fn recorded_zero_overrides_a_matching_property_without_hiding_other_properties() {
        let mut catalog = bonus(EffectScope::Global, 50.0);
        let mut dynamic = bonus(EffectScope::Global, 0.0);
        assert!(replaced_by_runtime(&mut catalog, &[dynamic]));
        dynamic = bonus(EffectScope::Global, 0.0);
        dynamic.property_name = Some("OtherRange".into());
        assert!(!replaced_by_runtime(&mut catalog, &[dynamic]));
    }
    #[test]
    fn targeted_runtime_override_preserves_global_bonus_on_other_abilities() {
        let mut base = bonus(EffectScope::Global, 10.0);
        assert!(!replaced_by_runtime(
            &mut base,
            &[bonus(EffectScope::Imbued, 20.0)]
        ));
        assert_eq!(base.excluded_targets, BTreeSet::from([123]));
        assert_eq!(base.value, Some(10.0));
    }
    #[test]
    fn conditional_inference_requires_a_timed_modifier_and_clock() {
        let mut modifier = record(123, json!({}));
        modifier.modifier_id = Some(7);
        let mut entry = CModifierTableEntry {
            modifier_subclass: Some(7),
            ability_subclass: Some(123),
            last_applied_time: Some(10.0),
            ..Default::default()
        };
        assert_eq!(inferred_state(&[], &modifier, true), EffectState::Inactive);
        assert_eq!(
            inferred_state(&[&entry], &modifier, true),
            EffectState::Unresolved
        );
        entry.duration = Some(5.0);
        assert_eq!(
            inferred_state(&[&entry], &modifier, true),
            EffectState::Active
        );
        assert_eq!(
            inferred_state(&[&entry], &modifier, false),
            EffectState::Unresolved
        );
    }
    #[test]
    fn modes_keep_permanent_and_imbued_bonuses_but_separate_active_buffs() {
        let mut catalog = catalog();
        catalog
            .modifier_value_types
            .insert(900, AbilityStat::RangeBonus.symbol().into());
        let intrinsic = &mut catalog.modifiers[1];
        intrinsic.definition_path = "/test_gun/m_AutoIntrinsicModifiers/0".into();
        intrinsic.stat_changes = vec![
            json!({"stat":AbilityStat::RangeBonus.symbol(),"value":10,"property_name":"GlobalRange","definition_path":"/global"}),
            json!({"stat":AbilityStat::RangeBonus.symbol(),"value":20,"property_name":"ImbuedRange","definition_path":"/imbued","apply_filter":"EApplyFilter_OnlyIfImbued"}),
        ];
        let buff = &mut catalog.modifiers[2];
        buff.ability_id = Some(123);
        buff.definition_path = "/test_gun/m_BuffModifier".into();
        buff.stat_changes = vec![json!({
            "stat":AbilityStat::RangeBonus.symbol(),"value":25,"definition_path":"/active"
        })];
        let entries = [
            CModifierTableEntry {
                modifier_subclass: Some(11),
                serial_number: Some(1),
                ..Default::default()
            },
            CModifierTableEntry {
                modifier_subclass: Some(12),
                serial_number: Some(2),
                duration: Some(5.0),
                last_applied_time: Some(10.0),
                ..Default::default()
            },
        ];
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let imbues = BTreeSet::from([(123, 777)]);
        for mode in [StatMode::Baseline, StatMode::Current] {
            let mut inputs = PlayerInputs {
                hero: &catalog.heroes[&999],
                level: Some(0.0),
                weapon: &catalog.abilities[&123],
                inventory: vec![],
                owned: vec![&catalog.abilities[&123]],
                active: entries.iter().collect(),
                permanent: vec![RecordedStat {
                    source_id: 10,
                    value_type: Some(900),
                    value: 5.0,
                }],
            };
            assert!(inputs.select_mode(&catalog, mode).is_empty());
            let mut scratch = Vec::new();
            let mut resolver = resolver(&ctx, &catalog, &controller, 999, &mut scratch);
            resolver.mode = mode;
            resolver.game_time = Some(12.0);
            let effects = catalog_effects(&mut resolver, &inputs, &imbues, vec![]).unwrap();
            for (id, baseline, current) in [(777, 31.6, 48.7), (888, 14.5, 35.875)] {
                let ability = record(id, json!({}));
                let target = Target {
                    record: &ability,
                    ultimate: false,
                    item: false,
                };
                let values = effects
                    .iter()
                    .filter(|effect| applies(effect, &target, &resolver).unwrap())
                    .filter(|effect| effect.state == EffectState::Active)
                    .filter_map(|effect| effect.value);
                let value = crate::rulesets::range_bonus::calculate(values).unwrap();
                assert!(
                    (value
                        - if mode == StatMode::Baseline {
                            baseline
                        } else {
                            current
                        })
                    .abs()
                        < 1e-10
                );
            }
            assert_eq!(
                effects.iter().any(|effect| effect.serial == Some(2)),
                mode == StatMode::Current
            );
        }
    }

    #[test]
    fn dynamic_baseline_needs_passive_evidence_not_just_an_imbue_target() {
        let mut catalog = catalog();
        catalog.modifiers[1].definition_path = "/test_gun/m_AutoIntrinsicModifiers/0".into();
        catalog.modifiers[2].definition_path = "/test_gun/m_BuffModifier".into();
        let owner = &catalog.abilities[&123];
        let passive = json!({"modifier_keys":[catalog.modifiers[1].record_key]});
        let temporary = json!({"modifier_keys":[catalog.modifiers[2].record_key]});
        let mixed = json!({"modifier_keys":[catalog.modifiers[1].record_key,catalog.modifiers[2].record_key]});
        let conditional = json!({"usage_flags":"ConditionallyApplied"});
        let declared = json!({"usage_flags":"IntrinsicallyProvidedInAbility"});
        assert_eq!(
            dynamic_role(&catalog, owner, Some(&passive)),
            DynamicRole::Passive
        );
        assert_eq!(
            dynamic_role(&catalog, owner, Some(&declared)),
            DynamicRole::Passive
        );
        assert_eq!(
            dynamic_role(&catalog, owner, Some(&temporary)),
            DynamicRole::Temporary
        );
        assert_eq!(
            dynamic_role(&catalog, owner, Some(&conditional)),
            DynamicRole::Temporary
        );
        for property in [
            None,
            Some(&mixed),
            Some(&json!({"apply_filter":"EApplyFilter_OnlyIfImbued"})),
        ] {
            assert_eq!(
                dynamic_role(&catalog, owner, property),
                DynamicRole::Unknown
            );
        }
        // An ambiguous recorded value must suppress a catalog fallback on that
        // target, even though the recorded value cannot enter the subtotal.
        let mut recorded = bonus(EffectScope::Imbued, 35.0);
        recorded.conditional(EffectState::Unresolved, "unknown baseline role");
        let mut default = bonus(EffectScope::Global, 10.0);
        assert!(!replaced_by_runtime(&mut default, &[recorded]));
        assert_eq!(default.excluded_targets, BTreeSet::from([123]));
    }

    #[test]
    fn all_v1_rules_use_complement_products_and_reject_invalid_values() {
        for rule in [
            crate::rulesets::cooldown_reduction::calculate,
            crate::rulesets::item_cooldown_reduction::calculate,
            crate::rulesets::duration_bonus::calculate,
            crate::rulesets::range_bonus::calculate,
            crate::rulesets::radius_bonus::calculate,
        ] {
            assert_eq!(rule([10.0, 20.0]).unwrap(), 28.0);
            assert_eq!(rule([0.75, 0.0]).unwrap(), 0.75);
            assert!(rule([f64::NAN, 20.0]).is_err());
            assert!(rule([100.1, 0.0]).is_err());
        }
    }
}
