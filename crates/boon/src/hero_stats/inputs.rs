//! Resolve replay inputs without embedding hero IDs, item values or enum numbers.
pub mod abilities;
mod modes;

use super::catalog::{Record, number};
use super::{
    CalculationError, Contribution, HeroStat, HeroStatQuery, StatCatalog, StatMode, StatResult,
    StatRow,
};
use crate::{Context, EffectiveModifierState, Entity, FieldValue, ModifierClock, rulesets};
use boon_proto::proto::CModifierTableEntry;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};

type Result<T> = std::result::Result<T, CalculationError>;
const FLAT: &str = "MODIFIER_VALUE_AMMO_CLIP_SIZE";
const PERCENT: &str = "MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT";
const WEAPON_DAMAGE: &str = "MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE";
const SPIRIT: &str = "MODIFIER_VALUE_TECH_POWER";
const BULLET_LIFESTEAL: &str = "MODIFIER_VALUE_BULLET_LIFESTEAL";
const SPIRIT_LIFESTEAL: &str = "MODIFIER_VALUE_TECH_LIFESTEAL";
const MOVE_SPEED: &str = "MODIFIER_VALUE_MOVEMENT_SPEED_MAX";
const SPRINT_SPEED: &str = "MODIFIER_VALUE_SPRINT_SPEED_BONUS";

// Narrow engine exception: these ping markers have no stat effects in VData.
// IDs are MurmurHash2(name, 0x31415926), confirmed against client/server strings
// at GameTracking-Deadlock revision 19022f397ce9ba65856752d0cbaa82e80a2da73f.
// Keep this exclusion in stat resolution; raw modifier data must retain them.
const PLAYER_PINGED: u32 = 1_215_616_703; // modifier_player_pinged
const ENTITY_PINGED: u32 = 2_693_099_904; // modifier_entity_pinged

fn speed_property_number(value: &Value, stat: &str) -> Option<f64> {
    number(value).or_else(|| {
        if !matches!(stat, MOVE_SPEED | SPRINT_SPEED) {
            return None;
        }
        // VData's `m` suffix encodes metres. Bare numbers for these modifier
        // symbols use Source units. Resolve upgrades in the same units as base.
        value
            .as_str()?
            .trim()
            .strip_suffix('m')?
            .trim()
            .parse::<f64>()
            .ok()
            .map(|metres| metres / rulesets::METERS_PER_SOURCE_UNIT)
    })
}

fn empty_declaration(owner: &Record, effect: &Value) -> bool {
    // Inherited type/display declarations are not bonuses. Only skip unbound,
    // empty declarations without activation flags or upgrades; actual malformed
    // bound values must still fail validation.
    effect["kind"] == "ability_property"
        && effect["modifier_keys"]
            .as_array()
            .is_none_or(|keys| keys.is_empty())
        && effect["usage_flags"].as_str().is_none_or(str::is_empty)
        && ["value", "raw_value", "value_min", "value_max", "scaling"]
            .iter()
            .all(|key| {
                effect[*key].is_null()
                    || effect[*key]
                        .as_str()
                        .is_some_and(|value| value.trim().is_empty())
            })
        && effect["property_name"]
            .as_str()
            .is_some_and(|name| !owner.upgrades_property(name))
}

fn different_effect_owner(effect: &Value, entry: &CModifierTableEntry) -> bool {
    effect["source_ability_id"].as_u64().is_some_and(|owner| {
        entry
            .ability_subclass
            .is_some_and(|id| u64::from(id) != owner)
    })
}

fn modifier_units(stat: &str, value: f64) -> f64 {
    if matches!(stat, MOVE_SPEED | SPRINT_SPEED) {
        value * rulesets::METERS_PER_SOURCE_UNIT
    } else {
        value
    }
}

fn contribution_kind(kind: &'static str, effect: &Value) -> &'static str {
    if effect["stat"] == SPIRIT && effect["calculation_stage"] == "post_multiplier" {
        "post_multiplier_flat"
    } else {
        kind
    }
}

fn invalid(message: impl Into<String>) -> CalculationError {
    CalculationError::Invalid(message.into())
}
fn field(ctx: &Context, entity: &Entity, path: &str) -> Option<f64> {
    let serializer = ctx.serializers().get(&entity.class_name)?;
    match entity.get_by_name(path, serializer)? {
        FieldValue::F32(v) => Some(f64::from(*v)),
        FieldValue::I32(v) => Some(f64::from(*v)),
        FieldValue::U32(v) => Some(f64::from(*v)),
        FieldValue::I64(v) => Some(*v as f64),
        FieldValue::U64(v) => Some(*v as f64),
        _ => None,
    }
    .filter(|v| v.is_finite())
}
fn required(ctx: &Context, entity: &Entity, path: &str) -> Result<f64> {
    field(ctx, entity, path).ok_or_else(|| invalid(format!("missing replay field {path}")))
}
fn count(ctx: &Context, entity: &Entity, path: &str) -> Result<usize> {
    let n = required(ctx, entity, path)?;
    if !(0.0..=65536.0).contains(&n) || n.fract() != 0.0 {
        return Err(invalid(format!("invalid vector length for {path}")));
    }
    Ok(n as usize)
}
fn id(ctx: &Context, entity: &Entity, path: &str) -> Result<u32> {
    let value = required(ctx, entity, path)?;
    if !(0.0..=f64::from(u32::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(format!("invalid ID in {path}")));
    }
    Ok(value as u32)
}
fn checked_runtime_count(value: f64, path: &str) -> Result<f64> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(invalid(format!("invalid runtime count {path}")));
    }
    Ok(value)
}

fn checked_counter_contribution(value: f64, count: f64) -> Result<f64> {
    let contribution = value * count;
    if !contribution.is_finite() {
        return Err(invalid("nonfinite counter contribution"));
    }
    Ok(contribution)
}

fn runtime_count_field(counter: &Value) -> Result<&str> {
    counter
        .as_str()
        .or_else(|| counter["field"].as_str())
        .filter(|field| !field.is_empty())
        .ok_or_else(|| invalid("invalid catalog runtime count field"))
}

// Steam IDs exceed f64's exact integer range. Read the wire integer directly.
fn steam_id(ctx: &Context, controller: &Entity) -> Option<u64> {
    let serializer = ctx.serializers().get(&controller.class_name)?;
    let key = serializer.resolve_field_key("m_steamID")?;
    match controller.fields.get(&key)? {
        FieldValue::U64(id) if *id != 0 => Some(*id),
        _ => None,
    }
}

fn handle<'a>(ctx: &'a Context, entity: &Entity, path: &str) -> Option<&'a Entity> {
    let s = ctx.serializers().get(&entity.class_name)?;
    ctx.entities()
        .get_by_handle(entity.get_handle(s.resolve_field_key(path))?)
}

fn hero_pawn<'a>(ctx: &'a Context, controller: &Entity) -> Option<&'a Entity> {
    // m_hPawn can refer to the spectator pawn while the hero is dead.
    handle(ctx, controller, "m_hHeroPawn").or_else(|| handle(ctx, controller, "m_hPawn"))
}

pub(super) fn collect(
    ctx: &Context,
    query: &HeroStatQuery,
    catalog: &StatCatalog,
    modifiers: &EffectiveModifierState,
    result: &mut StatResult,
) -> Result<()> {
    let game_time = ModifierClock::resolve(ctx).game_time(ctx).map(f64::from);
    let game_start = ctx
        .entities()
        .iter()
        .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
        .and_then(|(_, e)| field(ctx, e, "m_pGameRules.m_flGameStartTime"));
    let mut selected = HashSet::new();
    for (_, controller) in ctx
        .entities()
        .iter()
        .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
    {
        let steam_id = steam_id(ctx, controller);
        if query
            .steam_ids
            .as_ref()
            .is_some_and(|ids| steam_id.is_none_or(|id| !ids.contains(&id)))
        {
            continue;
        }
        let hero_id = required(ctx, controller, "m_PlayerDataGlobal.m_nHeroID")? as i64;
        if hero_id == 0
            || query
                .heroes
                .as_ref()
                .is_some_and(|heroes| !heroes.contains(&hero_id))
        {
            continue;
        }
        selected.extend(steam_id);
        let mut resolver = Resolver {
            mode: query.mode,
            ctx,
            catalog,
            controller,
            hero_id,
            steam_id,
            game_time,
            game_start,
            explain: query.explain,
            contributions: &mut result.contributions,
            ignored_modifiers: BTreeMap::new(),
            inferred_bindings: BTreeSet::new(),
            unmapped_inputs: BTreeSet::new(),
            stat: query.stats[0],
        };
        let mut inputs = PlayerInputs::collect(ctx, catalog, controller, hero_id, modifiers);
        let mode_diagnostics = inputs.as_mut().map_or_else(
            |_| BTreeSet::new(),
            |inputs| inputs.select_mode(catalog, query.mode),
        );
        for &stat in &query.stats {
            resolver.stat = stat;
            resolver.ignored_modifiers.clear();
            resolver.inferred_bindings.clear();
            resolver.unmapped_inputs.clone_from(&mode_diagnostics);
            let value = if stat == HeroStat::GravityScale {
                resolver.gravity_scale().map_err(|error| error.to_string())
            } else {
                inputs
                    .as_ref()
                    .map_err(ToString::to_string)
                    .and_then(|inputs| {
                        match stat {
                            HeroStat::ClipSize => resolver.ammo(inputs).map(f64::from),
                            HeroStat::BulletVelocity => resolver.bullet_velocity(inputs),
                            HeroStat::WeaponDamage => resolver.weapon_damage(inputs),
                            HeroStat::MeleeDistance => resolver.melee_distance(inputs),
                            HeroStat::LightMeleeDamage | HeroStat::HeavyMeleeDamage => {
                                resolver.melee_damage(inputs, stat)
                            }
                            HeroStat::ReloadTime => resolver.reload_time(inputs),
                            HeroStat::FireRate => resolver.fire_rate(inputs),
                            HeroStat::SlideDistance | HeroStat::BulletEvasion => {
                                resolver.movement_percent(inputs, stat)
                            }
                            HeroStat::GravityScale => resolver.gravity_scale(),
                            HeroStat::Stamina => resolver.stamina(inputs),
                            HeroStat::DebuffResist => resolver.debuff_resist(inputs),
                            HeroStat::BulletResist
                            | HeroStat::SpiritResist
                            | HeroStat::MeleeResist => resolver.resistance(inputs, stat),
                            HeroStat::BulletLifesteal | HeroStat::SpiritLifesteal => {
                                resolver.lifesteal(inputs, stat)
                            }
                            HeroStat::MeleeLifesteal => resolver.melee_lifesteal(inputs),
                            HeroStat::MoveSpeed | HeroStat::SprintSpeed => {
                                resolver.speed(inputs, stat)
                            }
                            HeroStat::StaminaCooldown => resolver.stamina_cooldown(inputs),
                            HeroStat::DashSpeed
                            | HeroStat::DashDuration
                            | HeroStat::AirDashSpeed
                            | HeroStat::AirDashDuration => resolver.dash(inputs, stat),
                            HeroStat::FalloffStart | HeroStat::FalloffEnd => {
                                resolver.falloff_range(inputs, stat)
                            }
                        }
                        .map_err(|error| error.to_string())
                    })
            };
            let (value, mut diagnostic) = match value {
                Ok(value) => (Some(value), None),
                Err(error) => {
                    let message = format!(
                        "tick {} player {} hero {hero_id} {}: {error}",
                        ctx.tick(),
                        steam_id.map_or_else(|| "without a Steam ID".into(), |id| id.to_string()),
                        stat.as_str()
                    );
                    if query.strict {
                        return Err(invalid(message));
                    }
                    (None, Some(message))
                }
            };
            if !resolver.ignored_modifiers.is_empty() {
                let ignored = format!(
                    "ignored modifiers: {}",
                    resolver
                        .ignored_modifiers
                        .values()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join("; ")
                );
                diagnostic = Some(match diagnostic {
                    Some(error) => format!("{error}; {ignored}"),
                    None => ignored,
                });
            }
            if !resolver.inferred_bindings.is_empty() {
                let inferred = format!(
                    "inferred activation: {}",
                    resolver
                        .inferred_bindings
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join("; ")
                );
                diagnostic = Some(match diagnostic {
                    Some(error) => format!("{error}; {inferred}"),
                    None => inferred,
                });
            }
            if !resolver.unmapped_inputs.is_empty() {
                let missing = format!(
                    "unmapped inputs: {}",
                    resolver
                        .unmapped_inputs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join("; ")
                );
                diagnostic = Some(match diagnostic {
                    Some(error) => format!("{error}; {missing}"),
                    None => missing,
                });
            }
            result.values.push(StatRow {
                mode: query.mode,
                tick: ctx.tick(),
                steam_id,
                hero_id,
                stat,
                value,
                unit: stat.unit(),
                ruleset: stat.rule().id,
                status: if value.is_none() {
                    "unresolved"
                } else if diagnostic.is_some() {
                    "partial"
                } else {
                    "calculated"
                },
                diagnostic,
            });
        }
    }
    if let Some(ids) = &query.steam_ids {
        for id in ids {
            if !selected.contains(id) {
                return Err(invalid(format!(
                    "Steam ID {id} has no selected hero at tick {}",
                    ctx.tick()
                )));
            }
        }
    }
    Ok(())
}

fn purchase_threshold_bonus(table: &Value, invested: f64) -> Result<f64> {
    let rows = table
        .as_array()
        .ok_or_else(|| invalid("invalid purchase-bonus table"))?;
    let mut selected = None;
    for row in rows {
        let threshold = number(&row["nGoldThreshold"])
            .filter(|v| *v >= 0.0)
            .ok_or_else(|| invalid("invalid purchase-bonus threshold"))?;
        let bonus = number(&row["flBonus"]).ok_or_else(|| invalid("invalid purchase bonus"))?;
        if threshold <= invested && selected.is_none_or(|(previous, _)| threshold > previous) {
            selected = Some((threshold, bonus));
        }
    }
    Ok(selected.map_or(0.0, |(_, bonus)| bonus))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValuePolicy {
    Declared,
    Bound,
    // A declaration can describe a per-kill reward, not a bonus from ownership.
    Registered,
    // Debuffs affect the recipient of a live modifier, not the ability owner.
    Recipient,
}

// Item removal can destroy its ability without a modifier-table removal row.
// Only intrinsic modifiers depend on that entity's lifetime; cast effects can
// outlive their source ability. Missing handles do not prove removal.
fn intrinsic_ability_present(
    ctx: &Context,
    catalog: &StatCatalog,
    entry: &CModifierTableEntry,
) -> bool {
    let Some(handle) = entry.ability.filter(|h| *h != crate::INVALID_ENTITY_HANDLE) else {
        return true;
    };
    if ctx.entities().get_by_handle(handle).is_some() {
        return true;
    }
    let Some(owner) = entry
        .ability_subclass
        .and_then(|id| catalog.abilities.get(&id))
    else {
        return true;
    };
    entry
        .modifier_subclass
        .and_then(|id| catalog.modifier(id, entry.ability_subclass).ok())
        .is_none_or(|modifier| !modifier.is_intrinsic_modifier_of(owner))
}

struct RecordedStat {
    source_id: u32,
    value_type: Option<u32>,
    value: f64,
}

struct PlayerInputs<'a> {
    hero: &'a Record,
    level: Option<f64>,
    weapon: &'a Record,
    inventory: Vec<&'a Record>,
    owned: Vec<&'a Record>,
    active: Vec<&'a CModifierTableEntry>,
    permanent: Vec<RecordedStat>,
}

fn modifier_states_absent(source: &Record, catalog: &StatCatalog, evidence: &[u32]) -> bool {
    let Some(mask) = source.definition["m_nEnabledStateMask"].as_str() else {
        return false;
    };
    let mut declared = false;
    for name in mask
        .split('|')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let Some(&index) = catalog.modifier_states.get(name) else {
            return false;
        };
        let Some(word) = evidence.get(index as usize / u32::BITS as usize) else {
            return false;
        };
        if word & (1 << (index % u32::BITS)) != 0 {
            return false;
        }
        declared = true;
    }
    declared
}

impl<'a> PlayerInputs<'a> {
    fn collect(
        ctx: &Context,
        catalog: &'a StatCatalog,
        controller: &Entity,
        hero_id: i64,
        state: &'a EffectiveModifierState,
    ) -> Result<Self> {
        let hero = catalog
            .heroes
            .get(&hero_id)
            .ok_or_else(|| invalid(format!("missing hero {hero_id}")))?;
        let weapon = catalog.weapon(hero)?;
        let pawn =
            hero_pawn(ctx, controller).ok_or_else(|| invalid("player has no current pawn"))?;
        let states = crate::player_states::modifier_state_evidence(ctx, pawn);
        let mut active: Vec<_> = state
            .entries()
            .values()
            .filter(|m| intrinsic_ability_present(ctx, catalog, m))
            .filter(|m| {
                m.parent
                    .and_then(|h| ctx.entities().get_by_handle(h))
                    .is_some_and(|e| std::ptr::eq(e, pawn))
            })
            .filter(|m| {
                // Untimed rows can remain after their effects end. A catalog
                // modifier that enables states cannot still apply when all of
                // those states are absent. Missing masks or names prove nothing;
                // a present bit alone does not prove that an old row is active.
                !states.as_ref().is_some_and(|evidence| {
                    m.modifier_subclass
                        .and_then(|id| catalog.modifier(id, m.ability_subclass).ok())
                        .is_some_and(|source| modifier_states_absent(source, catalog, evidence))
                })
            })
            .collect();
        active.sort_by_key(|m| m.serial_number);
        let mut inventory = Vec::new();
        let inventory_path = "m_PlayerDataGlobal.m_vecUpgrades";
        for i in 0..count(ctx, controller, inventory_path)? {
            let ability = id(ctx, controller, &format!("{inventory_path}.{i}"))?;
            if ability == 0 {
                continue;
            }
            let record = catalog
                .abilities
                .get(&ability)
                .ok_or_else(|| invalid(format!("missing item {ability}")))?;
            inventory.push(record);
        }
        let mut owned = inventory.clone();
        let mut owned_ids: HashSet<_> = owned.iter().filter_map(|r| r.ability_id).collect();
        let vector = "m_CCitadelAbilityComponent.m_vecAbilities";
        for i in 0..count(ctx, pawn, vector)? {
            let Some(ability) = handle(ctx, pawn, &format!("{vector}.{i}")) else {
                continue;
            };
            let ability_id = id(ctx, ability, "m_nSubclassID")?;
            if ability_id == 0 || !owned_ids.insert(ability_id) {
                continue;
            }
            let record = catalog
                .abilities
                .get(&ability_id)
                .ok_or_else(|| invalid(format!("missing owned ability {ability_id}")))?;
            owned.push(record);
        }
        let vector = "m_PlayerDataGlobal.m_vecStatViewerModifierValues";
        let mut permanent = Vec::new();
        for i in 0..count(ctx, controller, vector)? {
            let prefix = format!("{vector}.{i}");
            permanent.push(RecordedStat {
                source_id: id(ctx, controller, &format!("{prefix}.m_SourceModifierID"))?,
                value_type: field(ctx, controller, &format!("{prefix}.m_eValType"))
                    .map(|_| id(ctx, controller, &format!("{prefix}.m_eValType")))
                    .transpose()?,
                value: required(ctx, controller, &format!("{prefix}.m_flValue"))?,
            });
        }
        Ok(Self {
            hero,
            level: field(ctx, controller, "m_PlayerDataGlobal.m_iLevel"),
            weapon,
            inventory,
            owned,
            active,
            permanent,
        })
    }
}

struct Resolver<'a, 'b> {
    mode: StatMode,
    ctx: &'a Context,
    catalog: &'a StatCatalog,
    controller: &'a Entity,
    hero_id: i64,
    steam_id: Option<u64>,
    stat: HeroStat,
    game_time: Option<f64>,
    game_start: Option<f64>,
    explain: bool,
    contributions: &'b mut Vec<Contribution>,
    ignored_modifiers: BTreeMap<u32, String>,
    inferred_bindings: BTreeSet<String>,
    unmapped_inputs: BTreeSet<String>,
}
impl<'a> Resolver<'a, '_> {
    fn modifier(&mut self, id: u32, ability: Option<u32>) -> Option<&'a Record> {
        if matches!(id, PLAYER_PINGED | ENTITY_PINGED) {
            return None;
        }
        match self.catalog.modifier(id, ability) {
            Ok(record) => Some(record),
            Err(error) => {
                self.ignored_modifiers
                    .entry(id)
                    .or_insert_with(|| error.to_string());
                None
            }
        }
    }

    fn recorded_stat(&mut self, recorded: &RecordedStat) -> Result<Option<&'a str>> {
        // One source can supply several stats (range/radius pickups or corruption).
        // The replay records the type; enum numbers come from the selected catalog.
        if let Some(value_type) = recorded.value_type
            && !self.catalog.modifier_value_types.is_empty()
        {
            return self.catalog.modifier_value_types.get(&value_type)
                .map(|stat| Some(stat.as_str()))
                .ok_or_else(|| invalid(format!(
                    "catalog has no modifier value type {value_type}; refresh the matching boon-data version"
                )));
        }
        // Older recordings or catalogs can omit enum data. Require a unique binding.
        let Some(source) = self.modifier(recorded.source_id, None) else {
            return Ok(None);
        };
        let mut stats = source
            .stat_changes
            .iter()
            .filter_map(|effect| effect["stat"].as_str());
        let stat = stats.next();
        if stat.is_none() || stats.any(|other| Some(other) != stat) {
            return Err(invalid(format!(
                "ambiguous permanent stat source {}",
                recorded.source_id
            )));
        }
        Ok(stat)
    }

    fn recorded_source(&self, source_id: u32) -> String {
        // A missing label does not invalidate a recorded type and value.
        self.catalog.modifier(source_id, None).map_or_else(
            |_| format!("modifier:{source_id}"),
            |source| source.record_key.clone(),
        )
    }

    fn infer_binding(&mut self, effect: &Value, ability: &Record, modifier: &Record) {
        self.inferred_bindings.insert(format!(
            "{} in {} -> {}",
            effect["property_name"]
                .as_str()
                .unwrap_or("unknown property"),
            ability.record_key,
            modifier.record_key,
        ));
    }

    fn record(
        &mut self,
        input: &str,
        kind: &'static str,
        value: f64,
        source: &Record,
        path: &str,
        serial: Option<u32>,
    ) {
        if self.explain {
            self.contributions.push(Contribution {
                mode: self.mode,
                tick: self.ctx.tick(),
                steam_id: self.steam_id,
                hero_id: self.hero_id,
                stat: self.stat,
                input: input.into(),
                kind,
                value,
                source: source.record_key.clone(),
                definition_path: path.into(),
                modifier_serial: serial,
            });
        }
    }
    fn ammo(&mut self, inputs: &PlayerInputs<'_>) -> Result<u32> {
        let weapon = inputs.weapon;
        let (info, path) = weapon.weapon_info();
        let base =
            number(&info["m_iClipSize"]).ok_or_else(|| invalid("weapon has no base clip size"))?;
        self.record(
            "clip_size",
            "base",
            base,
            weapon,
            &format!("{}/{path}/m_iClipSize", weapon.definition_path),
            None,
        );
        let spirit = self.spirit_scaled_bonus(inputs, "EClipSize", "clip_size", "flat")?;
        let flat = spirit + self.total(FLAT, "clip_size", "flat", inputs)?;
        let percent = self.total(PERCENT, "clip_size", "percent", inputs)?;
        rulesets::clip_size::calculate(base, flat, percent)
    }

    fn bullet_velocity(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let weapon = inputs.weapon;
        let (info, path) = weapon.weapon_info();
        let base = number(&info["m_flBulletSpeed"])
            .ok_or_else(|| invalid("weapon has no base bullet speed"))?;
        self.record(
            "bullet_velocity",
            "base",
            base * rulesets::bullet_velocity::METERS_PER_SOURCE_UNIT,
            weapon,
            &format!("{}/{path}/m_flBulletSpeed", weapon.definition_path),
            None,
        );
        if self.total(
            "MODIFIER_VALUE_BASE_BULLET_SPEED_OVERRIDE",
            "bullet_velocity",
            "override",
            inputs,
        )? != 0.0
        {
            return Err(invalid(
                "base bullet-speed overrides are not supported by bullet_velocity.v1",
            ));
        }
        let percent = self.total(
            "MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT",
            "bullet_velocity",
            "percent",
            inputs,
        )?;
        rulesets::bullet_velocity::calculate(base, percent)
    }

    fn falloff_range(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let weapon = inputs.weapon;
        let (info, path) = weapon.weapon_info();
        let start = number(&info["m_flDamageFalloffStartRange"])
            .ok_or_else(|| invalid("weapon has no base falloff start"))?;
        let end = number(&info["m_flDamageFalloffEndRange"])
            .ok_or_else(|| invalid("weapon has no base falloff end"))?;
        let (base, field) = match stat {
            HeroStat::FalloffStart => (start, "m_flDamageFalloffStartRange"),
            HeroStat::FalloffEnd => (end, "m_flDamageFalloffEndRange"),
            _ => return Err(invalid("expected a falloff endpoint")),
        };
        self.record(
            stat.as_str(),
            "base",
            base * rulesets::METERS_PER_SOURCE_UNIT,
            weapon,
            &format!("{}/{path}/{field}", weapon.definition_path),
            None,
        );
        let (percent, count) = self.total_and_count(
            "MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT",
            stat.as_str(),
            "percent",
            inputs,
        )?;
        if count > 1 {
            return Err(invalid(
                "combining falloff-range bonuses is not supported by v1",
            ));
        }
        let (start, end) = rulesets::falloff_range::calculate(start, end, percent)?;
        Ok(if stat == HeroStat::FalloffStart {
            start
        } else {
            end
        })
    }

    fn movement_percent(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let modifier = match stat {
            HeroStat::SlideDistance => "MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE",
            HeroStat::BulletEvasion => "MODIFIER_VALUE_BULLET_EVASION",
            _ => return Err(invalid("expected slide distance or bullet evasion")),
        };
        let mut slide = rulesets::slide_distance::Modifiers::default();
        let mut total = 0.0;
        let mut count = 0;
        self.visit_values(
            modifier,
            stat.as_str(),
            "percent",
            inputs,
            ValuePolicy::Bound,
            |value| {
                if stat == HeroStat::SlideDistance {
                    slide.add(value)?;
                } else {
                    rulesets::bullet_evasion::calculate(value)?;
                    total += value;
                    count += usize::from(value != 0.0);
                }
                Ok(())
            },
        )?;
        if stat == HeroStat::BulletEvasion && count > 1 {
            return Err(invalid(
                "combining bullet-evasion chances is not supported by bullet_evasion.v1",
            ));
        }
        if stat == HeroStat::SlideDistance {
            slide.calculate()
        } else {
            rulesets::bullet_evasion::calculate(total)
        }
    }

    fn hero_base(
        &mut self,
        inputs: &PlayerInputs<'_>,
        key: &str,
        input: &str,
        kind: &'static str,
    ) -> Result<f64> {
        let hero = inputs.hero;
        let base = number(&hero.definition["m_mapStartingStats"][key])
            .ok_or_else(|| invalid(format!("hero has no valid base {key}")))?;
        self.record(
            input,
            kind,
            base,
            hero,
            &format!("{}/m_mapStartingStats/{key}", hero.definition_path),
            None,
        );
        Ok(base + self.spirit_scaled_bonus(inputs, key, input, "spirit_flat")?)
    }

    /// Only intrinsic properties and explicit modifier bindings qualify. In
    /// particular, an attack's own dash range is not an ordinary movement stat.
    fn bound_total(
        &mut self,
        inputs: &PlayerInputs<'_>,
        symbol: &str,
        input: &str,
        kind: &'static str,
    ) -> Result<(f64, usize)> {
        let mut total = 0.0;
        let mut count = 0;
        self.bound_values(inputs, symbol, input, kind, |value| {
            total += value;
            count += usize::from(value != 0.0);
            Ok(())
        })?;
        Ok((total, count))
    }

    fn bound_values(
        &mut self,
        inputs: &PlayerInputs<'_>,
        symbol: &str,
        input: &str,
        kind: &'static str,
        add: impl FnMut(f64) -> Result<()>,
    ) -> Result<()> {
        self.visit_values(symbol, input, kind, inputs, ValuePolicy::Bound, add)?;
        self.missing_intrinsics(inputs, symbol)
    }

    fn missing_intrinsics(&mut self, inputs: &PlayerInputs<'_>, symbol: &str) -> Result<()> {
        let present: HashSet<_> = inputs
            .active
            .iter()
            .filter_map(|entry| {
                self.modifier(entry.modifier_subclass?, entry.ability_subclass)
                    .map(|record| record.record_key.as_str())
            })
            .collect();
        for owner in &inputs.owned {
            for effect in &owner.stat_changes {
                if effect["stat"] != symbol || !self.mode.includes_effect(effect) {
                    continue;
                }
                let Some(keys) = effect["modifier_keys"].as_array() else {
                    continue;
                };
                let intrinsic = keys.iter().filter_map(Value::as_str).filter(|key| {
                    key.strip_prefix(&owner.record_key)
                        .and_then(|path| path.strip_prefix("/m_AutoIntrinsicModifiers/"))
                        // Only the direct intrinsic modifier is expected merely
                        // from ownership. Its nested proc buffs are conditional.
                        .is_some_and(|index| index.parse::<usize>().is_ok())
                });
                if intrinsic.clone().next().is_some()
                    && !intrinsic.clone().any(|key| present.contains(key))
                    && self.effect(effect, owner, None)? != 0.0
                {
                    // Some replay intervals omit an owned item's intrinsic
                    // modifier. Do not infer its effect from ownership alone or
                    // present a base-only value as a complete calculation.
                    self.unmapped_inputs.insert(format!(
                        "{} in {} has no effective intrinsic modifier ({})",
                        effect["property_name"].as_str().unwrap_or(symbol),
                        owner.record_key,
                        intrinsic.collect::<Vec<_>>().join(", "),
                    ));
                }
            }
        }
        Ok(())
    }

    fn speed(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let (key, symbol) = if stat == HeroStat::MoveSpeed {
            ("EMaxMoveSpeed", MOVE_SPEED)
        } else {
            ("ESprintSpeed", SPRINT_SPEED)
        };
        let input = stat.as_str();
        let base = self.hero_base(inputs, key, input, "base")?;
        for owner in &inputs.owned {
            let Some(properties) = owner.definition["m_mapAbilityProperties"].as_object() else {
                continue;
            };
            for (name, property) in properties {
                let Some(target) = property["m_strLocTokenOverride"].as_str() else {
                    continue;
                };
                if properties
                    .get(target)
                    .is_some_and(|target| target["m_eProvidedPropertyType"] == symbol)
                    && !owner
                        .stat_changes
                        .iter()
                        .any(|effect| effect["property_name"] == *name)
                    && speed_property_number(&property["m_strValue"], symbol) != Some(0.0)
                {
                    // A shared display token suggests a related input, but does
                    // not establish a stat binding, activation or per-stack rule.
                    self.unmapped_inputs.insert(format!(
                        "{name} in {} uses the {target} display token but has no stat binding",
                        owner.record_key,
                    ));
                }
            }
        }
        if stat == HeroStat::SprintSpeed {
            let (bonus, _) = self.bound_total(inputs, symbol, input, "flat")?;
            return rulesets::sprint_speed::calculate(base, bonus);
        }
        let mut bonuses = rulesets::move_speed::Modifiers::default();
        self.bound_values(inputs, symbol, input, "flat", |bonus| bonuses.add(bonus))?;
        let (percent, count) = self.bound_total(
            inputs,
            "MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT",
            input,
            "percent",
        )?;
        if count > 1 {
            return Err(invalid(
                "combining movement-speed percentages is not supported by move_speed.v1",
            ));
        }
        // Nominal stat values exclude slows, speed limits, firing/crouch state
        // and sprint ramp-up. The extra sprint component is calculated separately.
        bonuses.calculate(base, percent)
    }

    fn debuff_resist(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let input = "debuff_resist";
        let key = "EDebuffResist";
        let stats = inputs.hero.definition["m_mapStartingStats"]
            .as_object()
            .ok_or_else(|| invalid("hero has no starting stats"))?;
        let mut modifiers = rulesets::debuff_resist::Modifiers::default();
        // VData omits zero innate resistance on most heroes. An explicit value
        // must still be valid; do not turn malformed catalog data into zero.
        if let Some(value) = stats.get(key) {
            let base = number(value).ok_or_else(|| invalid("invalid base EDebuffResist"))?;
            modifiers.add(base)?;
            self.record(
                input,
                "base",
                base,
                inputs.hero,
                &format!("{}/m_mapStartingStats/{key}", inputs.hero.definition_path),
                None,
            );
        }
        self.bound_values(
            inputs,
            "MODIFIER_VALUE_STATUS_RESISTANCE",
            input,
            "percent",
            |value| modifiers.add(value),
        )?;
        Ok(modifiers.calculate())
    }

    fn resistance(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let (key, symbol, reduction) = match stat {
            HeroStat::BulletResist => (
                "EBulletArmorDamageReduction",
                "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST",
                "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION",
            ),
            HeroStat::SpiritResist => (
                "ETechArmorDamageReduction",
                "MODIFIER_VALUE_TECH_RESIST",
                "MODIFIER_VALUE_TECH_RESIST_REDUCTION",
            ),
            HeroStat::MeleeResist => (
                "EMeleeResist",
                "MODIFIER_VALUE_MELEE_RESIST",
                "MODIFIER_VALUE_MELEE_RESIST_REDUCTION",
            ),
            _ => return Err(invalid("expected a resistance stat")),
        };
        let input = stat.as_str();
        let stats = inputs.hero.definition["m_mapStartingStats"]
            .as_object()
            .ok_or_else(|| invalid("hero has no starting stats"))?;
        let mut innate = 0.0;
        if let Some(value) = stats.get(key) {
            innate = number(value).ok_or_else(|| invalid(format!("invalid base {key}")))?;
            self.record(
                input,
                "base",
                innate,
                inputs.hero,
                &format!("{}/m_mapStartingStats/{key}", inputs.hero.definition_path),
                None,
            );
        }
        // The innate component is additive, even when a zero base is omitted.
        // In particular, resolve catalog spirit scaling without checking hero IDs.
        innate += self.spirit_scaled_bonus(inputs, key, input, "spirit_flat")?;
        if inputs.hero.definition["m_mapStandardLevelUpUpgrades"]
            .get(symbol)
            .is_some()
        {
            innate += self.level_bonus(inputs, symbol, input, "boon_flat")?;
        }
        let mut modifiers = rulesets::resistance::Modifiers::default();
        modifiers.add_resistance(innate)?;
        self.bound_values(inputs, symbol, input, "percent", |value| {
            modifiers.add_resistance(value)
        })?;
        for owner in &inputs.owned {
            self.unbound_property(owner, symbol, None)?;
        }
        self.visit_values(
            reduction,
            input,
            "reduction",
            inputs,
            ValuePolicy::Recipient,
            |value| {
                // VData encodes these hero debuffs as negative percentages. Positive
                // values have a different/unknown meaning; do not take their abs().
                if value > 0.0 {
                    return Err(invalid(
                        "positive resistance-reduction values are not supported",
                    ));
                }
                modifiers.add_reduction(-value)
            },
        )?;
        for entry in &inputs.active {
            let Some(source) = entry
                .modifier_subclass
                .and_then(|id| self.modifier(id, entry.ability_subclass))
            else {
                continue;
            };
            if let Some(owner) = source
                .ability_id
                .and_then(|id| self.catalog.abilities.get(&id))
            {
                self.unbound_property(owner, symbol, Some(entry))?;
                if !source.is_intrinsic_modifier_of(owner) {
                    self.unbound_property(owner, reduction, Some(entry))?;
                }
            }
        }
        modifiers.calculate()
    }

    fn lifesteal(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let (key, symbol) = if stat == HeroStat::BulletLifesteal {
            ("EBulletLifesteal", BULLET_LIFESTEAL)
        } else {
            ("ETechLifesteal", SPIRIT_LIFESTEAL)
        };
        let input = stat.as_str();
        let stats = inputs.hero.definition["m_mapStartingStats"]
            .as_object()
            .ok_or_else(|| invalid("hero has no starting stats"))?;
        let mut modifiers = rulesets::lifesteal::Modifiers::default();
        // Missing innate lifesteal is zero; malformed explicit data is an error.
        if stats.contains_key(key) {
            modifiers.add(self.hero_base(inputs, key, input, "base")?)?;
        }
        self.bound_values(inputs, symbol, input, "percent", |value| {
            modifiers.add(value)
        })?;
        for owner in &inputs.owned {
            self.unbound_property(owner, symbol, None)?;
        }
        for entry in &inputs.active {
            let Some(source) = entry
                .modifier_subclass
                .and_then(|id| self.modifier(id, entry.ability_subclass))
            else {
                continue;
            };
            if let Some(owner) = source
                .ability_id
                .and_then(|id| self.catalog.abilities.get(&id))
            {
                self.unbound_property(owner, symbol, Some(entry))?;
            }
        }
        Ok(modifiers.calculate())
    }

    fn unbound_property(
        &mut self,
        owner: &Record,
        symbol: &str,
        entry: Option<&CModifierTableEntry>,
    ) -> Result<()> {
        for effect in &owner.stat_changes {
            if empty_declaration(owner, effect) || !self.mode.includes_effect(effect) {
                continue;
            }
            if effect["stat"] == symbol
                && effect["modifier_keys"]
                    .as_array()
                    .is_none_or(|keys| keys.is_empty())
                && !effect["usage_flags"]
                    .as_str()
                    .is_some_and(|flags| flags.contains("IntrinsicallyProvidedInAbility"))
                && self.effect(effect, owner, entry)? != 0.0
            {
                // A provided stat type alone does not prove activation. This also
                // covers declarations with missing/empty usage flags; do not
                // silently apply active or ability-specific bonuses globally.
                self.unmapped_inputs.insert(format!(
                    "{} in {} has no modifier binding",
                    effect["property_name"].as_str().unwrap_or(symbol),
                    owner.record_key,
                ));
            }
        }
        Ok(())
    }

    fn melee_lifesteal(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let input = "melee_lifesteal";
        let mut modifiers = rulesets::lifesteal::Modifiers::default();
        // Resolve unknown modifier IDs consistently, without inventing a melee
        // lifesteal engine enum (the catalog has no such provided-property type).
        for entry in &inputs.active {
            if let Some(id) = entry.modifier_subclass {
                self.modifier(id, entry.ability_subclass);
            }
        }
        for owner in &inputs.owned {
            for name in ["MeleeLifesteal", "TargetLifesteal"] {
                if self.mode == StatMode::Baseline
                    && (name == "TargetLifesteal"
                        || owner.definition["m_eAbilityActivation"]
                            != "CITADEL_ABILITY_ACTIVATION_PASSIVE")
                {
                    continue;
                }
                let Some(property) = owner.definition["m_mapAbilityProperties"].get(name) else {
                    continue;
                };
                let path = format!("{}/m_mapAbilityProperties/{name}", owner.definition_path);
                let effect = json!({
                    "stat": input, "property_name": name, "definition_path": path,
                    "value": number(&property["m_strValue"]),
                    "scaling": property["m_subclassScaleFunction"],
                });
                let value = self.effect(&effect, owner, None)?;
                if value == 0.0 {
                    continue;
                }
                if name == "TargetLifesteal" {
                    return Err(invalid(format!(
                        "TargetLifesteal in {} requires target and damage-type context",
                        owner.record_key,
                    )));
                }
                if owner.definition["m_eAbilityActivation"] != "CITADEL_ABILITY_ACTIVATION_PASSIVE"
                {
                    self.unmapped_inputs.insert(format!(
                        "MeleeLifesteal in {} has no passive activation binding",
                        owner.record_key,
                    ));
                    continue;
                }
                // Approved property rule: an owned passive's exact MeleeLifesteal
                // property is nominal melee lifesteal. VData omits its stat binding;
                // disclose this assumption. Values and upgrades remain catalog data.
                self.inferred_bindings.insert(format!(
                    "MeleeLifesteal in {} -> owned passive ability (property rule)",
                    owner.record_key,
                ));
                modifiers.add(value)?;
                self.record(input, "property", value, owner, &path, None);
            }
        }
        // Cooldown-based healing properties are separate procs, not continuous
        // lifesteal. In particular, do not match names containing 'lifesteal'.
        Ok(modifiers.calculate())
    }

    fn stamina(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let base = self.hero_base(inputs, "EStamina", "stamina", "base")?;
        let symbol = "MODIFIER_VALUE_STAMINA";
        let (mut flat, _) = self.bound_total(inputs, symbol, "stamina", "flat")?;
        if inputs.hero.definition["m_mapStandardLevelUpUpgrades"]
            .get(symbol)
            .is_some()
        {
            flat += self.level_bonus(inputs, symbol, "stamina", "boon_flat")?;
        }
        flat += self.purchase_bonus(inputs, symbol, "stamina", "purchase_flat")?;
        rulesets::stamina::calculate(base, flat)
    }

    fn stamina_cooldown(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let input = "stamina_cooldown";
        let base = self.hero_base(inputs, "EStaminaRegenPerSecond", input, "base_per_second")?;
        for entry in &inputs.active {
            let Some(source) = self.modifier(
                entry
                    .modifier_subclass
                    .ok_or_else(|| invalid("modifier has no ID"))?,
                entry.ability_subclass,
            ) else {
                continue;
            };
            if source.definition["m_nEnabledStateMask"]
                .as_str()
                .is_some_and(|states| {
                    states
                        .split('|')
                        .any(|state| state.trim() == "MODIFIER_STATE_STAMINA_REGEN_PAUSED")
                })
            {
                return Err(invalid(format!(
                    "stamina recovery is paused by {}",
                    source.record_key
                )));
            }
        }
        let (flat, _) = self.bound_total(
            inputs,
            "MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_ADDITIVE",
            input,
            "flat_per_second",
        )?;
        let (percent, count) = self.bound_total(
            inputs,
            "MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE",
            input,
            "percent",
        )?;
        // VData supplies values and bindings, not the engine's stacking order.
        if count > 1 {
            return Err(invalid(
                "combining stamina recovery percentages is not supported by stamina_cooldown.v1",
            ));
        }
        rulesets::stamina_cooldown::calculate(base, flat, percent)
    }

    fn dash(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let air = matches!(stat, HeroStat::AirDashSpeed | HeroStat::AirDashDuration);
        let (distance_key, duration_key, symbol) = if air {
            (
                "EAirDashDistanceInMeters",
                "EAirDashDuration",
                "MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT",
            )
        } else {
            (
                "EGroundDashDistanceInMeters",
                "EGroundDashDuration",
                "MODIFIER_VALUE_MOVEMENT_GROUND_DASH_INCREASE_PERCENT",
            )
        };
        let input = stat.as_str();
        let duration = self.hero_base(inputs, duration_key, input, "duration_seconds")?;
        match stat {
            HeroStat::DashDuration => return rulesets::dash_duration::calculate(duration),
            HeroStat::AirDashDuration => return rulesets::air_dash_duration::calculate(duration),
            _ => {}
        }
        let distance = self.hero_base(inputs, distance_key, input, "distance_metres")?;
        let (mut percent, mut count) =
            self.bound_total(inputs, symbol, input, "distance_percent")?;
        if !air {
            // Reductions are signed negative values in the catalog.
            let (reduction, reductions) = self.bound_total(
                inputs,
                "MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT",
                input,
                "distance_percent",
            )?;
            percent += reduction;
            count += reductions;
        }
        if count > 1 {
            return Err(invalid(
                "combining dash-distance percentages is not supported by dash speed v1",
            ));
        }
        // Ordinary dash timing stays fixed when distance changes in the checked
        // replay. This is nominal average speed, not an instantaneous speed or
        // a claim about flight, attack movement, or interrupted dashes.
        if air {
            rulesets::air_dash_speed::calculate(distance, duration, percent)
        } else {
            rulesets::dash_speed::calculate(distance, duration, percent)
        }
    }

    fn gravity_scale(&mut self) -> Result<f64> {
        if self.mode == StatMode::Baseline {
            return Err(invalid(
                "baseline gravity scale is unavailable: the replay records only the current pawn value",
            ));
        }
        let pawn = hero_pawn(self.ctx, self.controller)
            .ok_or_else(|| invalid("player has no current pawn"))?;
        let base = required(self.ctx, pawn, "m_flGravityScale")?;
        if self.explain {
            self.contributions.push(Contribution {
                mode: self.mode,
                tick: self.ctx.tick(),
                steam_id: self.steam_id,
                hero_id: self.hero_id,
                stat: self.stat,
                input: "gravity_scale".into(),
                kind: "base",
                value: base,
                source: "replay/player_pawn".into(),
                definition_path: "m_flGravityScale".into(),
                modifier_serial: None,
            });
        }
        rulesets::gravity_scale::calculate(base)
    }

    fn melee_distance(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let percent = self.total(
            "MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE",
            "melee_distance",
            "percent",
            inputs,
        )?;
        rulesets::melee_distance::calculate(percent)
    }

    fn reload_time(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let weapon = inputs.weapon;
        let (info, path) = weapon.weapon_info();
        if info["m_bReloadUseActiveWeaponInfoDuration"] == true {
            return Err(invalid(
                "dynamic weapon reload duration is not supported by reload_time.v1",
            ));
        }
        if inputs
            .hero
            .definition
            .pointer("/m_mapScalingStats/EReloadSpeed")
            .is_some()
        {
            return Err(invalid(
                "hero reload scaling is not supported by reload_time.v1",
            ));
        }
        let base = number(&info["m_reloadDuration"])
            .ok_or_else(|| invalid("weapon has no base reload duration"))?;
        self.record(
            "reload_time",
            "base",
            base,
            weapon,
            &format!("{}/{path}/m_reloadDuration", weapon.definition_path),
            None,
        );
        let (percent, count) = self.total_and_count(
            "MODIFIER_VALUE_RELOAD_SPEED",
            "reload_time",
            "percent",
            inputs,
        )?;
        if count > 1 {
            return Err(invalid(
                "combining reload-time adjustments is not supported by reload_time.v1",
            ));
        }
        rulesets::reload_time::calculate(base, percent)
    }

    fn fire_rate(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let mut modifiers = rulesets::fire_rate::Modifiers::default();
        modifiers.add(self.spirit_scaled_bonus(inputs, "EFireRate", "fire_rate", "percent")?)?;
        self.for_each_value(
            "MODIFIER_VALUE_FIRE_RATE",
            "fire_rate",
            "percent",
            inputs,
            |value| modifiers.add(value),
        )?;
        self.for_each_value(
            "MODIFIER_VALUE_FIRE_RATE_SLOW",
            "fire_rate",
            "slow",
            inputs,
            |value| {
                if !(0.0..=100.0).contains(&value) {
                    return Err(invalid("fire-rate slow must be between 0 and 100 percent"));
                }
                modifiers.add(-value)
            },
        )?;
        modifiers.calculate()
    }

    fn level_bonus(
        &mut self,
        inputs: &PlayerInputs<'_>,
        stat: &str,
        input: &str,
        kind: &'static str,
    ) -> Result<f64> {
        let hero = inputs.hero;
        let level = inputs
            .level
            .ok_or_else(|| invalid("missing replay hero level"))?;
        if level < 0.0 || !level.is_finite() || level.fract() != 0.0 {
            return Err(invalid("invalid replay hero level"));
        }
        let levels = hero.definition["m_mapLevelInfo"]
            .as_object()
            .ok_or_else(|| invalid("missing hero level definitions"))?;
        let standard = levels
            .iter()
            .filter(|(key, v)| {
                key.parse::<f64>().is_ok_and(|n| n <= level) && v["m_bUseStandardUpgrade"] == true
            })
            .count();
        let per_level = match hero.definition["m_mapStandardLevelUpUpgrades"].get(stat) {
            Some(value) => number(value).ok_or_else(|| invalid("invalid hero boon bonus"))?,
            None => 0.0,
        };
        let total = per_level * standard as f64;
        self.record(
            input,
            kind,
            total,
            hero,
            &format!(
                "{}/m_mapStandardLevelUpUpgrades/{stat}",
                hero.definition_path
            ),
            None,
        );
        Ok(total)
    }

    fn purchase_bonus(
        &mut self,
        inputs: &PlayerInputs<'_>,
        stat: &str,
        input: &str,
        kind: &'static str,
    ) -> Result<f64> {
        let hero = inputs.hero;
        if let Some(tables) = hero.definition["m_MapModCostBonuses"].as_object() {
            let mut total = 0.0;
            for (slot, table) in tables {
                let matches_stat = if let Some(bonuses) =
                    hero.definition["m_mapPurchaseBonuses"][slot].as_array()
                {
                    bonuses.iter().any(|b| b["m_ValueType"] == stat)
                } else {
                    // Engine category semantics, not item or hero balance values.
                    // New VData retains cost tables but omits the legacy stat binding.
                    // Prices, thresholds and amounts still come from the catalog.
                    matches!(
                        (slot.as_str(), stat),
                        ("EItemSlotType_WeaponMod", WEAPON_DAMAGE) | ("EItemSlotType_Tech", SPIRIT)
                    )
                };
                if !matches_stat {
                    continue;
                }
                let mut invested = 0.0;
                for item in &inputs.inventory {
                    if item.definition["m_eItemSlotType"] != *slot {
                        continue;
                    }
                    let tier = item.definition["m_iItemTier"]
                        .as_str()
                        .and_then(|s| s.strip_prefix("EModTier_"))
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| {
                            invalid(format!("missing item tier for {}", item.record_key))
                        })?;
                    let price = self.catalog.generic_data["m_nItemPricePerTier"]
                        .get(tier)
                        .and_then(number)
                        .filter(|price| *price >= 0.0)
                        .ok_or_else(|| invalid("missing catalog item prices in misc.json; rebuild boon-data with generic_data.vdata, then run `boon get VERSION --force`"))?;
                    invested += price;
                    self.record(
                        input,
                        "purchase_cost",
                        price,
                        item,
                        &format!("generic_data.vdata#/m_nItemPricePerTier/{tier}"),
                        None,
                    );
                }
                let value = purchase_threshold_bonus(table, invested)?;
                total += value;
                self.record(
                    input,
                    kind,
                    value,
                    hero,
                    &format!("{}/m_MapModCostBonuses/{slot}", hero.definition_path),
                    None,
                );
            }
            return Ok(total);
        }
        let mut total = 0.0;
        for item in &inputs.inventory {
            let Some(slot) = item.definition["m_eItemSlotType"].as_str() else {
                continue;
            };
            let tier = item.definition["m_iItemTier"]
                .as_str()
                .and_then(|s| s.strip_prefix("EModTier_"))
                .and_then(|s| s.parse::<u64>().ok());
            if let Some(bonuses) = hero.definition["m_mapPurchaseBonuses"][slot].as_array() {
                if tier.is_none() && bonuses.iter().any(|bonus| bonus["m_ValueType"] == stat) {
                    return Err(invalid(format!(
                        "missing item tier for {}",
                        item.record_key
                    )));
                }
                for bonus in bonuses {
                    if bonus["m_ValueType"] == stat
                        && tier.is_some()
                        && bonus["m_nTier"].as_u64() == tier
                    {
                        let value = number(&bonus["m_strValue"])
                            .ok_or_else(|| invalid("invalid purchase bonus"))?;
                        total += value;
                        self.record(
                            input,
                            kind,
                            value,
                            item,
                            &format!("{}/m_mapPurchaseBonuses/{slot}", hero.definition_path),
                            None,
                        );
                    }
                }
            }
        }
        Ok(total)
    }

    fn weapon_damage(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let input = "weapon_damage";
        let mut modifiers = rulesets::weapon_damage::Modifiers::default();
        modifiers.add(self.purchase_bonus(inputs, WEAPON_DAMAGE, input, "purchase_percent")?)?;
        if inputs.hero.definition["m_mapStandardLevelUpUpgrades"]
            .get(WEAPON_DAMAGE)
            .is_some()
        {
            modifiers.add(self.level_bonus(inputs, WEAPON_DAMAGE, input, "boon_percent")?)?;
        }
        self.visit_values(
            WEAPON_DAMAGE,
            input,
            "percent",
            inputs,
            ValuePolicy::Registered,
            |value| modifiers.add(value),
        )?;
        self.missing_intrinsics(inputs, WEAPON_DAMAGE)?;
        for owner in &inputs.owned {
            self.unbound_weapon_damage(owner);
            for effect in &owner.stat_changes {
                if effect["stat"] != WEAPON_DAMAGE
                    || !self.mode.includes_effect(effect)
                    || effect.get("runtime_counts").is_none()
                {
                    continue;
                }
                if effect["modifier_keys"]
                    .as_array()
                    .is_some_and(|keys| !keys.is_empty())
                {
                    return Err(invalid("owned counter effect also has a modifier binding"));
                }
                let count = self.owned_runtime_count(owner, effect, input)?;
                let value = self.effect(effect, owner, None)? * count;
                modifiers.add(value)?;
                self.record(
                    input,
                    "percent",
                    value,
                    owner,
                    effect["definition_path"]
                        .as_str()
                        .unwrap_or(&owner.definition_path),
                    None,
                );
            }
        }
        // A recipient marker does not grant its source ability's unbound owner
        // rewards. Explicit effects on foreign modifiers are still read above.
        // Legacy weapon power is not proven to be a percentage. Do not silently
        // substitute it for the explicit weapon-damage-increase modifier value.
        let mut legacy_power = false;
        self.visit_values(
            "MODIFIER_VALUE_WEAPON_POWER",
            input,
            "unsupported_weapon_power",
            inputs,
            ValuePolicy::Registered,
            |value| {
                legacy_power |= value != 0.0;
                Ok(())
            },
        )?;
        if legacy_power {
            self.unmapped_inputs
                .insert("weapon power has no verified percentage conversion".into());
        }
        for (key, neutral) in [("EWeaponPower", 0.0), ("EWeaponPowerScale", 1.0)] {
            if inputs.hero.definition["m_mapStartingStats"]
                .get(key)
                .is_some_and(|value| number(value) != Some(neutral))
                || inputs.hero.definition["m_mapScalingStats"]
                    .get(key)
                    .is_some()
            {
                self.unmapped_inputs.insert(format!(
                    "{key} has no verified weapon-damage percentage conversion"
                ));
            }
        }
        Ok(modifiers.calculate())
    }

    fn owned_ability(&self, owner: &Record) -> Result<&'a Entity> {
        let ability_id = owner
            .ability_id
            .ok_or_else(|| invalid("counter has no owning ability"))?;
        let pawn = hero_pawn(self.ctx, self.controller)
            .ok_or_else(|| invalid("counter has no owning pawn"))?;
        let vector = "m_CCitadelAbilityComponent.m_vecAbilities";
        for i in 0..count(self.ctx, pawn, vector)? {
            if let Some(ability) = handle(self.ctx, pawn, &format!("{vector}.{i}"))
                && id(self.ctx, ability, "m_nSubclassID")? == ability_id
            {
                return Ok(ability);
            }
        }
        Err(invalid("counter has no owned ability entity"))
    }

    fn owned_runtime_count(&mut self, owner: &Record, effect: &Value, input: &str) -> Result<f64> {
        let ability = self.owned_ability(owner)?;
        let terms = effect["runtime_counts"]
            .as_array()
            .filter(|terms| !terms.is_empty())
            .ok_or_else(|| invalid("invalid catalog runtime counts"))?;
        let mut total = 0.0;
        for term in terms {
            let path = term["field"]
                .as_str()
                .ok_or_else(|| invalid("invalid counter field"))?;
            let count = checked_runtime_count(required(self.ctx, ability, path)?, path)?;
            self.record(input, "runtime_count", count, owner, path, None);
            let percent = if let Some(property) = term.get("percent") {
                let percent = self.effect(property, owner, None)?;
                if percent < 0.0 {
                    return Err(invalid("negative runtime count weight"));
                }
                self.record(
                    input,
                    "runtime_count_percent",
                    percent,
                    owner,
                    property["definition_path"]
                        .as_str()
                        .unwrap_or(&owner.definition_path),
                    None,
                );
                percent
            } else {
                100.0
            };
            total += count * percent / 100.0;
        }
        if !total.is_finite() {
            return Err(invalid("nonfinite runtime count total"));
        }
        Ok(total)
    }

    fn unbound_weapon_damage(&mut self, owner: &Record) {
        for effect in &owner.stat_changes {
            if effect["stat"] != WEAPON_DAMAGE
                || !self.mode.includes_effect(effect)
                || effect.get("runtime_counts").is_some()
                || effect["modifier_keys"]
                    .as_array()
                    .is_some_and(|keys| !keys.is_empty())
                || empty_declaration(owner, effect)
            {
                continue;
            }
            if let Some(name) = effect["property_name"].as_str()
                && owner.definition["m_mapAbilityProperties"][name]["m_strConditionalLocTokenOverride"]
                    == "#EnemyAboveHealthThreshold_conditional"
            {
                // This explicit condition label describes the enemy target.
                // It is outside a global weapon stat; no item identity or health
                // threshold is inferred or evaluated here.
                continue;
            }
            let upgraded = effect["property_name"]
                .as_str()
                .is_some_and(|name| owner.upgrades_property(name));
            if number(&effect["value"]) == Some(0.0) && effect["scaling"].is_null() && !upgraded {
                continue;
            }
            // Even intrinsic declarations can specify a per-kill increment.
            // Neither ownership nor an arbitrary live modifier proves its count
            // or activation. Keep a partial subtotal until a binding is supplied.
            self.unmapped_inputs.insert(format!(
                "{} in {} has no modifier binding or recorded total",
                effect["property_name"].as_str().unwrap_or(WEAPON_DAMAGE),
                owner.record_key,
            ));
        }
        // Some VData retains this exact property without a provided-stat type.
        // Detect the missing mapping; do not infer its activation from the name.
        if let Some(property) =
            owner.definition["m_mapAbilityProperties"].get("BaseAttackDamagePercent")
            && ((!property["m_strValue"].is_null()
                && !property["m_strValue"]
                    .as_str()
                    .is_some_and(|value| value.trim().is_empty())
                && number(&property["m_strValue"]) != Some(0.0))
                || owner.upgrades_property("BaseAttackDamagePercent"))
            && !owner
                .stat_changes
                .iter()
                .any(|effect| effect["property_name"] == "BaseAttackDamagePercent")
        {
            self.unmapped_inputs.insert(format!(
                "BaseAttackDamagePercent in {} has no stat mapping",
                owner.record_key,
            ));
        }
    }

    fn melee_damage(&mut self, inputs: &PlayerInputs<'_>, stat: HeroStat) -> Result<f64> {
        let key = match stat {
            HeroStat::LightMeleeDamage => "ELightMeleeDamage",
            HeroStat::HeavyMeleeDamage => "EHeavyMeleeDamage",
            _ => return Err(invalid("expected a melee damage stat")),
        };
        let hero = inputs.hero;
        let starting = &hero.definition["m_mapStartingStats"];
        let base =
            number(&starting[key]).ok_or_else(|| invalid("hero has no base melee damage"))?;
        let light = number(&starting["ELightMeleeDamage"])
            .ok_or_else(|| invalid("hero has no base light melee damage"))?;
        let input = stat.as_str();
        self.record(
            input,
            "base",
            base,
            hero,
            &format!("{}/m_mapStartingStats/{key}", hero.definition_path),
            None,
        );
        if stat == HeroStat::HeavyMeleeDamage {
            self.record(
                input,
                "base_light_reference",
                light,
                hero,
                &format!(
                    "{}/m_mapStartingStats/ELightMeleeDamage",
                    hero.definition_path
                ),
                None,
            );
        }
        let growth = self.level_bonus(
            inputs,
            "MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL",
            input,
            "boon_flat",
        )?;
        let spirit = self.spirit_scaled_bonus(inputs, key, input, "flat")?;
        let trace_start = self.contributions.len();
        let weapon = self.weapon_damage(inputs)?;
        // Keep dependency sources in the requested melee trace. These are full
        // weapon percentages; the equation applies the 50% factor exactly once.
        if self.explain {
            for row in &mut self.contributions[trace_start..] {
                if row.input == "weapon_damage" {
                    row.input = input.into();
                    if matches!(row.kind, "percent" | "purchase_percent" | "boon_percent") {
                        row.kind = "weapon_percent";
                    }
                }
            }
        }
        let melee_symbol = "MODIFIER_VALUE_MELEE_DAMAGE_INCREASE";
        let (melee, _) = self.bound_total(inputs, melee_symbol, input, "percent")?;
        for owner in &inputs.owned {
            self.unbound_property(owner, melee_symbol, None)?;
        }
        // Only a registered live effect establishes a global multiplier.
        // Unbound source-ability properties can describe damage to a particular
        // target, rather than a change to this player's nominal melee stat.
        for unsupported in [
            "MODIFIER_VALUE_MELEE_DAMAGE_MULTIPLIER",
            "MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER",
        ] {
            self.visit_values(
                unsupported,
                input,
                "unsupported",
                inputs,
                ValuePolicy::Registered,
                |value| {
                    if value == 0.0 {
                        Ok(())
                    } else {
                        Err(invalid(format!(
                            "{unsupported} is not supported by melee damage v1"
                        )))
                    }
                },
            )?;
        }
        rulesets::melee_damage::calculate(base, light, growth, spirit, weapon, melee)
    }

    fn spirit_scaled_bonus(
        &mut self,
        inputs: &PlayerInputs<'_>,
        stat: &str,
        input: &str,
        kind: &'static str,
    ) -> Result<f64> {
        let hero = inputs.hero;
        let Some(scale) = hero.definition["m_mapScalingStats"].get(stat) else {
            return Ok(0.0);
        };
        if scale["eScalingStat"].as_str() != Some("ETechPower") {
            return Err(invalid(format!("unsupported hero {input} scaling stat")));
        }
        let factor = number(&scale["flScale"])
            .ok_or_else(|| invalid(format!("missing {input} scaling coefficient")))?;
        let bonus = if factor == 0.0 {
            0.0
        } else {
            self.spirit(inputs)? * factor
        };
        self.record(
            input,
            kind,
            bonus,
            hero,
            &format!("{}/m_mapScalingStats/{stat}", hero.definition_path),
            None,
        );
        Ok(bonus)
    }

    fn spirit(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let hero = inputs.hero;
        let base = hero
            .definition
            .pointer("/m_mapStartingStats/ETechPower")
            .and_then(number)
            .unwrap_or(0.0);
        self.record(
            "spirit_power",
            "base",
            base,
            hero,
            &format!("{}/m_mapStartingStats/ETechPower", hero.definition_path),
            None,
        );
        let total = base
            + self.level_bonus(inputs, SPIRIT, "spirit_power", "flat")?
            + self.purchase_bonus(inputs, SPIRIT, "spirit_power", "flat")?;
        let (mut flat, mut post_multiplier) = (0.0, 0.0);
        let flat_result = self.visit_effect_values(
            SPIRIT,
            "spirit_power",
            "flat",
            inputs,
            ValuePolicy::Declared,
            |value, effect| {
                match effect.and_then(|effect| effect.get("calculation_stage")) {
                    None | Some(Value::Null) => flat += value,
                    Some(stage) if stage == "pre_multiplier" => flat += value,
                    Some(stage) if stage == "post_multiplier" => post_multiplier += value,
                    Some(stage) => {
                        return Err(invalid(format!("unsupported spirit-power stage {stage}")));
                    }
                }
                Ok(())
            },
        );
        // Trace percentage inputs even if an unbound flat effect prevents a total.
        let mut modifiers = rulesets::spirit_power::Modifiers::default();
        let percent = self.for_each_value(
            "MODIFIER_VALUE_TECH_POWER_PERCENT",
            "spirit_power",
            "percent",
            inputs,
            |value| modifiers.add(value),
        );
        flat_result?;
        percent?;
        // Catalog bindings can declare a flat bonus after the multipliers.
        // Ability-only inputs are excluded by the same apply filter in both stages.
        modifiers.calculate(total + flat, post_multiplier)
    }

    fn total(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
    ) -> Result<f64> {
        self.total_and_count(stat, input, kind, inputs)
            .map(|(value, _)| value)
    }

    fn total_and_count(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
    ) -> Result<(f64, usize)> {
        let mut total = 0.0;
        let mut count = 0;
        self.for_each_value(stat, input, kind, inputs, |value| {
            total += value;
            count += usize::from(value != 0.0);
            Ok(())
        })?;
        Ok((total, count))
    }

    fn for_each_value(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
        add: impl FnMut(f64) -> Result<()>,
    ) -> Result<()> {
        self.visit_values(stat, input, kind, inputs, ValuePolicy::Declared, add)
    }

    fn global_effect(&mut self, source: &Record, effect: &Value) -> bool {
        use abilities::EffectScope;
        let filter = abilities::apply_filter(self.catalog, source, effect);
        match abilities::scope(filter) {
            EffectScope::Global => true,
            EffectScope::Unknown => {
                self.unmapped_inputs.insert(format!(
                    "unknown apply filter {} in {}",
                    filter.unwrap_or_default(),
                    source.record_key
                ));
                false
            }
            // An ability-specific property cannot contribute to a nominal hero
            // stat, including spirit power used for weapon or hero scaling.
            _ => false,
        }
    }

    fn visit_values(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
        policy: ValuePolicy,
        mut add: impl FnMut(f64) -> Result<()>,
    ) -> Result<()> {
        self.visit_effect_values(stat, input, kind, inputs, policy, |value, _| add(value))
    }

    fn visit_effect_values(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
        policy: ValuePolicy,
        mut add: impl FnMut(f64, Option<&Value>) -> Result<()>,
    ) -> Result<()> {
        let explicit_only = policy != ValuePolicy::Declared;
        let permanent = if policy == ValuePolicy::Recipient {
            &[][..]
        } else {
            &inputs.permanent
        };
        let owned = if matches!(policy, ValuePolicy::Recipient | ValuePolicy::Registered) {
            &[][..]
        } else {
            &inputs.owned
        };
        // The stat viewer contains cumulative recorded values, not pickup counts.
        let vector = "m_PlayerDataGlobal.m_vecStatViewerModifierValues";
        for (i, recorded) in permanent.iter().enumerate() {
            if recorded.value == 0.0 || self.recorded_stat(recorded)? != Some(stat) {
                continue;
            }
            let value = modifier_units(stat, recorded.value);
            add(value, None)?;
            if self.explain {
                let source = self.recorded_source(recorded.source_id);
                self.contributions.push(Contribution {
                    mode: self.mode,
                    tick: self.ctx.tick(),
                    steam_id: self.steam_id,
                    hero_id: self.hero_id,
                    stat: self.stat,
                    input: input.into(),
                    kind,
                    value,
                    source,
                    definition_path: format!("{vector}.{i}.m_flValue"),
                    modifier_serial: None,
                });
            }
        }
        // Unbound conditional properties use an explicit or inferred activation link.
        // Bound properties are read below from their live modifier instance, once.
        for item in owned {
            for effect in &item.stat_changes {
                if effect["stat"] != stat
                    || !self.mode.includes_effect(effect)
                    || !self.global_effect(item, effect)
                    || effect["modifier_keys"]
                        .as_array()
                        .is_some_and(|v| !v.is_empty())
                {
                    continue;
                }
                if effect["value"].is_null()
                    && effect["raw_value"].is_null()
                    && effect["scaling"].is_null()
                {
                    // An inherited property declaration can have no assigned value.
                    continue;
                }
                // Unregistered passive declarations can outlive an old effect in
                // VData. Only explicit intrinsic or conditional flags give them
                // meaning without a registered modifier property.
                if effect["usage_flags"].as_str().is_none_or(|flags| {
                    !flags.contains("IntrinsicallyProvidedInAbility")
                        && !flags.contains("ConditionallyApplied")
                }) {
                    if !explicit_only && !self.effect(effect, item, None).is_ok_and(|v| v == 0.0) {
                        self.unmapped_inputs.insert(format!(
                            "{} in {} has no modifier binding or intrinsic usage flag",
                            effect["property_name"].as_str().unwrap_or(stat),
                            item.record_key
                        ));
                    }
                    continue;
                }
                let value = self.effect(effect, item, None)?;
                if explicit_only
                    && !effect["usage_flags"]
                        .as_str()
                        .is_some_and(|s| s.contains("IntrinsicallyProvidedInAbility"))
                {
                    if value != 0.0
                        && effect["usage_flags"]
                            .as_str()
                            .is_some_and(|flags| flags.contains("ConditionallyApplied"))
                    {
                        self.unmapped_inputs.insert(format!(
                            "{} in {} has no modifier binding",
                            effect["property_name"].as_str().unwrap_or(stat),
                            item.record_key
                        ));
                    }
                    continue;
                }
                if effect["usage_flags"]
                    .as_str()
                    .is_some_and(|s| s.contains("ConditionallyApplied"))
                {
                    if value != 0.0 {
                        let modifier = item
                            .ability_id
                            .and_then(|id| self.catalog.conditional_modifier(id));
                        if let Some(modifier) = modifier {
                            // Assumption: absence of the owner's unique effect modifier
                            // also means these conditional bonuses are inactive. Report
                            // the inferred link even when it contributes zero.
                            self.infer_binding(effect, item, modifier);
                        } else {
                            return Err(invalid(format!(
                                "unresolved conditional property {} in {}: catalog has no activation binding or unique non-intrinsic modifier",
                                effect["property_name"].as_str().unwrap_or(stat),
                                item.record_key,
                            )));
                        }
                    }
                    continue;
                }
                if let Some(message) = self.corruption_diagnostic(effect, item, None)? {
                    self.unmapped_inputs.insert(message);
                }
                add(value, Some(effect))?;
                let kind = contribution_kind(kind, effect);
                self.record(
                    input,
                    kind,
                    value,
                    item,
                    effect["definition_path"]
                        .as_str()
                        .unwrap_or(&item.definition_path),
                    None,
                );
            }
        }
        let mut seen_properties = HashSet::new();
        for entry in &inputs.active {
            let Some(source) = self.modifier(
                entry
                    .modifier_subclass
                    .ok_or_else(|| invalid("modifier has no ID"))?,
                entry.ability_subclass,
            ) else {
                continue;
            };
            // Permanent pickups are already represented by the recorded totals.
            if source
                .misc_id
                .and_then(|id| self.catalog.misc.get(&id))
                .is_some_and(|r| r.definition["m_bIsPermanentPickup"] == true)
            {
                continue;
            }
            if explicit_only
                && policy != ValuePolicy::Registered
                && let Some(ability) = source
                    .ability_id
                    .and_then(|id| self.catalog.abilities.get(&id))
                && (policy != ValuePolicy::Recipient || !source.is_intrinsic_modifier_of(ability))
            {
                for effect in &ability.stat_changes {
                    if effect["stat"] == stat
                        && self.global_effect(ability, effect)
                        && effect["modifier_keys"]
                            .as_array()
                            .is_none_or(|keys| keys.is_empty())
                        && effect["usage_flags"]
                            .as_str()
                            .is_some_and(|flags| flags.contains("ConditionallyApplied"))
                        && number(&effect["value"]) != Some(0.0)
                    {
                        self.unmapped_inputs.insert(format!(
                            "{} in {} has no binding to recipient modifier {}",
                            effect["property_name"].as_str().unwrap_or(stat),
                            ability.record_key,
                            source.record_key
                        ));
                    }
                }
            }
            let owner = source
                .ability_id
                .and_then(|id| self.catalog.abilities.get(&id))
                .filter(|ability| !explicit_only && source.is_effect_modifier_of(ability));
            let inferred = owner
                .into_iter()
                .flat_map(|ability| &ability.stat_changes)
                .filter(|effect| {
                    effect["usage_flags"]
                        .as_str()
                        .is_some_and(|s| s.contains("ConditionallyApplied"))
                        && effect["modifier_keys"]
                            .as_array()
                            .is_none_or(|keys| keys.is_empty())
                        && (!effect["value"].is_null()
                            || !effect["raw_value"].is_null()
                            || !effect["scaling"].is_null())
                })
                .map(|effect| (effect, owner));
            for (effect, inferred_owner) in source
                .stat_changes
                .iter()
                .map(|effect| (effect, None))
                .chain(inferred)
            {
                if effect["stat"] != stat
                    || different_effect_owner(effect, entry)
                    || !self.mode.includes_effect(effect)
                    || !self.global_effect(source, effect)
                {
                    continue;
                }
                if entry.duration.is_some_and(|d| d > 0.0)
                    && (self.game_time.is_none() || entry.last_applied_time.is_none())
                {
                    return Err(invalid("cannot determine timed modifier expiry"));
                }
                if effect["runtime_count"].is_null() && entry.stack_count.is_some_and(|n| n > 1) {
                    return Err(invalid(format!(
                        "unresolved stacking rule for {}",
                        source.record_key
                    )));
                }
                if policy == ValuePolicy::Recipient
                    && let Some(property) = effect["property_name"].as_str()
                    && let Some(owner) = source
                        .ability_id
                        .or(entry.ability_subclass)
                        .and_then(|id| self.catalog.abilities.get(&id))
                    && owner.upgrades_property(property)
                    && entry
                        .caster
                        .and_then(|handle| self.ctx.entities().get_by_handle(handle))
                        .is_none()
                {
                    // Applying the victim's upgrade tier to an enemy debuff would
                    // be incorrect. Resolve the caster or leave the result unknown.
                    return Err(invalid(
                        "cannot resolve resistance-reduction caster upgrades",
                    ));
                }
                let mut value = self.effect(effect, source, Some(entry))?;
                if let Some(counter) = effect.get("runtime_count").filter(|c| !c.is_null()) {
                    let path = runtime_count_field(counter)?;
                    let count = self.runtime_count(source, entry, counter)?;
                    self.record(
                        input,
                        "runtime_count",
                        count,
                        source,
                        path,
                        entry.serial_number,
                    );
                    value = checked_counter_contribution(value, count)?;
                }
                if let Some(ability) = inferred_owner
                    && value != 0.0
                {
                    if !ability
                        .ability_id
                        .and_then(|id| self.catalog.conditional_modifier(id))
                        .is_some_and(|modifier| std::ptr::eq(modifier, source))
                    {
                        return Err(invalid(format!(
                            "no unique conditional modifier for {}",
                            ability.record_key
                        )));
                    }
                    if !entry.duration.is_some_and(|d| d.is_finite() && d > 0.0) {
                        return Err(invalid(format!(
                            "cannot infer conditional activation from untimed modifier {}",
                            source.record_key
                        )));
                    }
                }
                let path = effect["definition_path"]
                    .as_str()
                    .unwrap_or(&source.definition_path);
                // A bound property can be listed more than once within one record.
                if !seen_properties.insert((entry.serial_number, path)) {
                    continue;
                }
                if let Some(ability) = inferred_owner {
                    if value == 0.0 {
                        continue;
                    }
                    // Assumption: this temporary modifier activates the unbound
                    // conditional bonuses of its unique owning ability. Ownership
                    // is explicit; activation is inferred, not an engine guarantee.
                    self.infer_binding(effect, ability, source);
                }
                if effect["kind"] == "inferred_property"
                    && let Some(ability) = source
                        .ability_id
                        .and_then(|id| self.catalog.abilities.get(&id))
                {
                    self.infer_binding(effect, ability, source);
                }
                if let Some(message) = self.corruption_diagnostic(effect, source, Some(entry))? {
                    self.unmapped_inputs.insert(message);
                }
                add(value, Some(effect))?;
                let kind = contribution_kind(kind, effect);
                self.record(input, kind, value, source, path, entry.serial_number);
            }
        }
        Ok(())
    }

    fn runtime_count(
        &self,
        source: &Record,
        entry: &CModifierTableEntry,
        counter: &Value,
    ) -> Result<f64> {
        let path = runtime_count_field(counter)?;
        let value = match counter
            .as_str()
            .map(|_| "ability")
            .or_else(|| counter["source"].as_str())
        {
            Some("ability") => {
                let ability_id = source
                    .ability_id
                    .ok_or_else(|| invalid("counter has no owning ability"))?;
                let ability = entry
                    .ability
                    .and_then(|h| self.ctx.entities().get_by_handle(h))
                    .ok_or_else(|| invalid("counter has no owning ability entity"))?;
                if id(self.ctx, ability, "m_nSubclassID")? != ability_id {
                    return Err(invalid("counter ability does not match the catalog owner"));
                }
                required(self.ctx, ability, path)?
            }
            Some("modifier") => {
                let count = match path {
                    "stack_count" => entry.stack_count,
                    _ => return Err(invalid("unsupported modifier counter field")),
                };
                count
                    .map(f64::from)
                    .ok_or_else(|| invalid("modifier counter is missing"))?
            }
            _ => return Err(invalid("unsupported catalog runtime count source")),
        };
        let divisor = match counter.get("divisor") {
            Some(value) => number(value)
                .filter(|n| n.is_finite() && *n > 0.0)
                .ok_or_else(|| invalid("invalid catalog runtime count divisor"))?,
            None => 1.0,
        };
        let count = checked_runtime_count(value, path)? / divisor;
        if !count.is_finite() {
            return Err(invalid("nonfinite normalized runtime count"));
        }
        Ok(count)
    }

    fn corruption_diagnostic(
        &self,
        effect: &Value,
        source: &Record,
        entry: Option<&CModifierTableEntry>,
    ) -> Result<Option<String>> {
        let Some(owner) = source
            .ability_id
            .or_else(|| entry.and_then(|e| e.ability_subclass))
            .and_then(|id| self.catalog.abilities.get(&id))
        else {
            return Ok(None);
        };
        let Some(changes) =
            owner.definition["m_CorruptedItemInfo"]["m_Upgrade"]["m_vecPropertyUpgrades"]
                .as_array()
        else {
            return Ok(None);
        };
        if !changes.iter().any(|change| change["m_strPropertyName"] == effect["property_name"])
            // AbilityUpgradeBits_t::ABILITY_UPGRADE_BIT_CORRUPTED is a wire flag,
            // not an item ID or a balance value. `upgrades` excludes the trained bit.
            || self.upgrades(entry, owner.ability_id)? & (128 >> 1) == 0
        {
            return Ok(None);
        }
        Ok(Some(format!(
            "{} in {} has a corrupted upgrade that is not included; its exact value is not resolved",
            effect["property_name"].as_str().unwrap_or("property"),
            owner.record_key,
        )))
    }

    fn check_scaling(&self, effect: &Value) -> Result<()> {
        let Some(scale) = effect["scaling"]["$value"]
            .as_object()
            .filter(|s| !s.is_empty())
        else {
            return Ok(());
        };
        let stat = effect["stat"].as_str().unwrap_or_default();
        let scaling = &effect["scaling"]["$value"];
        let class = scaling["_class"].as_str().unwrap_or_default();
        let defaults = &self.catalog.scaling_class_defaults[class]["defaults"];
        let base_defaults = &self.catalog.scaling_class_defaults["CScaleFunctionVData"]["defaults"];
        // These inherited fields are common to scale functions. Do not infer a
        // class alias or equation from an absent class or stat declaration.
        let scaling_field = |name: &str| {
            scaling
                .get(name)
                .or_else(|| defaults.get(name))
                .or_else(|| base_defaults.get(name))
        };
        let disabled = match scaling_field("m_bFunctionDisabled") {
            None => false,
            Some(value) => value
                .as_bool()
                .ok_or_else(|| invalid("invalid scaling disabled flag"))?,
        };
        let nominal_lifesteal = matches!(
            stat,
            BULLET_LIFESTEAL | SPIRIT_LIFESTEAL | "melee_lifesteal"
        );
        // Nominal lifesteal is the fraction before healing boosts/reduction.
        let nominal_healing = nominal_lifesteal
            && class == "scale_function_single_stat"
            && scale
                .get("m_eSpecificStatScaleType")
                .and_then(Value::as_str)
                == Some("EHealingOutput");
        let zero_linear = matches!(
            class,
            "scale_function_single_stat" | "scale_function_tech_damage"
        ) && scaling_field("m_flStatScale").and_then(number) == Some(0.0);
        if disabled || nominal_healing || zero_linear {
            return Ok(());
        }
        Err(invalid(format!(
            "unsupported property scaling at {} (class={class}, stat={}, coefficient={})",
            effect["definition_path"],
            scaling_field("m_eSpecificStatScaleType").unwrap_or(&Value::Null),
            scaling_field("m_flStatScale").unwrap_or(&Value::Null),
        )))
    }

    fn effect(
        &self,
        effect: &Value,
        source: &Record,
        entry: Option<&CModifierTableEntry>,
    ) -> Result<f64> {
        if let Some(owner) = effect["source_ability_id"].as_u64()
            && entry.and_then(|e| e.ability_subclass).map(u64::from) != Some(owner)
        {
            return Err(invalid(
                "cannot resolve registered property's source ability",
            ));
        }
        self.check_scaling(effect)?;
        let stat = effect["stat"].as_str().unwrap_or_default();
        let mut value = if let Some(value) = number(&effect["value"]).or_else(|| {
            matches!(stat, MOVE_SPEED | SPRINT_SPEED)
                .then(|| speed_property_number(&effect["raw_value"], stat))
                .flatten()
        }) {
            value
        } else if let (Some(min), Some(max), Some(t_min), Some(t_max)) = (
            number(&effect["value_min"]),
            number(&effect["value_max"]),
            number(&source.definition["m_flTimeMin"]),
            number(&source.definition["m_flTimeMax"]),
        ) {
            // These time-ranged powerup values use match minutes at application.
            let applied = entry
                .and_then(|m| m.last_applied_time)
                .map(f64::from)
                .ok_or_else(|| invalid("powerup application time is missing"))?;
            let start = self
                .game_start
                .ok_or_else(|| invalid("match start time is missing"))?;
            if t_max <= t_min {
                return Err(invalid("invalid powerup time range"));
            }
            let fraction = (((applied - start) / 60.0 - t_min) / (t_max - t_min)).clamp(0.0, 1.0);
            min + (max - min) * fraction
        } else {
            return Err(invalid(format!(
                "unresolved stat value in {}",
                source.record_key
            )));
        };
        if let Some(property) = effect["property_name"].as_str() {
            let ability_id = source
                .ability_id
                .or_else(|| entry.and_then(|e| e.ability_subclass));
            if let Some(ability) = ability_id.and_then(|id| self.catalog.abilities.get(&id))
                && let Some(tiers) = ability.definition["m_vecAbilityUpgrades"].as_array()
            {
                // A modifier's count does not supply its caster's upgrade tier.
                // Require that caster for upgraded weighted effects; do not use
                // the recipient's upgrades when the source entity is unavailable.
                if effect["runtime_count"]["source"] == "modifier"
                    && entry.is_some()
                    && ability.upgrades_property(property)
                    && entry
                        .and_then(|e| e.caster)
                        .and_then(|h| self.ctx.entities().get_by_handle(h))
                        .is_none()
                {
                    return Err(invalid("cannot resolve modifier counter caster upgrades"));
                }
                let upgrades = self.upgrades(entry, ability_id)?;
                for (tier, upgrade) in tiers.iter().enumerate() {
                    if tier >= u32::BITS as usize || upgrades & (1 << tier) == 0 {
                        continue;
                    }
                    if let Some(properties) = upgrade["m_vecPropertyUpgrades"].as_array() {
                        for change in properties {
                            if change["m_strPropertyName"] == property {
                                if change["m_eUpgradeType"]
                                    .as_str()
                                    .is_some_and(|kind| kind != "EAddToBase")
                                {
                                    return Err(invalid(format!(
                                        "unsupported ability upgrade type at {}",
                                        effect["definition_path"]
                                    )));
                                }
                                value += speed_property_number(&change["m_strBonus"], stat)
                                    .ok_or_else(|| {
                                        invalid("unsupported ability upgrade expression")
                                    })?;
                            }
                        }
                    }
                }
            }
        }
        if !value.is_finite() {
            return Err(invalid("nonfinite stat contribution"));
        }
        Ok(modifier_units(stat, value))
    }

    fn upgrades(
        &self,
        entry: Option<&CModifierTableEntry>,
        ability_id: Option<u32>,
    ) -> Result<u32> {
        let controller = if let Some(caster) = entry
            .and_then(|e| e.caster)
            .and_then(|h| self.ctx.entities().get_by_handle(h))
        {
            self.ctx
                .entities()
                .iter()
                .find(|(_, e)| {
                    e.class_name.as_ref() == "CCitadelPlayerController"
                        && hero_pawn(self.ctx, e).is_some_and(|p| std::ptr::eq(p, caster))
                })
                .map(|(_, e)| e)
                .ok_or_else(|| invalid("cannot resolve modifier caster upgrades"))?
        } else {
            self.controller
        };
        let vector = "m_PlayerDataGlobal.m_vecAbilityUpgradeState";
        for i in 0..count(self.ctx, controller, vector)? {
            if Some(id(self.ctx, controller, &format!("{vector}.{i}.m_ItemID"))?) == ability_id {
                // This is the replay's packed upgrade state, not a balance value.
                return Ok(id(
                    self.ctx,
                    controller,
                    &format!("{vector}.{i}.m_nUpgradeInfo"),
                )? >> 17);
            }
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests;
