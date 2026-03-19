use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

// eValType → stat name mapping (from misc.vdata idol pickup definitions)
const EVAL_HEALTH: u32 = 28;
const EVAL_SPIRIT_POWER: u32 = 48;
const EVAL_FIRE_RATE: u32 = 76;
const EVAL_WEAPON_DAMAGE: u32 = 15;
const EVAL_COOLDOWN_REDUCTION: u32 = 106;
const EVAL_AMMO: u32 = 169;

/// Index into the 6-stat array by eValType
fn stat_index(val_type: u32) -> Option<usize> {
    match val_type {
        EVAL_HEALTH => Some(0),
        EVAL_SPIRIT_POWER => Some(1),
        EVAL_FIRE_RATE => Some(2),
        EVAL_WEAPON_DAMAGE => Some(3),
        EVAL_COOLDOWN_REDUCTION => Some(4),
        EVAL_AMMO => Some(5),
        _ => None,
    }
}

fn get_i64(entity: &boon::Entity, key: Option<u64>) -> i64 {
    key.and_then(|k| entity.fields.get(&k))
        .and_then(|v| match v {
            boon::FieldValue::I32(n) => Some(*n as i64),
            boon::FieldValue::I64(n) => Some(*n),
            boon::FieldValue::U32(n) => Some(*n as i64),
            boon::FieldValue::U64(n) => Some(*n as i64),
            _ => None,
        })
        .unwrap_or(0)
}

fn get_u32(entity: &boon::Entity, key: Option<u64>) -> u32 {
    key.and_then(|k| entity.fields.get(&k))
        .and_then(|v| match v {
            boon::FieldValue::U32(n) => Some(*n),
            boon::FieldValue::I32(n) => Some(*n as u32),
            boon::FieldValue::U64(n) => Some(*n as u32),
            boon::FieldValue::I64(n) => Some(*n as u32),
            _ => None,
        })
        .unwrap_or(0)
}

fn get_f32(entity: &boon::Entity, key: Option<u64>) -> f32 {
    key.and_then(|k| entity.fields.get(&k))
        .and_then(|v| match v {
            boon::FieldValue::F32(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0.0)
}

#[derive(Serialize)]
struct StatModifierOutput {
    tick: i32,
    hero_id: i64,
    stat: String,
    value: f32,
}

#[derive(Serialize)]
struct StatModifierSummary {
    hero_id: i64,
    health: f32,
    spirit_power: f32,
    fire_rate: f32,
    weapon_damage: f32,
    cooldown_reduction: f32,
    ammo: f32,
}

const STAT_NAMES: [&str; 6] = [
    "health",
    "spirit_power",
    "fire_rate",
    "weapon_damage",
    "cooldown_reduction",
    "ammo",
];

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

    let class_filter: HashSet<&str> = ["CCitadelPlayerController"].into_iter().collect();

    let mut keys_resolved = false;
    let mut ck_hero_id: Option<u64> = None;
    // StatViewerModifierValues keys for indices 0..20: (modifier_id, val_type, value)
    let mut sv_keys: Vec<(Option<u64>, Option<u64>, Option<u64>)> = Vec::new();

    // Previous state per hero: [health, spirit_power, fire_rate, weapon_damage, cooldown_reduction, ammo]
    let mut prev_state: HashMap<i64, [f32; 6]> = HashMap::new();
    let mut events_out: Vec<StatModifierOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerController") {
                    ck_hero_id = s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                    for i in 0..20 {
                        let mid = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_SourceModifierID"
                        ));
                        let vt = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_eValType"
                        ));
                        let val = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_flValue"
                        ));
                        sv_keys.push((mid, vt, val));
                    }
                }
                keys_resolved = true;
            }

            for (_, entity) in ctx.entities.iter() {
                if entity.class_name != "CCitadelPlayerController" {
                    continue;
                }
                let hero_id = get_i64(entity, ck_hero_id);
                if hero_id == 0 {
                    continue;
                }

                // Sum values by eValType
                let mut sums = [0.0f32; 6];
                for (mid_key, vt_key, val_key) in &sv_keys {
                    let mid = get_u32(entity, *mid_key);
                    let vt = get_u32(entity, *vt_key);
                    let val = get_f32(entity, *val_key);
                    if mid == 0 && vt == 0 && val == 0.0 {
                        continue;
                    }
                    if let Some(idx) = stat_index(vt) {
                        sums[idx] += val;
                    }
                }

                // Compare to previous state, emit changes
                let prev = prev_state.entry(hero_id).or_insert([0.0f32; 6]);
                for i in 0..6 {
                    if sums[i] != prev[i] && sums[i] > prev[i] {
                        events_out.push(StatModifierOutput {
                            tick: ctx.tick,
                            hero_id,
                            stat: STAT_NAMES[i].to_string(),
                            value: sums[i],
                        });
                    }
                }
                *prev = sums;
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events_out.retain(|e| e.stat.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        events_out.retain(|e| e.tick >= min);
    }
    if let Some(max) = max_tick {
        events_out.retain(|e| e.tick <= max);
    }

    if summary {
        // Build per-hero final values from prev_state
        let mut summaries: Vec<StatModifierSummary> = prev_state
            .iter()
            .map(|(&hero_id, sums)| StatModifierSummary {
                hero_id,
                health: sums[0],
                spirit_power: sums[1],
                fire_rate: sums[2],
                weapon_damage: sums[3],
                cooldown_reduction: sums[4],
                ammo: sums[5],
            })
            .collect();
        summaries.sort_by_key(|s| s.hero_id);

        let limit = limit.unwrap_or(summaries.len());

        if json {
            let output: Vec<_> = summaries.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:>8} {:>8} {:>14} {:>10} {:>15} {:>18} {:>6}",
            "Hero ID".bold(),
            "Health".bold(),
            "Spirit Power".bold(),
            "Fire Rate".bold(),
            "Weapon Damage".bold(),
            "Cooldown Reduction".bold(),
            "Ammo".bold()
        );
        println!("{}", "-".repeat(80));

        for s in summaries.iter().take(limit) {
            println!(
                "{:>8} {:>8.1} {:>14.1} {:>10.3} {:>15.3} {:>18.3} {:>6.3}",
                s.hero_id,
                s.health,
                s.spirit_power,
                s.fire_rate,
                s.weapon_damage,
                s.cooldown_reduction,
                s.ammo
            );
        }

        println!(
            "\n{} heroes{}",
            summaries.len(),
            if limit < summaries.len() {
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
            "{:<8} {:>8} {:<20} {:>10}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Stat".bold(),
            "Value".bold()
        );
        println!("{}", "-".repeat(50));

        for e in events_out.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<20} {:>10.3}",
                e.tick,
                e.hero_id,
                e.stat.green(),
                e.value
            );
        }

        println!(
            "\n{} stat modifier changes{}",
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
