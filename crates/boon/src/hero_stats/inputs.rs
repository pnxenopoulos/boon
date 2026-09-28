//! Resolve replay inputs without embedding hero IDs, item values or enum numbers.
pub mod abilities;

use super::catalog::{Record, number};
use super::{
    CalculationError, Contribution, HeroStat, HeroStatQuery, PlayerSlot, StatCatalog, StatResult,
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

fn modifier_units(stat: &str, value: f64) -> f64 {
    if matches!(stat, MOVE_SPEED | SPRINT_SPEED) {
        value * rulesets::METERS_PER_SOURCE_UNIT
    } else {
        value
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

fn handle<'a>(ctx: &'a Context, entity: &Entity, path: &str) -> Option<&'a Entity> {
    let s = ctx.serializers().get(&entity.class_name)?;
    ctx.entities()
        .get_by_handle(entity.get_handle(s.resolve_field_key(path))?)
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
    for (index, controller) in ctx
        .entities()
        .iter()
        .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
    {
        let slot =
            PlayerSlot(u32::try_from(index - 1).map_err(|_| invalid("invalid controller slot"))?);
        if query
            .players
            .as_ref()
            .is_some_and(|players| !players.contains(&slot))
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
        selected.insert(slot);
        let mut resolver = Resolver {
            ctx,
            catalog,
            controller,
            hero_id,
            slot,
            game_time,
            game_start,
            explain: query.explain,
            contributions: &mut result.contributions,
            ignored_modifiers: BTreeMap::new(),
            inferred_bindings: BTreeSet::new(),
            unmapped_inputs: BTreeSet::new(),
        };
        let inputs = PlayerInputs::collect(ctx, catalog, controller, hero_id, modifiers);
        for &stat in &query.stats {
            resolver.ignored_modifiers.clear();
            resolver.inferred_bindings.clear();
            resolver.unmapped_inputs.clear();
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
                        slot.0,
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
                tick: ctx.tick(),
                player_slot: slot,
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
    if let Some(players) = &query.players {
        for slot in players {
            if !selected.contains(slot) {
                return Err(invalid(format!(
                    "player {} has no selected hero at tick {}",
                    slot.0,
                    ctx.tick()
                )));
            }
        }
    }
    Ok(())
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

struct PlayerInputs<'a> {
    hero: &'a Record,
    level: Option<f64>,
    weapon: &'a Record,
    inventory: Vec<&'a Record>,
    owned: Vec<&'a Record>,
    active: Vec<&'a CModifierTableEntry>,
    permanent: Vec<(u32, f64)>,
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
        let pawn = handle(ctx, controller, "m_hPawn")
            .ok_or_else(|| invalid("player has no current pawn"))?;
        let mut active: Vec<_> = state
            .entries()
            .values()
            .filter(|m| {
                m.parent
                    .and_then(|h| ctx.entities().get_by_handle(h))
                    .is_some_and(|e| std::ptr::eq(e, pawn))
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
            permanent.push((
                id(ctx, controller, &format!("{prefix}.m_SourceModifierID"))?,
                required(ctx, controller, &format!("{prefix}.m_flValue"))?,
            ));
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
    ctx: &'a Context,
    catalog: &'a StatCatalog,
    controller: &'a Entity,
    hero_id: i64,
    slot: PlayerSlot,
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
                tick: self.ctx.tick(),
                player_slot: self.slot,
                hero_id: self.hero_id,
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
        let base = weapon
            .definition
            .pointer("/m_WeaponInfo/m_iClipSize")
            .and_then(number)
            .ok_or_else(|| invalid("weapon has no base clip size"))?;
        self.record(
            "clip_size",
            "base",
            base,
            weapon,
            &format!("{}/m_WeaponInfo/m_iClipSize", weapon.definition_path),
            None,
        );
        let spirit = self.spirit_scaled_bonus(inputs, "EClipSize", "clip_size", "flat")?;
        let flat = spirit + self.total(FLAT, "clip_size", "flat", inputs)?;
        let percent = self.total(PERCENT, "clip_size", "percent", inputs)?;
        rulesets::clip_size::calculate(base, flat, percent)
    }

    fn bullet_velocity(&mut self, inputs: &PlayerInputs<'_>) -> Result<f64> {
        let weapon = inputs.weapon;
        let base = weapon
            .definition
            .pointer("/m_WeaponInfo/m_flBulletSpeed")
            .and_then(number)
            .ok_or_else(|| invalid("weapon has no base bullet speed"))?;
        self.record(
            "bullet_velocity",
            "base",
            base * rulesets::bullet_velocity::METERS_PER_SOURCE_UNIT,
            weapon,
            &format!("{}/m_WeaponInfo/m_flBulletSpeed", weapon.definition_path),
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
        let info = &weapon.definition["m_WeaponInfo"];
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
            &format!("{}/m_WeaponInfo/{field}", weapon.definition_path),
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
                if effect["stat"] != symbol {
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
            if empty_declaration(owner, effect) {
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
        let pawn = handle(self.ctx, self.controller, "m_hPawn")
            .ok_or_else(|| invalid("player has no current pawn"))?;
        let base = required(self.ctx, pawn, "m_flGravityScale")?;
        if self.explain {
            self.contributions.push(Contribution {
                tick: self.ctx.tick(),
                player_slot: self.slot,
                hero_id: self.hero_id,
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
        let info = &weapon.definition["m_WeaponInfo"];
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
            &format!("{}/m_WeaponInfo/m_reloadDuration", weapon.definition_path),
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
                if effect["stat"] != WEAPON_DAMAGE || effect.get("runtime_counts").is_none() {
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
        let pawn = handle(self.ctx, self.controller, "m_hPawn")
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
        let mut total = base
            + self.level_bonus(inputs, SPIRIT, "spirit_power", "flat")?
            + self.purchase_bonus(inputs, SPIRIT, "spirit_power", "flat")?;
        total += self.total(SPIRIT, "spirit_power", "flat", inputs)?;
        // Do not guess the interaction of percentage spirit modifiers with scaling.
        if self.total(
            "MODIFIER_VALUE_TECH_POWER_PERCENT",
            "spirit_power",
            "percent",
            inputs,
        )? != 0.0
        {
            return Err(invalid("percentage spirit-power scaling is not supported"));
        }
        Ok(total)
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

    fn visit_values(
        &mut self,
        stat: &str,
        input: &str,
        kind: &'static str,
        inputs: &PlayerInputs<'_>,
        policy: ValuePolicy,
        mut add: impl FnMut(f64) -> Result<()>,
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
        // Resolve its source ID through boon-data, never through unstable enum ordinals.
        let vector = "m_PlayerDataGlobal.m_vecStatViewerModifierValues";
        for (i, &(source_id, value)) in permanent.iter().enumerate() {
            let prefix = format!("{vector}.{i}");
            if value == 0.0 {
                continue;
            }
            let Some(source) = self.modifier(source_id, None) else {
                continue;
            };
            let stats: HashSet<_> = source
                .stat_changes
                .iter()
                .filter_map(|s| s["stat"].as_str())
                .collect();
            if stats.len() != 1 {
                return Err(invalid(format!(
                    "ambiguous permanent stat source {source_id}"
                )));
            }
            if stats.contains(stat) {
                let value = modifier_units(stat, value);
                add(value)?;
                self.record(
                    input,
                    kind,
                    value,
                    source,
                    &format!("{prefix}.m_flValue"),
                    None,
                );
            }
        }
        // Unbound conditional properties use an explicit or inferred activation link.
        // Bound properties are read below from their live modifier instance, once.
        for item in owned {
            for effect in &item.stat_changes {
                if effect["stat"] != stat
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
                if explicit_only
                    && effect["usage_flags"].as_str().is_none_or(|flags| {
                        !flags.contains("IntrinsicallyProvidedInAbility")
                            && !flags.contains("ConditionallyApplied")
                    })
                {
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
                add(value)?;
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
                if effect["stat"] != stat {
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
                if let Some(counter) = effect.get("runtime_count") {
                    let path = counter
                        .as_str()
                        .ok_or_else(|| invalid("invalid catalog runtime count field"))?;
                    let count = self.runtime_count(source, entry, path)?;
                    self.record(
                        input,
                        "runtime_count",
                        count,
                        source,
                        path,
                        entry.serial_number,
                    );
                    value *= count;
                    if !value.is_finite() {
                        return Err(invalid("nonfinite counter contribution"));
                    }
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
                add(value)?;
                self.record(input, kind, value, source, path, entry.serial_number);
            }
        }
        Ok(())
    }

    fn runtime_count(
        &self,
        source: &Record,
        entry: &CModifierTableEntry,
        path: &str,
    ) -> Result<f64> {
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
        checked_runtime_count(required(self.ctx, ability, path)?, path)
    }

    fn effect(
        &self,
        effect: &Value,
        source: &Record,
        entry: Option<&CModifierTableEntry>,
    ) -> Result<f64> {
        let stat = effect["stat"].as_str().unwrap_or_default();
        let nominal_lifesteal = matches!(
            stat,
            BULLET_LIFESTEAL | SPIRIT_LIFESTEAL | "melee_lifesteal"
        );
        if let Some(scale) = effect["scaling"].get("$value").and_then(Value::as_object)
            && !scale.is_empty()
            // These stats describe the fraction before healing boosts/reduction.
            // Other scaling functions (including spirit scaling) remain checked.
            && !(nominal_lifesteal
                && scale.get("_class").and_then(Value::as_str) == Some("scale_function_single_stat")
                && scale.get("m_eSpecificStatScaleType").and_then(Value::as_str) == Some("EHealingOutput"))
            && !(matches!(
                scale.get("_class").and_then(Value::as_str),
                Some("scale_function_single_stat" | "scale_function_tech_damage")
            ) && scale.get("m_flStatScale").and_then(number) == Some(0.0))
        {
            return Err(invalid(format!(
                "unsupported property scaling at {}",
                effect["definition_path"]
            )));
        }
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
                        && handle(self.ctx, e, "m_hPawn").is_some_and(|p| std::ptr::eq(p, caster))
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
mod tests {
    use super::*;
    use serde_json::json;

    fn stat_result(
        catalog: &StatCatalog,
        active: bool,
        stat: HeroStat,
    ) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
        stat_result_with_permanent(catalog, active, stat, vec![])
    }

    fn stat_result_with_permanent(
        catalog: &StatCatalog,
        active: bool,
        stat: HeroStat,
        permanent: Vec<(u32, f64)>,
    ) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
        let mut inferred = BTreeSet::new();
        let modifier = CModifierTableEntry {
            modifier_subclass: Some(12),
            serial_number: Some(42),
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..Default::default()
        };
        stat_result_with_modifier(
            catalog,
            stat,
            permanent,
            active.then_some(&modifier),
            true,
            &mut inferred,
        )
    }

    fn stat_result_with_modifier(
        catalog: &StatCatalog,
        stat: HeroStat,
        permanent: Vec<(u32, f64)>,
        modifier: Option<&CModifierTableEntry>,
        owned: bool,
        inferred: &mut BTreeSet<String>,
    ) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let mut contributions = Vec::new();
        let mut resolver = Resolver {
            ctx: &ctx,
            catalog,
            controller: &controller,
            hero_id: 999,
            slot: PlayerSlot(0),
            game_time: Some(12.0),
            game_start: Some(0.0),
            explain: true,
            contributions: &mut contributions,
            ignored_modifiers: BTreeMap::new(),
            inferred_bindings: BTreeSet::new(),
            unmapped_inputs: BTreeSet::new(),
        };
        let hero = &catalog.heroes[&999];
        let weapon = catalog.weapon(hero).unwrap();
        let inputs = PlayerInputs {
            hero,
            level: Some(5.0),
            weapon,
            inventory: vec![weapon],
            owned: if owned { vec![weapon] } else { vec![] },
            active: modifier.into_iter().collect(),
            permanent,
        };
        let result = match stat {
            HeroStat::ClipSize => resolver.ammo(&inputs).map(f64::from),
            HeroStat::BulletVelocity => resolver.bullet_velocity(&inputs),
            HeroStat::WeaponDamage => resolver.weapon_damage(&inputs),
            HeroStat::MeleeDistance => resolver.melee_distance(&inputs),
            HeroStat::LightMeleeDamage | HeroStat::HeavyMeleeDamage => {
                resolver.melee_damage(&inputs, stat)
            }
            HeroStat::ReloadTime => resolver.reload_time(&inputs),
            HeroStat::FireRate => resolver.fire_rate(&inputs),
            HeroStat::SlideDistance | HeroStat::BulletEvasion => {
                resolver.movement_percent(&inputs, stat)
            }
            HeroStat::GravityScale => resolver.gravity_scale(),
            HeroStat::Stamina => resolver.stamina(&inputs),
            HeroStat::DebuffResist => resolver.debuff_resist(&inputs),
            HeroStat::BulletResist | HeroStat::SpiritResist | HeroStat::MeleeResist => {
                resolver.resistance(&inputs, stat)
            }
            HeroStat::BulletLifesteal | HeroStat::SpiritLifesteal => {
                resolver.lifesteal(&inputs, stat)
            }
            HeroStat::MeleeLifesteal => resolver.melee_lifesteal(&inputs),
            HeroStat::MoveSpeed | HeroStat::SprintSpeed => resolver.speed(&inputs, stat),
            HeroStat::StaminaCooldown => resolver.stamina_cooldown(&inputs),
            HeroStat::DashSpeed
            | HeroStat::DashDuration
            | HeroStat::AirDashSpeed
            | HeroStat::AirDashDuration => resolver.dash(&inputs, stat),
            HeroStat::FalloffStart | HeroStat::FalloffEnd => resolver.falloff_range(&inputs, stat),
        };
        *inferred = resolver.inferred_bindings;
        inferred.extend(resolver.unmapped_inputs);
        let ignored = resolver.ignored_modifiers;
        (result, contributions, ignored)
    }

    #[test]
    fn resistance_uses_catalog_growth_and_separately_combines_recipient_shred() {
        let folder = super::super::catalog::tests::fixture();
        let modifier_path = folder.path().join("modifiers.json");
        let mut records: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        for (stat, key, symbol, reduction) in [
            (
                HeroStat::BulletResist,
                "EBulletArmorDamageReduction",
                "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST",
                "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION",
            ),
            (
                HeroStat::SpiritResist,
                "ETechArmorDamageReduction",
                "MODIFIER_VALUE_TECH_RESIST",
                "MODIFIER_VALUE_TECH_RESIST_REDUCTION",
            ),
            (
                HeroStat::MeleeResist,
                "EMeleeResist",
                "MODIFIER_VALUE_MELEE_RESIST",
                "MODIFIER_VALUE_MELEE_RESIST_REDUCTION",
            ),
        ] {
            let resist = json!({"stat":symbol,"value":20,"definition_path":"/resist"});
            records["records"][2]["stat_changes"] = json!([
                resist, resist,
                {"stat":reduction,"value":-25,"definition_path":"/shred1"},
                {"stat":reduction,"value":-20,"definition_path":"/shred2"},
                {"stat":"MODIFIER_VALUE_INCOMING_DAMAGE_PERCENTAGE","value":-99,"definition_path":"/damage"},
                {"stat":"MODIFIER_VALUE_BULLET_RESIST_NON_HERO","value":99,"definition_path":"/npc"}
            ]);
            std::fs::write(&modifier_path, serde_json::to_vec(&records).unwrap()).unwrap();
            let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
            hero["m_mapStartingStats"] = json!({key:10,"ETechPower":20});
            hero["m_mapScalingStats"] = json!({key:{"eScalingStat":"ETechPower","flScale":0.25}});
            hero["m_mapStandardLevelUpUpgrades"] = json!({symbol:2});
            hero["m_mapLevelInfo"] = json!({"1":{"m_bUseStandardUpgrade":true},"3":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true},"6":{"m_bUseStandardUpgrade":true}});
            // Owning an offensive ability does not apply its shred to the owner.
            catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
                "stat":reduction,"value":-90,"usage_flags":"IntrinsicallyProvidedInAbility"
            })];
            let (inactive, _, ignored) = stat_result(&catalog, false, stat);
            assert_eq!(inactive.unwrap(), 21.0); // 10 base + 5 spirit + 6 boon.
            assert!(ignored.is_empty());
            let (active, trace, ignored) = stat_result(&catalog, true, stat);
            assert!((active.unwrap() + 3.2).abs() < 1e-12); // (21 + 20 * .79) - 40.
            assert!(ignored.is_empty());
            assert_eq!(
                trace
                    .iter()
                    .filter(|row| row.kind == "percent" && row.input == stat.as_str())
                    .count(),
                1
            );
            assert_eq!(
                trace.iter().filter(|row| row.kind == "reduction").count(),
                2
            );
            // A missing zero base must not suppress explicit hero scaling.
            catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), 11.0);
            let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
            hero["m_mapStartingStats"] = json!({key:-15});
            hero["m_mapScalingStats"] = json!({});
            hero["m_mapStandardLevelUpUpgrades"] = json!({});
            assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), -15.0);
            catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"][key] =
                json!("bad");
            assert!(stat_result(&catalog, false, stat).0.is_err());
        }
    }

    #[test]
    fn melee_component_does_not_include_shared_weapon_resistance_or_shred() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        records["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST","value":40,"definition_path":"/bullet"},
            {"stat":"MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION","value":-25,"definition_path":"/shared_shred"},
            {"stat":"MODIFIER_VALUE_MELEE_RESIST","value":20,"definition_path":"/melee"},
            {"stat":"MODIFIER_VALUE_MELEE_RESIST_REDUCTION","value":-10,"definition_path":"/melee_shred"}
        ]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapStartingStats"] = json!({});
        hero["m_mapScalingStats"] = json!({});
        assert_eq!(
            stat_result(&catalog, true, HeroStat::BulletResist)
                .0
                .unwrap(),
            15.0
        );
        assert_eq!(
            stat_result(&catalog, true, HeroStat::SpiritResist)
                .0
                .unwrap(),
            0.0
        );
        assert_eq!(
            stat_result(&catalog, true, HeroStat::MeleeResist)
                .0
                .unwrap(),
            10.0
        );
    }

    #[test]
    fn resistance_reports_unbound_bonuses_and_rejects_unknown_reduction_signs() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        records["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_TECH_RESIST_REDUCTION","value":20,"definition_path":"/positive"}
        ]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":"MODIFIER_VALUE_TECH_RESIST","value":27,"property_name":"ConditionalArmor","modifier_keys":[]
        })];
        let mut diagnostics = BTreeSet::new();
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::SpiritResist,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("ConditionalArmor") && d.contains("no modifier binding"))
        );
        assert!(
            stat_result(&catalog, true, HeroStat::SpiritResist)
                .0
                .unwrap_err()
                .to_string()
                .contains("positive resistance-reduction")
        );
    }

    #[test]
    fn empty_unbound_resistance_declarations_are_not_effects() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "kind":"ability_property","stat":"MODIFIER_VALUE_TECH_RESIST",
            "property_name":"EmptyResist","value":null,"raw_value":null,
            "modifier_keys":[],"usage_flags":"","scaling":null
        })];
        let mut diagnostics = BTreeSet::new();
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::SpiritResist,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty() && diagnostics.is_empty());
        // An explicit invalid value is not the same as an empty declaration.
        catalog.abilities.get_mut(&123).unwrap().stat_changes[0]["raw_value"] = json!("bad");
        assert!(
            stat_result(&catalog, false, HeroStat::SpiritResist)
                .0
                .is_err()
        );
        catalog.abilities.get_mut(&123).unwrap().stat_changes[0]["raw_value"] = Value::Null;
        // An upgrade turns this into a potential input with a missing base.
        catalog.abilities.get_mut(&123).unwrap().definition["m_vecAbilityUpgrades"] = json!([
            {"m_vecPropertyUpgrades":[{"m_strPropertyName":"EmptyResist","m_strBonus":"10"}]}
        ]);
        assert!(
            stat_result(&catalog, false, HeroStat::SpiritResist)
                .0
                .is_err()
        );
    }

    #[test]
    fn upgraded_resistance_reduction_requires_its_caster() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        records["records"][2]["ability_id"] = json!(123);
        records["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_TECH_RESIST_REDUCTION","value":-10,"property_name":"Shred","definition_path":"/shred"}
        ]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
        catalog.abilities.get_mut(&123).unwrap().definition["m_vecAbilityUpgrades"] = json!([
            {"m_vecPropertyUpgrades":[{"m_strPropertyName":"Shred","m_strBonus":"-5"}]}
        ]);
        assert!(
            stat_result(&catalog, true, HeroStat::SpiritResist)
                .0
                .unwrap_err()
                .to_string()
                .contains("caster upgrades")
        );
    }

    #[test]
    fn lifesteal_combines_catalog_sources_and_keeps_damage_types_separate() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let scaling = json!({"$type":"subclass","$value":{
            "_class":"scale_function_single_stat","m_eSpecificStatScaleType":"EHealingOutput"
        }});
        let bullet = json!({"stat":BULLET_LIFESTEAL,"value":22,"definition_path":"/bullet","scaling":scaling});
        let spirit = json!({"stat":SPIRIT_LIFESTEAL,"value":30,"definition_path":"/spirit","scaling":scaling});
        records["records"][2]["stat_changes"] = json!([bullet, bullet, spirit]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] =
            json!({"EBulletLifesteal":30,"ETechLifesteal":10});
        for (stat, innate, combined) in [
            (HeroStat::BulletLifesteal, 30.0, 45.4),
            (HeroStat::SpiritLifesteal, 10.0, 37.0),
        ] {
            assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), innate);
            let (value, trace, ignored) = stat_result(&catalog, true, stat);
            assert!((value.unwrap() - combined).abs() < 1e-12);
            assert_eq!(trace.len(), 2); // Innate plus one deduplicated active source.
            assert!(trace.iter().all(|row| row.input == stat.as_str()));
            assert!(ignored.is_empty());
        }
        let stats = &mut catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"];
        *stats = json!({});
        assert_eq!(
            stat_result(&catalog, false, HeroStat::BulletLifesteal)
                .0
                .unwrap(),
            0.0
        );
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"]["EBulletLifesteal"] =
            json!("bad");
        assert!(
            stat_result(&catalog, false, HeroStat::BulletLifesteal)
                .0
                .is_err()
        );

        records["records"][2]["stat_changes"][0]["scaling"]["$value"]["m_eSpecificStatScaleType"] =
            json!("ETechPower");
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
        assert!(
            stat_result(&catalog, true, HeroStat::BulletLifesteal)
                .0
                .unwrap_err()
                .to_string()
                .contains("unsupported property scaling")
        );
    }

    #[test]
    fn unbound_property_is_diagnosed_without_assuming_activation() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
        for flags in [
            Value::Null,
            json!(""),
            json!("ConditionallyApplied"),
            json!("IntrinsicallyProvidedInAbility"),
        ] {
            catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
                "stat":BULLET_LIFESTEAL,"value":29,"property_name":"ArbitraryLifesteal",
                "usage_flags":flags,"modifier_keys":[],"definition_path":"/passive"
            })];
            let mut diagnostics = BTreeSet::new();
            let (value, trace, _) = stat_result_with_modifier(
                &catalog,
                HeroStat::BulletLifesteal,
                vec![],
                None,
                true,
                &mut diagnostics,
            );
            let intrinsic = flags == "IntrinsicallyProvidedInAbility";
            assert_eq!(value.unwrap(), if intrinsic { 29.0 } else { 0.0 });
            assert_eq!(trace.len(), usize::from(intrinsic));
            assert_eq!(diagnostics.is_empty(), intrinsic);
            if !intrinsic {
                assert!(
                    diagnostics
                        .iter()
                        .any(|d| d.contains("ArbitraryLifesteal")
                            && d.contains("no modifier binding"))
                );
            }
        }
    }

    #[test]
    fn melee_lifesteal_uses_the_exact_passive_property_and_excludes_healing_procs() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for amount in [18, 31] {
            catalog.abilities.get_mut(&123).unwrap().definition = json!({
                "m_WeaponInfo":{"m_iClipSize":20},
                "m_eAbilityActivation":"CITADEL_ABILITY_ACTIVATION_PASSIVE",
                "m_mapAbilityProperties":{
                    "MeleeLifesteal":{"m_strValue":amount,"m_subclassScaleFunction":{"$value":{
                        "_class":"scale_function_single_stat","m_eSpecificStatScaleType":"EHealingOutput"
                    }}},
                    "LifestealHealPercent":{"m_strValue":95},
                    "LifestealHeal":{"m_strValue":900},
                    "LifestrikeHealPercent":{"m_strValue":80},
                    "NotMeleeLifesteal":{"m_strValue":99}
                }
            });
            let mut diagnostics = BTreeSet::new();
            let (value, trace, ignored) = stat_result_with_modifier(
                &catalog,
                HeroStat::MeleeLifesteal,
                vec![],
                None,
                true,
                &mut diagnostics,
            );
            assert_eq!(value.unwrap(), f64::from(amount));
            assert_eq!(trace.len(), 1);
            assert!(trace[0].definition_path.ends_with("/MeleeLifesteal"));
            assert!(ignored.is_empty());
            assert!(diagnostics.iter().any(|d| d.contains("property rule")));
            let (value, trace, _) = stat_result_with_modifier(
                &catalog,
                HeroStat::MeleeLifesteal,
                vec![],
                None,
                false,
                &mut diagnostics,
            );
            assert_eq!(value.unwrap(), 0.0);
            assert!(trace.is_empty() && diagnostics.is_empty());
        }
        catalog.abilities.get_mut(&123).unwrap().definition["m_eAbilityActivation"] =
            json!("CITADEL_ABILITY_ACTIVATION_INSTANT_CAST");
        let mut diagnostics = BTreeSet::new();
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::MeleeLifesteal,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("no passive activation binding"))
        );
        catalog.abilities.get_mut(&123).unwrap().definition["m_mapAbilityProperties"]["TargetLifesteal"] =
            json!({"m_strValue":75});
        assert!(
            stat_result(&catalog, false, HeroStat::MeleeLifesteal)
                .0
                .unwrap_err()
                .to_string()
                .contains("requires target and damage-type context")
        );
    }

    #[test]
    fn debuff_resist_uses_catalog_innate_bound_and_recorded_values_once() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let symbol = "MODIFIER_VALUE_STATUS_RESISTANCE";
        let effect = json!({"stat":symbol,"value":37,"property_name":"ArbitraryResistance",
            "definition_path":"/test_gun/resistance","modifier_keys":["abilities#/second"]});
        abilities["records"][0]["stat_changes"] = json!([effect]);
        modifiers["records"][2]["stat_changes"] = json!([effect]);
        modifiers["records"][0]["stat_changes"] = json!([{"stat":symbol,"value":1}]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        for (stats, innate) in [(json!({}), 0.0), (json!({"EDebuffResist":-13}), -13.0)] {
            heroes["records"][0]["definition"]["m_mapStartingStats"] = stats;
            std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let (inactive, _, _) = stat_result(&catalog, false, HeroStat::DebuffResist);
            assert_eq!(inactive.unwrap(), innate);
            let (active, trace, _) = stat_result_with_permanent(
                &catalog,
                true,
                HeroStat::DebuffResist,
                vec![(10, 10.0)],
            );
            let expected = 100.0 * (1.0 - (1.0 - innate / 100.0) * 0.9 * 0.63);
            assert!((active.unwrap() - expected).abs() < 1e-12);
            assert_eq!(
                trace
                    .iter()
                    .filter(|r| r.modifier_serial == Some(42))
                    .count(),
                1
            );
            assert!(
                trace
                    .iter()
                    .any(|r| r.value == 10.0 && r.definition_path.ends_with("m_flValue"))
            );
            if innate != 0.0 {
                assert!(trace.iter().any(|r| r.value == innate && r.definition_path.ends_with("/EDebuffResist")));
            }
        }
        heroes["records"][0]["definition"]["m_mapStartingStats"]["EDebuffResist"] =
            json!("invalid");
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::DebuffResist)
                .0
                .unwrap_err()
                .to_string()
                .contains("invalid base")
        );
    }

    #[test]
    fn unbound_debuff_resist_condition_stays_partial() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_STATUS_RESISTANCE","value":20,"property_name":"ConditionalResistance",
             "modifier_keys":[],"usage_flags":"ConditionallyApplied"}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::DebuffResist,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("ConditionalResistance") && d.contains("no modifier binding"))
        );
    }

    #[test]
    fn movement_uses_each_bound_bonus_and_normalizes_units() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] =
            json!({"EMaxMoveSpeed":6.4,"ESprintSpeed":1.6});
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":MOVE_SPEED,"raw_value":"2m","definition_path":"/first"},
            {"stat":MOVE_SPEED,"raw_value":"2m","definition_path":"/first"},
            {"stat":MOVE_SPEED,"value":3.0/rulesets::METERS_PER_SOURCE_UNIT,"definition_path":"/second"},
            {"stat":SPRINT_SPEED,"raw_value":"2m","definition_path":"/sprint_one"},
            {"stat":SPRINT_SPEED,"raw_value":"1.5m","definition_path":"/sprint_two"},
            {"stat":"MODIFIER_VALUE_MOVEMENT_SPEED_SLOW_PERCENT","value":90},
            {"stat":"MODIFIER_VALUE_MOVE_SPEED_LIMIT","value":1},
            {"stat":"MODIFIER_VALUE_SPRINT_ACCELERATION","value":800}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for (stat, base, expected) in [
            (HeroStat::MoveSpeed, 6.4, 10.9),
            (HeroStat::SprintSpeed, 1.6, 5.1),
        ] {
            assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), base);
            let (value, trace, ignored) = stat_result(&catalog, true, stat);
            assert!((value.unwrap() - expected).abs() < 1e-12);
            assert!(ignored.is_empty());
            let flat: Vec<_> = trace.iter().filter(|row| row.kind == "flat").collect();
            assert_eq!(flat.len(), 2);
            assert_eq!(flat[0].modifier_serial, Some(42));
            assert_eq!(flat[0].value, 2.0);
        }
        modifiers["records"][2]["stat_changes"].as_array_mut().unwrap().push(json!({
            "stat":"MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT","value":70,"definition_path":"/percentage"
        }));
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            (stat_result(&catalog, true, HeroStat::MoveSpeed).0.unwrap() - 18.53).abs() < 1e-12
        );
        assert!(
            (stat_result(&catalog, true, HeroStat::SprintSpeed)
                .0
                .unwrap()
                - 5.1)
                .abs()
                < 1e-12
        );
        modifiers["records"][2]["stat_changes"].as_array_mut().unwrap().push(json!({
            "stat":"MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT","value":20,"definition_path":"/another_percentage"
        }));
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, HeroStat::MoveSpeed)
                .0
                .unwrap_err()
                .to_string()
                .contains("combining movement-speed percentages")
        );
    }

    #[test]
    fn movement_and_sprint_spirit_scaling_are_data_driven() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("heroes.json");
        let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] =
            json!({"EMaxMoveSpeed":6,"ESprintSpeed":1,"ETechPower":20});
        heroes["records"][0]["definition"]["m_mapLevelInfo"] = json!({});
        heroes["records"][0]["definition"]["m_mapScalingStats"] = json!({
            "EMaxMoveSpeed":{"eScalingStat":"ETechPower","flScale":0.1},
            "ESprintSpeed":{"eScalingStat":"ETechPower","flScale":0.2}
        });
        std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for (stat, expected, scale) in [
            (HeroStat::MoveSpeed, 8.0, 2.0),
            (HeroStat::SprintSpeed, 5.0, 4.0),
        ] {
            let (value, trace, _) = stat_result(&catalog, false, stat);
            assert_eq!(value.unwrap(), expected);
            let row = trace.iter().find(|r| r.kind == "spirit_flat").unwrap();
            assert_eq!(row.value, scale);
            assert!(row.definition_path.contains("m_mapScalingStats"));
        }
    }

    #[test]
    fn sprint_powerups_and_recorded_totals_use_source_units() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
        modifiers["records"][0]["stat_changes"] = json!([{"stat":SPRINT_SPEED,"value":1}]);
        modifiers["records"][2]["definition"] = json!({"m_flTimeMin":0,"m_flTimeMax":1});
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":SPRINT_SPEED,"value_min":2.0/rulesets::METERS_PER_SOURCE_UNIT,"value_max":7.0/rulesets::METERS_PER_SOURCE_UNIT,"definition_path":"/powerup"}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(456),
            serial_number: Some(42),
            last_applied_time: Some(12.0),
            duration: Some(60.0),
            ..Default::default()
        };
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::SprintSpeed,
            vec![],
            Some(&entry),
            true,
            &mut BTreeSet::new(),
        );
        assert!((value.unwrap() - 4.6).abs() < 1e-12);
        assert!((trace.last().unwrap().value - 3.0).abs() < 1e-12);
        let (value, trace, _) =
            stat_result_with_permanent(&catalog, false, HeroStat::SprintSpeed, vec![(10, 100.0)]);
        assert!((value.unwrap() - 4.14).abs() < 1e-12);
        assert!((trace.last().unwrap().value - 2.54).abs() < 1e-12);
    }

    #[test]
    fn nested_proc_buffs_are_not_required_intrinsic_effects() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EMaxMoveSpeed":6.4});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":MOVE_SPEED,"raw_value":"2m","modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0/m_ProcBuff"]}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::MoveSpeed,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 6.4);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn runtime_property_requires_an_active_modifier_and_its_ability_entity() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":SPRINT_SPEED,"raw_value":"0.27m","runtime_count":"m_iArbitraryCounter"}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (inactive, _, _) = stat_result(&catalog, false, HeroStat::SprintSpeed);
        assert_eq!(inactive.unwrap(), 1.6);
        let (active, _, _) = stat_result(&catalog, true, HeroStat::SprintSpeed);
        assert!(
            active
                .unwrap_err()
                .to_string()
                .contains("counter has no owning ability entity")
        );
    }

    #[test]
    fn unused_passive_speed_declaration_is_ignored_but_conditions_are_reported() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
        modifiers["records"][2]["ability_id"] = json!(123);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        for flags in ["", "ConditionallyApplied"] {
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":SPRINT_SPEED,"raw_value":"1m","modifier_keys":[],
                 "property_name":"OldSprintBonus","usage_flags":flags}
            ]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            for active in [false, true] {
                let entry = CModifierTableEntry {
                    modifier_subclass: Some(12),
                    ..Default::default()
                };
                let mut diagnostics = BTreeSet::new();
                let (value, _, _) = stat_result_with_modifier(
                    &catalog,
                    HeroStat::SprintSpeed,
                    vec![],
                    active.then_some(&entry),
                    true,
                    &mut diagnostics,
                );
                assert_eq!(value.unwrap(), 1.6);
                assert_eq!(diagnostics.is_empty(), flags.is_empty());
            }
        }
    }

    #[test]
    fn unbound_display_alias_warns_without_assuming_a_stacking_rule() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
        abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
            "Declared":{"m_eProvidedPropertyType":SPRINT_SPEED},
            "ArbitraryEarnedValue":{"m_strLocTokenOverride":"Declared","m_strValue":"0.15m"}
        });
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::SprintSpeed,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 1.6);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("ArbitraryEarnedValue") && d.contains("no stat binding"))
        );
        diagnostics.clear();
        stat_result_with_modifier(
            &catalog,
            HeroStat::SprintSpeed,
            vec![],
            None,
            false,
            &mut diagnostics,
        )
        .0
        .unwrap();
        assert!(diagnostics.is_empty());
        assert!(stat_result(&catalog, false, HeroStat::MoveSpeed).0.is_err());
    }

    #[test]
    fn metric_property_and_upgrade_values_require_a_speed_symbol() {
        for stat in [MOVE_SPEED, SPRINT_SPEED] {
            let base = speed_property_number(&json!("2.5m"), stat).unwrap();
            let upgrade = speed_property_number(&json!("-0.75m"), stat).unwrap();
            assert!((modifier_units(stat, base + upgrade) - 1.75).abs() < 1e-12);
            assert_eq!(
                modifier_units(stat, speed_property_number(&json!(100), stat).unwrap()),
                2.54
            );
            for raw in ["2.5 m/s", "1m + 2m", "bad"] {
                assert!(speed_property_number(&json!(raw), stat).is_none());
            }
        }
        assert!(speed_property_number(&json!("2m"), FLAT).is_none());
        assert_eq!(modifier_units(FLAT, 100.0), 100.0);
    }

    #[test]
    fn stamina_and_dashes_use_catalog_values_and_active_bindings() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        // Invented values ensure these rules do not depend on live hero buckets.
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({
            "EStamina":6.25, "EStaminaRegenPerSecond":0.4,
            "EGroundDashDistanceInMeters":12, "EGroundDashDuration":0.8,
            "EAirDashDistanceInMeters":9, "EAirDashDuration":0.3
        });
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_STAMINA","value":2.5,"definition_path":"/capacity"},
            {"stat":"MODIFIER_VALUE_STAMINA","value":2.5,"definition_path":"/capacity"},
            {"stat":"MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE","value":25,"definition_path":"/recovery"},
            {"stat":"MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT","value":-20,"definition_path":"/ground"},
            {"stat":"MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT","value":50,"definition_path":"/air"},
            {"stat":"MODIFIER_VALUE_AIR_CONTROL_PERCENT","value":900,"definition_path":"/control"}
        ]);
        // Unlimited air dashes do not grant unlimited stamina capacity.
        modifiers["records"][2]["definition"]["m_nEnabledStateMask"] =
            json!("MODIFIER_STATE_UNLIMITED_AIR_DASHES");
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for (stat, inactive, active) in [
            (HeroStat::Stamina, 6.25, 8.75),
            (HeroStat::StaminaCooldown, 2.5, 2.0),
            (HeroStat::DashSpeed, 15.0, 12.0),
            (HeroStat::DashDuration, 0.8, 0.8),
            (HeroStat::AirDashSpeed, 30.0, 45.0),
            (HeroStat::AirDashDuration, 0.3, 0.3),
        ] {
            assert!((stat_result(&catalog, false, stat).0.unwrap() - inactive).abs() < 1e-12);
            let (value, trace, ignored) = stat_result(&catalog, true, stat);
            assert!((value.unwrap() - active).abs() < 1e-12);
            assert!(ignored.is_empty());
            let effects: Vec<_> = trace
                .iter()
                .filter(|r| r.modifier_serial.is_some())
                .collect();
            if !matches!(stat, HeroStat::DashDuration | HeroStat::AirDashDuration) {
                assert_eq!(effects.len(), 1);
                assert_eq!(effects[0].modifier_serial, Some(42));
            }
        }
        modifiers["records"][2]["definition"]["m_nEnabledStateMask"] =
            json!("OTHER_STATE | MODIFIER_STATE_STAMINA_REGEN_PAUSED");
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, HeroStat::StaminaCooldown)
                .0
                .unwrap_err()
                .to_string()
                .contains("recovery is paused")
        );
        assert_eq!(
            stat_result(&catalog, false, HeroStat::StaminaCooldown)
                .0
                .unwrap(),
            2.5
        );
        assert_eq!(
            stat_result(&catalog, true, HeroStat::Stamina).0.unwrap(),
            8.75
        );
    }

    #[test]
    fn movement_percentages_do_not_guess_stacking_or_recipient_bonuses() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({
            "EStamina":5, "EStaminaRegenPerSecond":0.5,
            "EGroundDashDistanceInMeters":12, "EGroundDashDuration":0.8,
            "EAirDashDistanceInMeters":9, "EAirDashDuration":0.3
        });
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        for (stat, symbol, base) in [
            (
                HeroStat::StaminaCooldown,
                "MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE",
                2.0,
            ),
            (
                HeroStat::DashSpeed,
                "MODIFIER_VALUE_MOVEMENT_GROUND_DASH_INCREASE_PERCENT",
                15.0,
            ),
            (
                HeroStat::AirDashSpeed,
                "MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT",
                30.0,
            ),
        ] {
            modifiers["records"][2]["stat_changes"] = json!([
                {"stat":symbol,"value":20,"definition_path":"/one"},
                {"stat":symbol,"value":30,"definition_path":"/two"}
            ]);
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            assert!(
                stat_result(&catalog, true, stat)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("combining")
            );
            assert!((stat_result(&catalog, false, stat).0.unwrap() - base).abs() < 1e-12);
            modifiers["records"][2]["stat_changes"] = json!([]);
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":symbol,"value":70,"property_name":"Unbound","usage_flags":"ConditionallyApplied"}
            ]);
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let mut diagnostic = BTreeSet::new();
            let (value, _, _) =
                stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostic);
            assert!((value.unwrap() - base).abs() < 1e-12);
            assert!(diagnostic.iter().any(|d| d.contains("no modifier binding")));
            abilities["records"][0]["stat_changes"] = json!([]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        }
    }

    #[test]
    fn missing_intrinsic_capacity_is_reported_without_inventing_a_modifier() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EStamina":5});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_STAMINA","value":2,"modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0"]}
        ]);
        modifiers["records"][2]["record_key"] =
            json!("abilities#/test_gun/m_AutoIntrinsicModifiers/0");
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_STAMINA","value":2}
        ]);
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostic = BTreeSet::new();
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::Stamina,
            vec![],
            None,
            true,
            &mut diagnostic,
        );
        assert_eq!(value.unwrap(), 5.0);
        assert!(
            diagnostic
                .iter()
                .any(|d| d.contains("no effective intrinsic modifier"))
        );
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(456),
            serial_number: Some(42),
            ..Default::default()
        };
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::Stamina,
            vec![],
            Some(&entry),
            true,
            &mut diagnostic,
        );
        assert_eq!(value.unwrap(), 7.0);
        assert!(diagnostic.is_empty());
    }

    #[test]
    fn stamina_uses_time_ranged_powerup_and_requires_base_data() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut heroes: Value =
            serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EStamina":5});
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_STAMINA","value_min":2,"value_max":7,"definition_path":"/powerup"}
        ]);
        modifiers["records"][2]["definition"] = json!({"m_flTimeMin":0,"m_flTimeMax":1});
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(456),
            serial_number: Some(42),
            last_applied_time: Some(12.0),
            duration: Some(60.0),
            ..Default::default()
        };
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::Stamina,
            vec![],
            Some(&entry),
            true,
            &mut BTreeSet::new(),
        );
        assert_eq!(value.unwrap(), 8.0);
        assert_eq!(trace.last().unwrap().value, 3.0);
        for stat in [
            HeroStat::StaminaCooldown,
            HeroStat::DashSpeed,
            HeroStat::AirDashDuration,
        ] {
            assert!(
                stat_result(&catalog, false, stat)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("hero has no valid base")
            );
        }
    }

    #[test]
    fn movement_stats_use_bound_modifiers_without_inheriting_owner_bonuses() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        for (stat, symbol, value) in [
            (
                HeroStat::SlideDistance,
                "MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE",
                37.5,
            ),
            (
                HeroStat::BulletEvasion,
                "MODIFIER_VALUE_BULLET_EVASION",
                32.5,
            ),
        ] {
            let bound = json!({"stat":symbol,"value":value,"definition_path":"/test_gun/bound"});
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":symbol,"value":value,"definition_path":"/test_gun/bound","modifier_keys":["abilities#/second"]},
                {"stat":"MODIFIER_VALUE_MOVEMENT_SLIDE_TURN_SCALE","value":900}
            ]);
            modifiers["records"][2]["stat_changes"] = json!([bound, bound]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), 0.0);
            let (active, trace, ignored) = stat_result(&catalog, true, stat);
            assert_eq!(active.unwrap(), value);
            assert_eq!(trace.len(), 1);
            assert_eq!(trace[0].modifier_serial, Some(42));
            assert!(ignored.is_empty());

            // The same record now marks someone else. The owner's conditional
            // property has no recipient binding and must not transfer to them.
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":symbol,"property_name":"ConditionalBonus","value":99,"usage_flags":"ConditionallyApplied"}
            ]);
            modifiers["records"][2]["ability_id"] = json!(123);
            modifiers["records"][2]["definition_path"] = json!("/test_gun/m_TargetModifier");
            modifiers["records"][2]["stat_changes"] = json!([]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let entry = CModifierTableEntry {
                modifier_subclass: Some(12),
                ability_subclass: Some(123),
                serial_number: Some(42),
                ..Default::default()
            };
            let mut diagnostics = BTreeSet::new();
            let (result, trace, _) = stat_result_with_modifier(
                &catalog,
                stat,
                vec![],
                Some(&entry),
                false,
                &mut diagnostics,
            );
            assert_eq!(result.unwrap(), 0.0);
            assert!(trace.is_empty());
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.contains("no binding to recipient"))
            );
        }
    }

    #[test]
    fn slide_multiplies_distinct_bonuses_but_evasion_does_not_guess_stacking() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut modifiers: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (stat, symbol) in [
            (
                HeroStat::SlideDistance,
                "MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE",
            ),
            (HeroStat::BulletEvasion, "MODIFIER_VALUE_BULLET_EVASION"),
        ] {
            modifiers["records"][2]["stat_changes"] = json!([
                {"stat":symbol,"value":35,"definition_path":"/first"},
                {"stat":symbol,"value":50,"definition_path":"/second"}
            ]);
            std::fs::write(&path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let result = stat_result(&catalog, true, stat).0;
            if stat == HeroStat::SlideDistance {
                assert!((result.unwrap() - 102.5).abs() < 1e-12);
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("combining bullet-evasion")
                );
            }
        }
    }

    #[test]
    fn property_names_do_not_establish_stat_bindings_or_diagnostics() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("abilities.json");
        let mut abilities: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
            "AnyEvasionChance":{"m_strValue":"42"}
        });
        std::fs::write(path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let (result, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletEvasion,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(result.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn exact_evasion_property_uses_unique_effect_and_preserves_explicit_bindings() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
            "EvasionPercent": {"m_strValue": "27.5"},
            "ExtraEvasionPercent": {"m_strValue": "99"}
        });
        modifiers["records"][1]["definition_path"] = json!("/test_gun/m_AutoCastDelayModifier");
        modifiers["records"][2]["definition_path"] = json!("/test_gun/m_WhateverEffect");
        modifiers["records"][2]["ability_id"] = json!(123);
        let save = |abilities: &Value, modifiers: &Value| {
            std::fs::write(&ability_path, serde_json::to_vec(abilities).unwrap()).unwrap();
            std::fs::write(&modifier_path, serde_json::to_vec(modifiers).unwrap()).unwrap();
            StatCatalog::from_directory(folder.path()).unwrap()
        };
        let catalog = save(&abilities, &modifiers);
        let mut diagnostics = BTreeSet::new();
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(123),
            serial_number: Some(42),
            duration: Some(-1.0),
            ..Default::default()
        };
        for active in [None, Some(&entry)] {
            let (result, trace, _) = stat_result_with_modifier(
                &catalog,
                HeroStat::BulletEvasion,
                vec![],
                active,
                true,
                &mut diagnostics,
            );
            assert_eq!(result.unwrap(), if active.is_some() { 27.5 } else { 0.0 });
            assert_eq!(trace.len(), usize::from(active.is_some()));
            assert_eq!(diagnostics.len(), usize::from(active.is_some()));
        }
        let windup = CModifierTableEntry {
            modifier_subclass: Some(11),
            ..entry.clone()
        };
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletEvasion,
            vec![],
            Some(&windup),
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty() && diagnostics.is_empty());

        // A second non-intrinsic, non-cast candidate makes activation ambiguous.
        modifiers["records"][1]["definition_path"] = json!("/test_gun/m_OtherEffect");
        let catalog = save(&abilities, &modifiers);
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletEvasion,
            vec![],
            Some(&entry),
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("no modifier binding"))
        );

        // Intrinsic modifiers do not activate the fallback or create ambiguity.
        modifiers["records"][1]["definition_path"] = json!("/test_gun/m_AutoIntrinsicModifiers/0");
        let catalog = save(&abilities, &modifiers);
        assert_eq!(
            stat_result(&catalog, true, HeroStat::BulletEvasion)
                .0
                .unwrap(),
            27.5
        );

        // An explicit binding wins even if the raw property has a different value.
        modifiers["records"][2]["stat_changes"] = json!([{
            "stat":"MODIFIER_VALUE_BULLET_EVASION", "property_name":"EvasionPercent",
            "value":12.5, "definition_path":"/explicit"
        }]);
        let catalog = save(&abilities, &modifiers);
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletEvasion,
            vec![],
            Some(&entry),
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 12.5);
        assert_eq!(trace.len(), 1);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn melee_uses_catalog_boons_investment_modifiers_and_heavy_spirit_scaling() {
        let folder = super::super::catalog::tests::fixture();
        let hero_path = folder.path().join("heroes.json");
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let read = |path: &std::path::Path| -> Value {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        };
        let write = |path: &std::path::Path, value: &Value| {
            std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
        };
        let mut heroes = read(&hero_path);
        let hero = &mut heroes["records"][0]["definition"];
        hero["m_mapStartingStats"] =
            json!({"ELightMeleeDamage":40,"EHeavyMeleeDamage":100,"ETechPower":2});
        hero["m_mapLevelInfo"] = json!({
            "1":{},"2":{"m_bUseStandardUpgrade":true},"3":{},
            "4":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true},
            "6":{"m_bUseStandardUpgrade":true}
        });
        hero["m_mapStandardLevelUpUpgrades"] = json!({
            "MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL":2,"MODIFIER_VALUE_TECH_POWER":1
        });
        hero["m_mapPurchaseBonuses"] = json!({"EItemSlotType_WeaponMod":[
            {"m_nTier":2,"m_ValueType":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","m_strValue":"8"},
            {"m_nTier":2,"m_ValueType":"MODIFIER_VALUE_TECH_POWER","m_strValue":"7"},
            {"m_nTier":3,"m_ValueType":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","m_strValue":"99"}
        ]});
        hero["m_mapScalingStats"] =
            json!({"EHeavyMeleeDamage":{"eScalingStat":"ETechPower","flScale":0.6}});
        write(&hero_path, &heroes);
        let mut abilities = read(&ability_path);
        abilities["records"][0]["definition"]["m_eItemSlotType"] = json!("EItemSlotType_WeaponMod");
        abilities["records"][0]["definition"]["m_iItemTier"] = json!("EModTier_2");
        let melee = "MODIFIER_VALUE_MELEE_DAMAGE_INCREASE";
        let bound = json!({"stat":melee,"value":20,"definition_path":"/test_gun/buff"});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":12,"definition_path":"/weapon","modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_TECH_POWER","value":4},
            {"stat":melee,"value":10,"definition_path":"/melee","modifier_keys":["abilities#/second"]},
            {"stat":melee,"value":20,"definition_path":"/test_gun/buff","modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_TO_NPC_INCREASE","value":200}
        ]);
        write(&ability_path, &abilities);
        let mut modifiers = read(&modifier_path);
        modifiers["records"][0]["stat_changes"] = json!([
            {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":1}
        ]);
        modifiers["records"][2]["stat_changes"] = json!([
            bound, bound,
            {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":12,"definition_path":"/weapon"},
            {"stat":melee,"value":10,"definition_path":"/melee"}
        ]);
        write(&modifier_path, &modifiers);
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for (stat, base) in [
            (HeroStat::LightMeleeDamage, 46.0),
            (HeroStat::HeavyMeleeDamage, 124.6),
        ] {
            for active in [false, true] {
                let (value, trace, ignored) =
                    stat_result_with_permanent(&catalog, active, stat, vec![(10, 4.0)]);
                let factor = if active { 1.42 } else { 1.06 };
                assert!((value.unwrap() - base * factor).abs() < 1e-10);
                assert!(ignored.is_empty());
                assert_eq!(
                    trace
                        .iter()
                        .filter(|c| c.modifier_serial == Some(42))
                        .count(),
                    3 * usize::from(active)
                );
                assert_eq!(
                    trace.iter().find(|c| c.kind == "boon_flat").unwrap().value,
                    6.0
                );
                assert_eq!(
                    trace
                        .iter()
                        .filter(|c| c.input == stat.as_str() && c.kind == "weapon_percent")
                        .map(|c| c.value)
                        .sum::<f64>(),
                    if active { 24.0 } else { 12.0 }
                );
                if stat == HeroStat::HeavyMeleeDamage {
                    assert_eq!(
                        trace
                            .iter()
                            .find(|c| c.input == stat.as_str() && c.kind == "flat")
                            .unwrap()
                            .value,
                        9.6
                    );
                } else {
                    assert!(!trace.iter().any(|c| c.input == "spirit_power"));
                }
            }
        }
        // Changing the catalog coefficient changes the result for an arbitrary hero ID.
        heroes["records"][0]["definition"]["m_mapScalingStats"]["EHeavyMeleeDamage"]["flScale"] =
            json!(1.2);
        write(&hero_path, &heroes);
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let value = stat_result_with_permanent(
            &catalog,
            false,
            HeroStat::HeavyMeleeDamage,
            vec![(10, 4.0)],
        )
        .0
        .unwrap();
        assert!((value - 142.252).abs() < 1e-10);

        for unsupported in [
            "MODIFIER_VALUE_MELEE_DAMAGE_MULTIPLIER",
            "MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER",
        ] {
            modifiers["records"][2]["stat_changes"] = json!([{ "stat":unsupported,"value":10}]);
            write(&modifier_path, &modifiers);
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            assert!(
                stat_result(&catalog, true, HeroStat::LightMeleeDamage)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains(unsupported)
            );
        }
        abilities["records"][0]["stat_changes"] = json!([]);
        abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
            "BaseAttackDamagePercent":{"m_strValue":"18","m_eStatsUsageFlags":"ConditionallyApplied"}
        });
        write(&ability_path, &abilities);
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::LightMeleeDamage,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert!((value.unwrap() - 47.84).abs() < 1e-10);
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("BaseAttackDamagePercent")
                    && message.contains("no stat mapping"))
        );
        abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({});
        write(&ability_path, &abilities);
        heroes["records"][0]["definition"]["m_mapStartingStats"]
            .as_object_mut()
            .unwrap()
            .remove("ELightMeleeDamage");
        write(&hero_path, &heroes);
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::HeavyMeleeDamage)
                .0
                .unwrap_err()
                .to_string()
                .contains("no base light melee")
        );
    }

    #[test]
    fn melee_ignores_unbound_target_damage_and_reports_missing_global_bonuses() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapStartingStats"] = json!({"ELightMeleeDamage":40,"EHeavyMeleeDamage":100});
        hero["m_mapLevelInfo"] = json!({});
        let owner = catalog.abilities.get_mut(&123).unwrap();
        owner.stat_changes = vec![
            json!({"stat":WEAPON_DAMAGE,"value":25,"property_name":"EnemyBonus","usage_flags":"ConditionallyApplied"}),
            json!({"stat":"MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER","value":15,"property_name":"TargetAmp","usage_flags":"ConditionallyApplied"}),
        ];
        owner.definition["m_mapAbilityProperties"] = json!({
            "EnemyBonus":{"m_strConditionalLocTokenOverride":"#EnemyAboveHealthThreshold_conditional"}
        });
        for (stat, base) in [
            (HeroStat::LightMeleeDamage, 40.0),
            (HeroStat::HeavyMeleeDamage, 100.0),
        ] {
            let mut diagnostics = BTreeSet::new();
            let (value, _, _) =
                stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostics);
            assert_eq!(value.unwrap(), base);
            assert!(diagnostics.is_empty());
        }
        catalog.abilities.get_mut(&123).unwrap().stat_changes.push(json!({
            "stat":WEAPON_DAMAGE,"value":18,"property_name":"UnknownGlobalBonus","usage_flags":"ConditionallyApplied"
        }));
        for stat in [HeroStat::LightMeleeDamage, HeroStat::HeavyMeleeDamage] {
            let mut diagnostics = BTreeSet::new();
            assert!(
                stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostics)
                    .0
                    .is_ok()
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|message| message.contains("UnknownGlobalBonus"))
            );
        }
    }

    #[test]
    fn falloff_endpoints_use_catalog_values_and_only_active_range_bonuses() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let stat = "MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT";
        let endpoints = [HeroStat::FalloffStart, HeroStat::FalloffEnd];
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, endpoints[0])
                .0
                .unwrap_err()
                .to_string()
                .contains("no base falloff")
        );
        for (start, end, bonus) in [(1000.0, 2500.0, 20.0), (600.0, 900.0, 37.5)] {
            abilities["records"][0]["definition"]["m_WeaponInfo"] = json!({
                "m_flDamageFalloffStartRange":start, "m_flDamageFalloffEndRange":end,
                "m_flRange":500, "m_flDamageFalloffBias":0.3,
                "m_flDamageFalloffStartScale":1, "m_flDamageFalloffEndScale":0.1
            });
            let effect = json!({"stat":stat,"value":bonus,"definition_path":"/test_gun/range"});
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":stat,"value":bonus,"definition_path":"/test_gun/range","modifier_keys":["abilities#/second"]},
                {"stat":"MODIFIER_VALUE_BONUS_BULLET_DAMAGE_LONG_RANGE_MIN_RANGE","value":15},
                {"stat":"MODIFIER_VALUE_TECH_RANGE","value":50}
            ]);
            modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            for (endpoint, base) in endpoints.into_iter().zip([start, end]) {
                let expected = base * rulesets::METERS_PER_SOURCE_UNIT;
                assert!(
                    (stat_result(&catalog, false, endpoint).0.unwrap() - expected).abs() < 1e-10
                );
                let (value, trace, ignored) = stat_result(&catalog, true, endpoint);
                assert!((value.unwrap() - expected * (1.0 + bonus / 100.0)).abs() < 1e-10);
                assert!(ignored.is_empty());
                assert_eq!(trace.len(), 2); // No duplicate count or unrelated range effects.
                assert_eq!(trace[0].kind, "base");
                assert_eq!(trace[0].value, expected);
                assert_eq!(trace[1].modifier_serial, Some(42));
                assert_eq!(trace[1].value, bonus);
                assert!(trace.iter().all(|c| c.input == endpoint.as_str()));
            }
        }
        abilities["records"][0]["stat_changes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"stat":stat,"value":8}));
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for endpoint in endpoints {
            assert!(stat_result(&catalog, false, endpoint).0.is_ok());
            assert!(
                stat_result(&catalog, true, endpoint)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("combining falloff-range bonuses")
            );
        }
        abilities["records"][0]["definition"]["m_WeaponInfo"]["m_flDamageFalloffStartRange"] =
            json!(-1);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for endpoint in endpoints {
            assert!(
                stat_result(&catalog, false, endpoint)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("unsupported falloff-range inputs")
            );
        }
    }

    #[test]
    fn velocity_uses_catalog_values_and_counts_bound_effects_only_when_active() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let stat = "MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT";
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":stat, "value":10, "definition_path":"/test_gun/passive"},
            {"stat":stat, "value":30, "definition_path":"/test_gun/buff", "modifier_keys":["abilities#/second"], "usage_flags":"ConditionallyApplied"}
        ]);
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":stat, "value":30, "definition_path":"/test_gun/buff"}
        ]);
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        for base in [5000.0, 12000.0] {
            abilities["records"][0]["definition"]["m_WeaponInfo"]["m_flBulletSpeed"] = json!(base);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let (inactive, _, _) = stat_result(&catalog, false, HeroStat::BulletVelocity);
            let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::BulletVelocity);
            assert!((inactive.unwrap() - base * 0.0254 * 1.1).abs() < 1e-10);
            assert!((active.unwrap() - base * 0.0254 * 1.4).abs() < 1e-10);
            assert_eq!(trace.iter().filter(|c| c.kind == "percent").count(), 2);
            assert!(ignored.is_empty());
            assert_eq!(
                trace
                    .iter()
                    .find(|c| c.modifier_serial == Some(42))
                    .unwrap()
                    .value,
                30.0
            );
        }

        // Ownership alone cannot establish an unbound conditional bonus.
        abilities["records"][0]["stat_changes"][0]["usage_flags"] = json!("ConditionallyApplied");
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, HeroStat::BulletVelocity)
                .0
                .unwrap_err()
                .to_string()
                .contains("conditional")
        );

        abilities["records"][0]["stat_changes"] = json!([]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        modifiers["records"][2]["stat_changes"][0]["stat"] =
            json!("MODIFIER_VALUE_BASE_BULLET_SPEED_OVERRIDE");
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, HeroStat::BulletVelocity)
                .0
                .unwrap_err()
                .to_string()
                .contains("overrides")
        );
    }

    #[test]
    fn conditional_properties_use_the_unique_owner_modifier_with_diagnostics() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let effect = json!({
            "stat":"MODIFIER_VALUE_FIRE_RATE", "property_name":"AnyBonus", "value":27.5,
            "definition_path":"/test_gun/m_mapAbilityProperties/AnyBonus",
            "usage_flags":"ConditionallyApplied", "modifier_keys":[]
        });
        abilities["records"][0]["stat_changes"] = json!([effect]);
        modifiers["records"][1]["ability_id"] = json!(456);
        modifiers["records"][2]["ability_id"] = json!(123);
        modifiers["records"][2]["definition_path"] = json!("/test_gun/m_BuffModifier");
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(123),
            serial_number: Some(42),
            last_applied_time: Some(10.0),
            duration: Some(5.0),
            ..Default::default()
        };
        let mut inferred = BTreeSet::new();
        for bonus in [27.5, 83.0] {
            abilities["records"][0]["stat_changes"][0]["value"] = json!(bonus);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let (value, trace, ignored) = stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                None,
                true,
                &mut inferred,
            );
            assert_eq!(value.unwrap(), 0.0);
            assert!(trace.is_empty() && ignored.is_empty());
            assert_eq!(inferred.len(), 1);
            // Buff recipients need not own the source ability themselves.
            for owned in [false, true] {
                let (value, trace, ignored) = stat_result_with_modifier(
                    &catalog,
                    HeroStat::FireRate,
                    vec![],
                    Some(&entry),
                    owned,
                    &mut inferred,
                );
                assert_eq!(value.unwrap(), bonus);
                assert!(ignored.is_empty());
                assert_eq!(inferred.len(), 1);
                assert!(
                    inferred
                        .first()
                        .unwrap()
                        .contains("AnyBonus in abilities#/test_gun -> abilities#/second")
                );
                assert_eq!(trace.len(), 1);
                assert_eq!(trace[0].modifier_serial, Some(42));
            }
        }
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for duration in [None, Some(-1.0), Some(0.0), Some(f32::INFINITY)] {
            let untimed = CModifierTableEntry {
                duration,
                ..entry.clone()
            };
            let error = stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                Some(&untimed),
                true,
                &mut inferred,
            )
            .0
            .unwrap_err();
            assert!(error.to_string().contains("untimed modifier"));
        }
        let missing_time = CModifierTableEntry {
            last_applied_time: None,
            ..entry.clone()
        };
        assert!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                Some(&missing_time),
                true,
                &mut inferred
            )
            .0
            .unwrap_err()
            .to_string()
            .contains("expiry")
        );
        let stacked = CModifierTableEntry {
            stack_count: Some(2),
            ..entry.clone()
        };
        assert!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                Some(&stacked),
                true,
                &mut inferred
            )
            .0
            .unwrap_err()
            .to_string()
            .contains("stacking")
        );

        // A permanent intrinsic modifier is not an activation signal.
        modifiers["records"][2]["definition_path"] = json!("/test_gun/m_AutoIntrinsicModifiers/0");
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for active in [false, true] {
            assert!(
                stat_result(&catalog, active, HeroStat::FireRate)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("no activation binding")
            );
        }
        // Even one active instance cannot disambiguate multiple catalog candidates.
        modifiers["records"][2]["definition_path"] = json!("/test_gun/m_BuffModifier");
        modifiers["records"][1]["ability_id"] = json!(123);
        modifiers["records"][1]["definition_path"] = json!("/test_gun/m_OtherModifier");
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for active in [false, true] {
            assert!(
                stat_result(&catalog, active, HeroStat::FireRate)
                    .0
                    .unwrap_err()
                    .to_string()
                    .contains("no activation binding")
            );
        }

        assert!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                Some(&entry),
                false,
                &mut inferred
            )
            .0
            .unwrap_err()
            .to_string()
            .contains("no unique conditional modifier")
        );

        // Explicit catalog bindings take priority and do not need inference.
        abilities["records"][0]["stat_changes"][0]["modifier_keys"] = json!(["abilities#/second"]);
        modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
            0.0
        );
        let (value, trace, ignored) = stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            Some(&entry),
            true,
            &mut inferred,
        );
        assert_eq!(value.unwrap(), 27.5);
        assert!(ignored.is_empty() && inferred.is_empty());
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].modifier_serial, Some(42));
    }

    #[test]
    fn weapon_damage_uses_catalog_purchases_boons_and_recorded_totals_once() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let bound = json!({"stat":WEAPON_DAMAGE,"value":12.5,"definition_path":"/bound"});
        records["records"][2]["stat_changes"] = json!([
            bound, bound,
            {"stat":"MODIFIER_VALUE_FLAT_BULLET_DAMAGE_POST_SCALE","value":90,"definition_path":"/flat"},
            {"stat":"MODIFIER_VALUE_CLOSE_RANGE_WEAPON_DAMAGE_INCREASE","value":80,"definition_path":"/close"},
            {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_TO_NPC_INCREASE","value":70,"definition_path":"/npc"}
        ]);
        records["records"][0]["stat_changes"] = json!([{"stat":WEAPON_DAMAGE,"value":3}]);
        records["records"][0]["misc_id"] = json!(90);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapPurchaseBonuses"] = json!({"arbitrary_slot":[
            {"m_ValueType":WEAPON_DAMAGE,"m_nTier":2,"m_strValue":"7.25"}
        ]});
        hero["m_mapLevelInfo"] =
            json!({"1":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true}});
        hero["m_mapStandardLevelUpUpgrades"] = json!({WEAPON_DAMAGE:1.5});
        // Base bullet damage and its spirit scaling are not percentage bonuses.
        hero["m_mapStartingStats"] =
            json!({"EBaseWeaponDamage":999,"EWeaponPower":0,"EWeaponPowerScale":1});
        hero["m_mapScalingStats"] =
            json!({"EBulletDamage":{"eScalingStat":"ETechPower","flScale":999}});
        let weapon = catalog.abilities.get_mut(&123).unwrap();
        weapon.definition["m_eItemSlotType"] = json!("arbitrary_slot");
        weapon.definition["m_iItemTier"] = json!("EModTier_2");
        weapon.stat_changes =
            vec![json!({"stat":WEAPON_DAMAGE,"value":12.5,"modifier_keys":["abilities#/second"]})];
        let (value, trace, ignored) =
            stat_result_with_permanent(&catalog, true, HeroStat::WeaponDamage, vec![(10, 17.0)]);
        assert_eq!(value.unwrap(), 39.75); // 7.25 purchase + 3 boon + 12.5 bound + 17 recorded.
        assert!(ignored.is_empty());
        assert_eq!(trace.len(), 4);
        assert!(
            trace
                .iter()
                .any(|row| row.value == 17.0 && row.definition_path.ends_with(".m_flValue"))
        );
        // A live permanent pickup is already included in its accumulated total.
        let pickup: Record = serde_json::from_value(json!({
            "record_key":"misc#/permanent","definition_path":"/permanent","misc_id":90,
            "definition":{"m_bIsPermanentPickup":true},"stat_changes":[]
        }))
        .unwrap();
        catalog.misc.insert(90, pickup);
        let entry = CModifierTableEntry {
            modifier_subclass: Some(10),
            serial_number: Some(99),
            ..Default::default()
        };
        let (value, _, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![(10, 17.0)],
            Some(&entry),
            true,
            &mut BTreeSet::new(),
        );
        assert_eq!(value.unwrap(), 27.25); // No extra catalog pickup amount.
    }

    #[test]
    fn weapon_damage_does_not_apply_unbound_rewards_from_ownership() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for flags in ["", "ConditionallyApplied", "IntrinsicallyProvidedInAbility"] {
            catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
                "stat":WEAPON_DAMAGE,"property_name":"ArbitraryReward","value":6,
                "usage_flags":flags,"modifier_keys":[],"definition_path":"/reward",
                "scaling":{"unsupported_count":true}
            })];
            let mut diagnostics = BTreeSet::new();
            let (value, trace, _) = stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                None,
                true,
                &mut diagnostics,
            );
            assert_eq!(value.unwrap(), 0.0);
            assert!(trace.is_empty());
            assert!(
                diagnostics
                    .iter()
                    .any(|message| message.contains("ArbitraryReward")
                        && message.contains("no modifier binding"))
            );
        }
        let owner = catalog.abilities.get_mut(&123).unwrap();
        owner.stat_changes.clear();
        owner.definition["m_mapAbilityProperties"] =
            json!({"BaseAttackDamagePercent":{"m_strValue":"18"}});
        let mut diagnostics = BTreeSet::new();
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                None,
                true,
                &mut diagnostics
            )
            .0
            .unwrap(),
            0.0
        );
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("BaseAttackDamagePercent")
                    && message.contains("no stat mapping"))
        );
    }

    #[test]
    fn weapon_damage_ignores_empty_declarations_but_reports_missing_intrinsics() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for empty in [Value::Null, json!("")] {
            catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
                "stat":WEAPON_DAMAGE,"kind":"ability_property","property_name":"Empty",
                "value":empty,"raw_value":empty,"usage_flags":"","modifier_keys":[]
            })];
            let mut diagnostics = BTreeSet::new();
            assert_eq!(
                stat_result_with_modifier(
                    &catalog,
                    HeroStat::WeaponDamage,
                    vec![],
                    None,
                    true,
                    &mut diagnostics
                )
                .0
                .unwrap(),
                0.0
            );
            assert!(diagnostics.is_empty());
        }
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"value":22,"property_name":"Bonus",
            "modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0"]
        })];
        let mut diagnostics = BTreeSet::new();
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                None,
                true,
                &mut diagnostics
            )
            .0
            .unwrap(),
            0.0
        );
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("no effective intrinsic modifier"))
        );
    }

    #[test]
    fn weapon_damage_excludes_explicit_enemy_conditions_without_item_names() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let owner = catalog.abilities.get_mut(&123).unwrap();
        owner.stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"value":25,"property_name":"ArbitraryEnemyBonus",
            "usage_flags":"ConditionallyApplied","modifier_keys":[]
        })];
        owner.definition["m_mapAbilityProperties"] = json!({
            "ArbitraryEnemyBonus":{"m_strConditionalLocTokenOverride":"#EnemyAboveHealthThreshold_conditional"}
        });
        let mut diagnostics = BTreeSet::new();
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                None,
                true,
                &mut diagnostics
            )
            .0
            .unwrap(),
            0.0
        );
        assert!(diagnostics.is_empty());
        catalog.abilities.get_mut(&123).unwrap().definition["m_mapAbilityProperties"]["ArbitraryEnemyBonus"] =
            json!({});
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            None,
            true,
            &mut diagnostics,
        )
        .0
        .unwrap();
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn foreign_marker_does_not_transfer_owner_reward_or_its_diagnostic() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        records["records"][2]["ability_id"] = json!(123);
        // A real recipient effect must still apply even if its source ability
        // has an unrelated unbound reward for its owner.
        records["records"][2]["stat_changes"] = json!([{"stat":WEAPON_DAMAGE,"value":11}]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"value":3,"property_name":"OwnerReward",
            "usage_flags":"ConditionallyApplied","modifier_keys":[]
        })];
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            serial_number: Some(42),
            ..Default::default()
        };
        let mut diagnostics = BTreeSet::new();
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                Some(&entry),
                false,
                &mut diagnostics
            )
            .0
            .unwrap(),
            11.0
        );
        assert!(diagnostics.is_empty());
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            Some(&entry),
            true,
            &mut diagnostics,
        )
        .0
        .unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("OwnerReward"))
        );
    }

    #[test]
    fn owned_counter_binding_requires_replay_state_and_valid_counts() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"value":7.5,"property_name":"Reward",
            "runtime_counts":[{"field":"m_ArbitraryEarnedCount"}],"modifier_keys":[]
        })];
        assert!(
            stat_result(&catalog, false, HeroStat::WeaponDamage)
                .0
                .unwrap_err()
                .to_string()
                .contains("owning pawn")
        );
        for invalid in [-1.0, 0.5, f64::NAN, f64::INFINITY] {
            assert!(checked_runtime_count(invalid, "count").is_err());
        }
        assert_eq!(checked_runtime_count(0.0, "count").unwrap(), 0.0);
        assert_eq!(checked_runtime_count(12.0, "count").unwrap(), 12.0);
    }

    #[test]
    fn weapon_damage_does_not_guess_legacy_power_or_modifier_stacks() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("modifiers.json");
        let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        records["records"][2]["stat_changes"] = json!([
            {"stat":WEAPON_DAMAGE,"value":10,"definition_path":"/bonus"},
            {"stat":"MODIFIER_VALUE_WEAPON_POWER","value":7,"definition_path":"/legacy"}
        ]);
        std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostics = BTreeSet::new();
        let mut entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            serial_number: Some(42),
            ..Default::default()
        };
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                Some(&entry),
                true,
                &mut diagnostics
            )
            .0
            .unwrap(),
            10.0
        );
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("weapon power"))
        );
        entry.stack_count = Some(2);
        assert!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                Some(&entry),
                true,
                &mut diagnostics
            )
            .0
            .unwrap_err()
            .to_string()
            .contains("stacking rule")
        );
    }

    #[test]
    fn fire_rate_preserves_individual_slows_and_pickup_totals() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let bonus = "MODIFIER_VALUE_FIRE_RATE";
        let slow = "MODIFIER_VALUE_FIRE_RATE_SLOW";
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":bonus, "value":20, "definition_path":"/passive"},
            {"stat":bonus, "value":10, "definition_path":"/bound", "modifier_keys":["abilities#/second"]}
        ]);
        let bound = json!({"stat":bonus, "value":10, "definition_path":"/bound"});
        modifiers["records"][2]["stat_changes"] = json!([
            bound, bound,
            {"stat":slow, "value":20, "definition_path":"/slow_a"},
            {"stat":slow, "value":30, "definition_path":"/slow_b"}
        ]);
        modifiers["records"][0]["stat_changes"] = json!([{"stat":bonus,"value":1.5}]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
            20.0
        );
        let (value, trace, ignored) =
            stat_result_with_permanent(&catalog, true, HeroStat::FireRate, vec![(10, 4.0)]);
        // 20 + 10 + 4 - (1 - 0.8 * 0.7) * 100 = -10%.
        assert!((value.unwrap() + 10.0).abs() < 1e-12);
        assert!(ignored.is_empty());
        assert_eq!(trace.len(), 5);
        assert_eq!(trace.iter().filter(|r| r.kind == "slow").count(), 2);
        assert_eq!(trace[0].value, 4.0);

        // A signed negative FIRE_RATE input is also a slow under this rule.
        modifiers["records"][2]["stat_changes"][2]["stat"] = json!(bonus);
        modifiers["records"][2]["stat_changes"][2]["value"] = json!(-20);
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!((stat_result(&catalog, true, HeroStat::FireRate).0.unwrap() + 14.0).abs() < 1e-12);
    }

    #[test]
    fn zero_hero_scaling_needs_no_spirit_and_unknown_scaling_fails() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("heroes.json");
        let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapScalingStats"]["EFireRate"] =
            json!({"eScalingStat":"ETechPower", "flScale":0});
        std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
            0.0
        );
        heroes["records"][0]["definition"]["m_mapScalingStats"]["EFireRate"]["eScalingStat"] =
            json!("unknown");
        std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::FireRate)
                .0
                .unwrap_err()
                .to_string()
                .contains("scaling stat")
        );
    }

    #[test]
    fn reload_time_uses_catalog_duration_and_only_effective_adjustments() {
        let folder = super::super::catalog::tests::fixture();
        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let stat = "MODIFIER_VALUE_RELOAD_SPEED";
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::ReloadTime)
                .0
                .unwrap_err()
                .to_string()
                .contains("base reload duration")
        );

        let effect = json!({"stat":stat, "value":-20, "definition_path":"/test_gun/reload"});
        // The bound adjustment counts once, even if its binding is duplicated.
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":stat, "value":-20, "definition_path":"/test_gun/reload", "modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE", "value":50}
        ]);
        modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        for (base, single) in [(2.5, false), (0.4, true)] {
            abilities["records"][0]["definition"]["m_WeaponInfo"] = json!({
                "m_reloadDuration":base, "m_bReloadSingleBullets":single,
                "m_flReloadSingleBulletsInitialDelay":0.7, "m_iClipSize":30
            });
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let (inactive, _, _) = stat_result(&catalog, false, HeroStat::ReloadTime);
            let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::ReloadTime);
            assert_eq!(inactive.unwrap(), base);
            assert!((active.unwrap() - base * 0.8).abs() < 1e-12);
            assert!(ignored.is_empty());
            assert_eq!(trace.len(), 2);
            assert_eq!(trace[0].value, base);
            assert_eq!(trace[1].value, -20.0);
            assert_eq!(trace[1].modifier_serial, Some(42));
            assert!(trace.iter().all(|c| c.input == "reload_time"));
        }

        // An intrinsic property also applies without a modifier binding.
        abilities["records"][0]["stat_changes"][0] = json!({
            "stat":stat, "value":-10, "definition_path":"/test_gun/passive",
            "usage_flags":"IntrinsicallyProvidedInAbility"
        });
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            (stat_result(&catalog, false, HeroStat::ReloadTime)
                .0
                .unwrap()
                - 0.36)
                .abs()
                < 1e-12
        );
        // Do not assume additive or multiplicative stacking from VData names.
        assert!(
            stat_result(&catalog, true, HeroStat::ReloadTime)
                .0
                .unwrap_err()
                .to_string()
                .contains("combining")
        );

        abilities["records"][0]["stat_changes"][0]["usage_flags"] = json!("ConditionallyApplied");
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::ReloadTime)
                .0
                .unwrap_err()
                .to_string()
                .contains("conditional")
        );

        abilities["records"][0]["stat_changes"] = json!([]);
        abilities["records"][0]["definition"]["m_WeaponInfo"]["m_bReloadUseActiveWeaponInfoDuration"] =
            json!(true);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, false, HeroStat::ReloadTime)
                .0
                .unwrap_err()
                .to_string()
                .contains("dynamic weapon")
        );
    }

    #[test]
    fn melee_distance_resolves_catalog_bonuses_without_double_counting() {
        let folder = super::super::catalog::tests::fixture();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            stat_result(&catalog, true, HeroStat::MeleeDistance)
                .0
                .unwrap(),
            0.0
        );

        let ability_path = folder.path().join("abilities.json");
        let modifier_path = folder.path().join("modifiers.json");
        let mut abilities: Value =
            serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
        let mut modifiers: Value =
            serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
        let stat = "MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE";
        for bonus in [27.5, 83.0] {
            let effect = json!({"stat":stat, "value":bonus, "definition_path":"/test_gun/buff"});
            abilities["records"][0]["stat_changes"] = json!([
                {"stat":stat, "value":12.5, "definition_path":"/test_gun/passive"},
                {"stat":stat, "value":bonus, "definition_path":"/test_gun/buff", "modifier_keys":["abilities#/second"]},
                {"stat":"MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT", "value":99}
            ]);
            // Duplicate property bindings in a modifier must count only once.
            modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
            std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
            std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
            let catalog = StatCatalog::from_directory(folder.path()).unwrap();
            let (inactive, _, _) = stat_result(&catalog, false, HeroStat::MeleeDistance);
            let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::MeleeDistance);
            assert_eq!(inactive.unwrap(), 12.5);
            assert_eq!(active.unwrap(), 12.5 + bonus);
            assert_eq!(trace.len(), 2);
            assert!(ignored.is_empty());
            assert!(
                trace
                    .iter()
                    .all(|c| c.input == "melee_distance" && c.kind == "percent")
            );
            assert_eq!(trace[1].modifier_serial, Some(42));
            assert_eq!(trace[1].value, bonus);
        }

        // Skip a missing modifier, retain known bonuses, and report its ID.
        modifiers["records"].as_array_mut().unwrap().pop();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (value, trace, ignored) = stat_result(&catalog, true, HeroStat::MeleeDistance);
        assert_eq!(value.unwrap(), 12.5);
        assert_eq!(trace.len(), 1);
        assert_eq!(ignored[&12], "unresolved modifier ID 12");
    }

    #[test]
    fn ping_markers_do_not_change_stats_or_report_missing_inputs() {
        let folder = super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let definition = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        definition["m_mapScalingStats"] = json!({});
        definition["m_mapStartingStats"] = json!({});
        for stat in [
            HeroStat::ClipSize,
            HeroStat::FireRate,
            HeroStat::DebuffResist,
        ] {
            let (baseline, baseline_trace, _) = stat_result(&catalog, false, stat);
            let baseline = baseline.unwrap();
            for id in [PLAYER_PINGED, ENTITY_PINGED, 123456] {
                let modifier = CModifierTableEntry {
                    modifier_subclass: Some(id),
                    serial_number: Some(42),
                    ..Default::default()
                };
                let mut inferred = BTreeSet::new();
                let (value, trace, ignored) = stat_result_with_modifier(
                    &catalog,
                    stat,
                    vec![(id, 10.0)],
                    Some(&modifier),
                    true,
                    &mut inferred,
                );
                assert_eq!(value.unwrap(), baseline);
                assert_eq!(trace.len(), baseline_trace.len());
                assert!(trace.iter().all(|c| c.modifier_serial != Some(42)));
                assert!(inferred.is_empty());
                if id == 123456 {
                    assert_eq!(ignored.len(), 1);
                    assert_eq!(ignored[&id], "unresolved modifier ID 123456");
                } else {
                    assert!(ignored.is_empty());
                }
            }
        }
    }

    #[test]
    fn missing_and_ambiguous_sources_are_reported_once_without_losing_known_totals() {
        let folder = super::super::catalog::tests::fixture();
        let path = folder.path().join("heroes.json");
        let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        heroes["records"][0]["definition"]["m_mapScalingStats"] = json!({});
        std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (value, _, ignored) = stat_result_with_permanent(
            &catalog,
            true,
            HeroStat::ClipSize,
            vec![(10, 4.0), (123456, 10.0), (123456, 2.0), (6, 1.0)],
        );
        assert_eq!(value.unwrap(), 21.0); // ceil(20 * 1.04)
        assert_eq!(ignored.len(), 2); // Repeated input passes also deduplicate.
        assert_eq!(ignored[&123456], "unresolved modifier ID 123456");
        assert!(ignored[&6].contains("ambiguous modifier ID 6"));
    }

    #[test]
    fn powerup_uses_catalog_range_and_application_time() {
        let folder = super::super::catalog::tests::fixture();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let mut contributions = Vec::new();
        let resolver = Resolver {
            ctx: &ctx,
            catalog: &catalog,
            controller: &controller,
            hero_id: 999,
            slot: PlayerSlot(0),
            game_time: Some(800.0),
            game_start: Some(100.0),
            explain: true,
            contributions: &mut contributions,
            ignored_modifiers: BTreeMap::new(),
            inferred_bindings: BTreeSet::new(),
            unmapped_inputs: BTreeSet::new(),
        };
        let source: Record = serde_json::from_value(json!({"record_key":"misc#/any_powerup", "definition_path":"/any_powerup", "definition":{"m_flTimeMin":5,"m_flTimeMax":15}, "stat_changes":[]})).unwrap();
        let effect = json!({"value_min":10, "value_max":30});
        let mut entry = CModifierTableEntry {
            last_applied_time: Some(700.0),
            ..Default::default()
        };
        assert_eq!(
            resolver.effect(&effect, &source, Some(&entry)).unwrap(),
            20.0
        );
        entry.last_applied_time = Some(100.0);
        assert_eq!(
            resolver.effect(&effect, &source, Some(&entry)).unwrap(),
            10.0
        );
        entry.last_applied_time = Some(2000.0);
        assert_eq!(
            resolver.effect(&effect, &source, Some(&entry)).unwrap(),
            30.0
        );
        assert!(resolver.effect(&effect, &source, None).is_err());
        let zero_scale = json!({"value":10,"scaling":{"$value":{"_class":"scale_function_tech_damage", "m_flStatScale":0}}});
        assert_eq!(resolver.effect(&zero_scale, &source, None).unwrap(), 10.0);
        let nonzero_scale = json!({"value":10,"scaling":{"$value":{"_class":"scale_function_tech_damage", "m_flStatScale":0.2}}});
        assert!(resolver.effect(&nonzero_scale, &source, None).is_err());
        let unsupported = json!({"value":10,"scaling":{"$value":{"_class":"unknown"}}});
        assert!(resolver.effect(&unsupported, &source, None).is_err());
    }
}
