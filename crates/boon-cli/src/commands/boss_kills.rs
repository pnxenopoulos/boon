use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;
use serde::Serialize;

fn entity_class_name(class: i32) -> &'static str {
    match class {
        5 => "walker",
        8 => "mid_boss",
        28 => "titan_shield_generator",
        29 => "barracks",
        30 => "titan",
        31 => "core",
        _ => "unknown",
    }
}

#[derive(Serialize)]
struct BossKillOutput {
    tick: i32,
    objective_team: i32,
    objective_id: i32,
    entity_class: String,
    gametime: f32,
}

#[derive(Serialize)]
struct BossKillSummaryOutput {
    entity_class: String,
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

    let events = parser
        .events(None)
        .with_context(|| "failed to parse demo")?;

    let mut kills: Vec<BossKillOutput> = Vec::new();

    for event in &events {
        if event.msg_type == 347
            && let Ok(msg) =
                boon_proto::proto::CCitadelUserMsgBossKilled::decode(event.payload.as_slice())
        {
            let class_id = msg.entity_killed_class.unwrap_or(0);
            kills.push(BossKillOutput {
                tick: event.tick,
                objective_team: msg.objective_team.unwrap_or(0),
                objective_id: msg.objective_mask_change.unwrap_or(0),
                entity_class: entity_class_name(class_id).to_string(),
                gametime: msg.gametime.unwrap_or(0.0),
            });
        }
    }

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        kills.retain(|k| k.entity_class.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        kills.retain(|k| k.tick >= min);
    }
    if let Some(max) = max_tick {
        kills.retain(|k| k.tick <= max);
    }

    if summary {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for k in &kills {
            *counts.entry(k.entity_class.as_str()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<BossKillSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(entity_class, count)| BossKillSummaryOutput {
                    entity_class: entity_class.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!("{:<30} {:>6}", "Entity Class".bold(), "Count".bold());
        println!("{}", "-".repeat(38));

        for (entity_class, count) in sorted.iter().take(limit) {
            println!("{:<30} {:>6}", entity_class.green(), count);
        }

        println!(
            "\n{} boss kill events ({} unique types){}",
            kills.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(kills.len());

        if json {
            let output: Vec<_> = kills.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:>6} {:>6} {:<24} {:>10}",
            "Tick".bold(),
            "Team".bold(),
            "ObjID".bold(),
            "Class".bold(),
            "GameTime".bold()
        );
        println!("{}", "-".repeat(60));

        for k in kills.iter().take(limit) {
            println!(
                "{:<8} {:>6} {:>6} {:<24} {:>10.1}",
                k.tick,
                k.objective_team,
                k.objective_id,
                k.entity_class.green(),
                k.gametime
            );
        }

        println!(
            "\n{} boss kill events{}",
            kills.len(),
            if limit < kills.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
