use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

const OBJECTIVE_CLASSES: &[&str] = &[
    "CNPC_Boss_Tier2",
    "CNPC_Boss_Tier3",
    "CNPC_BarrackBoss",
    "CNPC_MidBoss",
];

fn objective_type(class_name: &str) -> &'static str {
    match class_name {
        "CNPC_Boss_Tier2" => "walker",
        "CNPC_Boss_Tier3" => "titan",
        "CNPC_BarrackBoss" => "barracks",
        "CNPC_MidBoss" => "mid_boss",
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

/// State snapshot for change detection.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ObjectiveState {
    health: i64,
    max_health: i64,
    team_num: i64,
    lane: i64,
}

#[derive(Serialize)]
struct ObjectiveOutput {
    tick: i32,
    objective_type: &'static str,
    team_num: i64,
    lane: i64,
    health: i64,
    max_health: i64,
}

#[derive(Serialize)]
struct ObjectiveSummaryOutput {
    objective_type: &'static str,
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

    let class_filter: HashSet<&str> = OBJECTIVE_CLASSES.iter().copied().collect();

    let mut keys_resolved = false;
    // Keys are the same across all NPC classes (inherited from CAI_BaseNPC)
    let mut nk_health: Option<u64> = None;
    let mut nk_max_health: Option<u64> = None;
    let mut nk_team_num: Option<u64> = None;
    let mut nk_lane: Option<u64> = None;

    let mut prev_state: HashMap<i32, ObjectiveState> = HashMap::new();
    let mut rows: Vec<ObjectiveOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved {
                // All objective NPCs share the same base fields; resolve from the first one found
                for class_name in OBJECTIVE_CLASSES {
                    if let Some(s) = ctx.serializers.get(class_name) {
                        nk_health = s.resolve_field_key("m_iHealth");
                        nk_max_health = s.resolve_field_key("m_iMaxHealth");
                        nk_team_num = s.resolve_field_key("m_iTeamNum");
                        nk_lane = s.resolve_field_key("m_iLane");
                        break;
                    }
                }
                keys_resolved = true;
            }

            for (&idx, entity) in ctx.entities.iter() {
                if !OBJECTIVE_CLASSES.contains(&entity.class_name.as_str()) {
                    continue;
                }

                let max_health = get_i64(entity, nk_max_health);

                // Skip entities with no max_health (not yet initialized)
                if max_health == 0 {
                    continue;
                }

                let current = ObjectiveState {
                    health: get_i64(entity, nk_health),
                    max_health,
                    team_num: get_i64(entity, nk_team_num),
                    lane: get_i64(entity, nk_lane),
                };

                if prev_state.get(&idx) == Some(&current) {
                    continue;
                }
                prev_state.insert(idx, current);

                rows.push(ObjectiveOutput {
                    tick: ctx.tick,
                    objective_type: objective_type(&entity.class_name),
                    team_num: current.team_num,
                    lane: current.lane,
                    health: current.health,
                    max_health,
                });
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.objective_type.to_lowercase().contains(&f_lower));
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
                .entry((r.objective_type, r.team_num, r.lane))
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
            let output: Vec<ObjectiveSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|((obj_type, team, lane), count)| ObjectiveSummaryOutput {
                    objective_type: obj_type,
                    team_num: *team,
                    lane: *lane,
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<12} {:>8} {:>6} {:>8}",
            "Type".bold(),
            "Team".bold(),
            "Lane".bold(),
            "Ticks".bold()
        );
        println!("{}", "-".repeat(38));

        for ((obj_type, team, lane), count) in sorted.iter().take(limit) {
            println!(
                "{:<12} {:>8} {:>6} {:>8}",
                obj_type.green(),
                team,
                lane,
                count
            );
        }

        println!(
            "\n{} total rows ({} unique objectives){}",
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
            "{:<8} {:<12} {:>6} {:>6} {:>8} {:>8}",
            "Tick".bold(),
            "Type".bold(),
            "Team".bold(),
            "Lane".bold(),
            "Health".bold(),
            "MaxHP".bold()
        );
        println!("{}", "-".repeat(56));

        for r in rows.iter().take(limit) {
            println!(
                "{:<8} {:<12} {:>6} {:>6} {:>8} {:>8}",
                r.tick,
                r.objective_type.green(),
                r.team_num,
                r.lane,
                r.health,
                r.max_health
            );
        }

        println!(
            "\n{} objective tick rows{}",
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
