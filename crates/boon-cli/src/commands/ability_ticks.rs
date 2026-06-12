use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

/// Field keys for one ability entity class, resolved once and cached.
struct AbilityKeys {
    subclass_id: Option<u64>,
    slot: Option<u64>,
    cooldown_start: Option<u64>,
    cooldown_end: Option<u64>,
    remaining_charges: Option<u64>,
    recharge_start: Option<u64>,
    recharge_end: Option<u64>,
    owner: Option<u64>,
}

/// The cooldown/charge state we change-detect on, per ability entity.
#[derive(PartialEq)]
struct AbilState {
    cooldown_start: f32,
    cooldown_end: f32,
    remaining_charges: i32,
    recharge_start: f32,
    recharge_end: f32,
}

#[derive(Serialize)]
struct AbilityTickOutput {
    tick: i32,
    hero_id: i64,
    ability: String,
    slot: i32,
    cooldown_start: f32,
    cooldown_end: f32,
    remaining_charges: i32,
    charge_recharge_start: f32,
    charge_recharge_end: f32,
}

#[derive(Serialize)]
struct AbilityTickSummary {
    hero_id: i64,
    ability: String,
    count: usize,
}

/// List ability cooldown/charge state changes from a demo.
///
/// Change-only: a row is emitted for an ability only on the tick its cooldown or
/// charge state changes. Each ability is its own networked entity, so we walk the
/// decoded ability entities each tick, resolving their per-class field keys once
/// and linking each back to its owning hero via `m_hOwnerEntity`.
pub fn run(
    file: &Path,
    filter: Option<String>,
    summary: bool,
    limit: Option<usize>,
    min_tick: Option<i32>,
    max_tick: Option<i32>,
    json: bool,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;

    // Every ability is its own networked class (hundreds of them). Collect their
    // names from the send tables — any class whose name contains "Ability" — and
    // decode those plus the pawns (for the owner -> hero mapping).
    let ability_class_names: Vec<String> = parser
        .parse_send_tables()
        .map(|sc| {
            sc.serializers
                .keys()
                .filter(|n| n.contains("Ability"))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let mut class_filter: HashSet<&str> = HashSet::new();
    class_filter.insert("CCitadelPlayerPawn");
    for n in &ability_class_names {
        class_filter.insert(n.as_str());
    }

    let mut keys_resolved = false;
    let mut pk_hero_id: Option<u64> = None;
    let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut entity_to_hero_built = false;

    // Per-ability-class resolved field keys, and per-entity previous state.
    let mut ability_keys_cache: HashMap<String, AbilityKeys> = HashMap::new();
    let mut prev: HashMap<i32, AbilState> = HashMap::new();
    let mut events_out: Vec<AbilityTickOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            // Resolve pawn hero_id key once (retry until serializers available).
            if !keys_resolved && let Some(s) = ctx.serializers.get("CCitadelPlayerPawn") {
                pk_hero_id = s.resolve_field_key("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID");
                keys_resolved = true;
            }

            // Build entity_to_hero map (retry until populated).
            if !entity_to_hero_built {
                for (&idx, entity) in ctx.entities.iter() {
                    if entity.class_name == "CCitadelPlayerPawn" {
                        let hid = entity.get_i64(pk_hero_id);
                        if hid != 0 {
                            entity_to_hero.insert(idx, hid);
                        }
                    }
                }
                if !entity_to_hero.is_empty() {
                    entity_to_hero_built = true;
                }
            }

            for (&idx, entity) in ctx.entities.iter() {
                if !entity.class_name.contains("Ability") {
                    continue;
                }
                if !ability_keys_cache.contains_key(&entity.class_name) {
                    let s = ctx.serializers.get(&entity.class_name);
                    let r = |p: &str| s.and_then(|s| s.resolve_field_key(p));
                    ability_keys_cache.insert(
                        entity.class_name.clone(),
                        AbilityKeys {
                            subclass_id: r("m_nSubclassID"),
                            slot: r("m_eAbilitySlot"),
                            cooldown_start: r("m_flCooldownStart"),
                            cooldown_end: r("m_flCooldownEnd"),
                            remaining_charges: r("m_iRemainingCharges"),
                            recharge_start: r("m_flChargeRechargeStart"),
                            recharge_end: r("m_flChargeRechargeEnd"),
                            owner: r("m_hOwnerEntity"),
                        },
                    );
                }
                let keys = &ability_keys_cache[&entity.class_name];
                // Capability gate: real abilities expose cooldown + charges.
                if keys.cooldown_end.is_none() || keys.remaining_charges.is_none() {
                    continue;
                }
                let hero_id = entity
                    .get_handle(keys.owner)
                    .map(|h| (h & boon::ENTITY_HANDLE_INDEX_MASK) as i32)
                    .and_then(|owner_idx| entity_to_hero.get(&owner_idx).copied())
                    .unwrap_or(0);
                if hero_id == 0 {
                    continue;
                }
                let state = AbilState {
                    cooldown_start: entity.get_f32(keys.cooldown_start),
                    cooldown_end: entity.get_f32(keys.cooldown_end),
                    remaining_charges: entity.get_i64(keys.remaining_charges) as i32,
                    recharge_start: entity.get_f32(keys.recharge_start),
                    recharge_end: entity.get_f32(keys.recharge_end),
                };
                let changed = prev.get(&idx).map(|p| *p != state).unwrap_or(true);
                if changed {
                    events_out.push(AbilityTickOutput {
                        tick: ctx.tick,
                        hero_id,
                        ability: boon::ability_name(entity.get_u32(keys.subclass_id)).to_string(),
                        slot: entity.get_i64(keys.slot) as i32,
                        cooldown_start: state.cooldown_start,
                        cooldown_end: state.cooldown_end,
                        remaining_charges: state.remaining_charges,
                        charge_recharge_start: state.recharge_start,
                        charge_recharge_end: state.recharge_end,
                    });
                    prev.insert(idx, state);
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filters.
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events_out.retain(|e| e.ability.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        events_out.retain(|e| e.tick >= min);
    }
    if let Some(max) = max_tick {
        events_out.retain(|e| e.tick <= max);
    }

    if summary {
        let mut counts: HashMap<(i64, &str), usize> = HashMap::new();
        for e in &events_out {
            *counts.entry((e.hero_id, e.ability.as_str())).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<AbilityTickSummary> = sorted
                .iter()
                .take(limit)
                .map(|((hero_id, ability), count)| AbilityTickSummary {
                    hero_id: *hero_id,
                    ability: ability.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:>8} {:<40} {:>8}",
            "Hero ID".bold(),
            "Ability".bold(),
            "Changes".bold()
        );
        println!("{}", "-".repeat(58));
        for ((hero_id, ability), count) in sorted.iter().take(limit) {
            println!("{:>8} {:<40} {:>8}", hero_id, ability.green(), count);
        }
        println!(
            "\n{} state changes ({} unique hero+ability combos){}",
            events_out.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(events_out.len());

        if json {
            let output: Vec<_> = events_out.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:>7} {:<34} {:>5} {:>9} {:>9} {:>5} {:>9} {:>9}",
            "Tick".bold(),
            "Hero".bold(),
            "Ability".bold(),
            "Slot".bold(),
            "CD Start".bold(),
            "CD End".bold(),
            "Chg".bold(),
            "Rch Strt".bold(),
            "Rch End".bold(),
        );
        println!("{}", "-".repeat(106));
        for e in events_out.iter().take(limit) {
            println!(
                "{:<8} {:>7} {:<34} {:>5} {:>9.1} {:>9.1} {:>5} {:>9.1} {:>9.1}",
                e.tick,
                e.hero_id,
                e.ability.green(),
                e.slot,
                e.cooldown_start,
                e.cooldown_end,
                e.remaining_charges,
                e.charge_recharge_start,
                e.charge_recharge_end,
            );
        }
        println!(
            "\n{} ability state changes{}",
            events_out.len(),
            if limit < events_out.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
