use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use boon::{StatModifierKind, decode_stat_modifier_value_type};
use colored::Colorize;
use serde::Serialize;

#[derive(Serialize)]
struct StatModifierOutput {
    tick: i32,
    hero_id: i64,
    stat: &'static str,
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
    bullet_resist: f32,
    spirit_resist: f32,
}

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

    // Previous state per hero, ordered by StatModifierKind::index().
    let mut prev_state: HashMap<i64, [f32; StatModifierKind::COUNT]> = HashMap::new();
    let mut events_out: Vec<StatModifierOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved {
                if let Some(s) = ctx.serializers().get("CCitadelPlayerController") {
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

            for (_, entity) in ctx.entities().iter() {
                if entity.class_name.as_ref() != "CCitadelPlayerController" {
                    continue;
                }
                let hero_id = entity.get_i64(ck_hero_id);
                if hero_id == 0 {
                    continue;
                }

                // Sum values by eValType
                let mut sums = [0.0f32; StatModifierKind::COUNT];
                for (mid_key, vt_key, val_key) in &sv_keys {
                    let mid = entity.get_u32(*mid_key);
                    let vt = entity.get_u32(*vt_key);
                    let val = entity.get_f32(*val_key);
                    if mid == 0 && vt == 0 && val == 0.0 {
                        continue;
                    }
                    if let Some(decoded) = decode_stat_modifier_value_type(vt) {
                        sums[decoded.kind.index()] += val * decoded.value_scale;
                    }
                }

                // Compare to previous state, emit changes
                let prev = prev_state
                    .entry(hero_id)
                    .or_insert([0.0f32; StatModifierKind::COUNT]);
                for stat in StatModifierKind::ALL {
                    let i = stat.index();
                    if (sums[i] - prev[i]).abs() > f32::EPSILON {
                        events_out.push(StatModifierOutput {
                            tick: ctx.tick(),
                            hero_id,
                            stat: stat.name(),
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
                bullet_resist: sums[6],
                spirit_resist: sums[7],
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
            "{:>8} {:>8} {:>14} {:>10} {:>15} {:>18} {:>6} {:>14} {:>14}",
            "Hero ID".bold(),
            "Health".bold(),
            "Spirit Power".bold(),
            "Fire Rate".bold(),
            "Weapon Damage".bold(),
            "Cooldown Reduction".bold(),
            "Ammo".bold(),
            "Bullet Resist".bold(),
            "Spirit Resist".bold()
        );
        println!("{}", "-".repeat(112));

        for s in summaries.iter().take(limit) {
            println!(
                "{:>8} {:>8.1} {:>14.1} {:>10.3} {:>15.3} {:>18.3} {:>6.3} {:>14.3} {:>14.3}",
                s.hero_id,
                s.health,
                s.spirit_power,
                s.fire_rate,
                s.weapon_damage,
                s.cooldown_reduction,
                s.ammo,
                s.bullet_resist,
                s.spirit_resist
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
