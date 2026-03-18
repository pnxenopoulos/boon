use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

const TROOPER_CLASSES: &[&str] = &["CNPC_Trooper", "CNPC_TrooperBoss"];

fn trooper_type(class_name: &str) -> &'static str {
    match class_name {
        "CNPC_Trooper" => "trooper",
        "CNPC_TrooperBoss" => "trooper_boss",
        _ => "unknown",
    }
}

fn get_i64(e: &boon::Entity, key: Option<u64>) -> i64 {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon::FieldValue::U32(n) => Some(*n as i64),
            boon::FieldValue::U64(n) => Some(*n as i64),
            boon::FieldValue::I32(n) => Some(*n as i64),
            boon::FieldValue::I64(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0)
}

fn get_f32(e: &boon::Entity, key: Option<u64>) -> f32 {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon::FieldValue::F32(f) => Some(*f),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// State snapshot for change detection.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TrooperState {
    health: i64,
    max_health: i64,
    team_num: i64,
    lane: i64,
    x_bits: u32,
    y_bits: u32,
    z_bits: u32,
}

#[derive(Serialize)]
struct TrooperOutput {
    tick: i32,
    trooper_type: &'static str,
    team_num: i64,
    lane: i64,
    health: i64,
    max_health: i64,
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Serialize)]
struct TrooperSummaryOutput {
    trooper_type: &'static str,
    team_num: i64,
    lane: i64,
    count: usize,
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

    let class_filter: HashSet<&str> = TROOPER_CLASSES.iter().copied().collect();

    let mut keys_resolved = false;
    let mut nk_health: Option<u64> = None;
    let mut nk_max_health: Option<u64> = None;
    let mut nk_team_num: Option<u64> = None;
    let mut nk_lane: Option<u64> = None;
    let mut nk_lifestate: Option<u64> = None;
    let mut nk_vec_x: Option<u64> = None;
    let mut nk_vec_y: Option<u64> = None;
    let mut nk_vec_z: Option<u64> = None;

    let mut prev_state: HashMap<i32, (bool, TrooperState)> = HashMap::new();
    let mut rows: Vec<TrooperOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved {
                for class_name in TROOPER_CLASSES {
                    if let Some(s) = ctx.serializers.get(class_name) {
                        nk_health = s.resolve_field_key("m_iHealth");
                        nk_max_health = s.resolve_field_key("m_iMaxHealth");
                        nk_team_num = s.resolve_field_key("m_iTeamNum");
                        nk_lane = s.resolve_field_key("m_iLane");
                        nk_lifestate = s.resolve_field_key("m_lifeState");
                        nk_vec_x = s.resolve_field_key(
                            "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                        );
                        nk_vec_y = s.resolve_field_key(
                            "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                        );
                        nk_vec_z = s.resolve_field_key(
                            "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                        );
                        break;
                    }
                }
                keys_resolved = true;
            }

            for (&idx, entity) in ctx.entities.iter() {
                if !TROOPER_CLASSES.contains(&entity.class_name.as_str()) {
                    continue;
                }

                let max_health = get_i64(entity, nk_max_health);
                if max_health == 0 {
                    continue;
                }

                let lifestate = get_i64(entity, nk_lifestate);
                let alive = lifestate == 0;

                let x = get_f32(entity, nk_vec_x);
                let y = get_f32(entity, nk_vec_y);
                let z = get_f32(entity, nk_vec_z);

                let current = TrooperState {
                    health: get_i64(entity, nk_health),
                    max_health,
                    team_num: get_i64(entity, nk_team_num),
                    lane: get_i64(entity, nk_lane),
                    x_bits: x.to_bits(),
                    y_bits: y.to_bits(),
                    z_bits: z.to_bits(),
                };

                let changed = match prev_state.get(&idx) {
                    None => true,
                    Some((was_alive, prev)) => alive != *was_alive || (alive && current != *prev),
                };

                if !changed {
                    continue;
                }
                prev_state.insert(idx, (alive, current));

                if !alive {
                    continue;
                }

                rows.push(TrooperOutput {
                    tick: ctx.tick,
                    trooper_type: trooper_type(&entity.class_name),
                    team_num: current.team_num,
                    lane: current.lane,
                    health: current.health,
                    max_health,
                    x,
                    y,
                    z,
                });
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.trooper_type.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        rows.retain(|r| r.tick >= min);
    }
    if let Some(max) = max_tick {
        rows.retain(|r| r.tick <= max);
    }

    if summary {
        let mut counts: std::collections::HashMap<(&str, i64, i64), usize> =
            std::collections::HashMap::new();
        for r in &rows {
            *counts
                .entry((r.trooper_type, r.team_num, r.lane))
                .or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| {
            a.0 .0
                .cmp(b.0 .0)
                .then_with(|| a.0 .1.cmp(&b.0 .1))
                .then_with(|| a.0 .2.cmp(&b.0 .2))
        });

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<TrooperSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|((ttype, team, lane), count)| TrooperSummaryOutput {
                    trooper_type: ttype,
                    team_num: *team,
                    lane: *lane,
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<14} {:>8} {:>6} {:>8}",
            "Type".bold(),
            "Team".bold(),
            "Lane".bold(),
            "Ticks".bold()
        );
        println!("{}", "-".repeat(40));

        for ((ttype, team, lane), count) in sorted.iter().take(limit) {
            println!(
                "{:<14} {:>8} {:>6} {:>8}",
                ttype.green(),
                team,
                lane,
                count
            );
        }

        println!(
            "\n{} total alive trooper ticks ({} unique groups){}",
            rows.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(rows.len());

        if json {
            let output: Vec<_> = rows.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:<14} {:>6} {:>6} {:>8} {:>8} {:>10} {:>10} {:>10}",
            "Tick".bold(),
            "Type".bold(),
            "Team".bold(),
            "Lane".bold(),
            "Health".bold(),
            "MaxHP".bold(),
            "X".bold(),
            "Y".bold(),
            "Z".bold()
        );
        println!("{}", "-".repeat(88));

        for r in rows.iter().take(limit) {
            println!(
                "{:<8} {:<14} {:>6} {:>6} {:>8} {:>8} {:>10.1} {:>10.1} {:>10.1}",
                r.tick,
                r.trooper_type.green(),
                r.team_num,
                r.lane,
                r.health,
                r.max_health,
                r.x,
                r.y,
                r.z
            );
        }

        println!(
            "\n{} alive trooper tick rows{}",
            rows.len(),
            if limit < rows.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
