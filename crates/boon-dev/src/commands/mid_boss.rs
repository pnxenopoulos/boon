use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use boon_proto::proto::CitadelUserMessageIds as Msg;
use colored::Colorize;
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
struct MidBossOutput {
    tick: i32,
    team_num: i32,
    event: String,
}

#[derive(Serialize)]
struct MidBossSummaryOutput {
    event: String,
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

    let class_filter: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut rows: Vec<MidBossOutput> = Vec::new();

    parser
        .run_to_end_with_events_filtered(&class_filter, |_ctx, events| {
            for event in events {
                if event.msg_type == Msg::KEUserMsgMidBossSpawned as u32 {
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        team_num: 0,
                        event: "spawned".to_string(),
                    });
                }
                if event.msg_type == Msg::KEUserMsgBossKilled as u32
                    && let Ok(msg) = boon_proto::proto::CCitadelUserMsgBossKilled::decode(
                        event.payload.as_slice(),
                    )
                    && msg.entity_killed_class.unwrap_or(0) == 8
                // mid_boss entity class
                {
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        team_num: msg.objective_team.unwrap_or(0),
                        event: "killed".to_string(),
                    });
                }
                if event.msg_type == Msg::KEUserMsgRejuvStatus as u32
                    && let Ok(msg) = boon_proto::proto::CCitadelUserMsgRejuvStatus::decode(
                        event.payload.as_slice(),
                    )
                {
                    // RejuvStatus event_type enum from proto
                    let event_name = match msg.event_type.unwrap_or(0) {
                        6 => "picked_up", // rejuv buff picked up
                        7 => "used",      // rejuv buff consumed
                        8 => "expired",   // rejuv buff expired
                        _ => "unknown",
                    };
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        team_num: msg.user_team.unwrap_or(0),
                        event: event_name.to_string(),
                    });
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Sort by tick
    rows.sort_by_key(|r| r.tick);

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.event.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        rows.retain(|r| r.tick >= min);
    }
    if let Some(max) = max_tick {
        rows.retain(|r| r.tick <= max);
    }

    if summary {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for r in &rows {
            *counts.entry(r.event.as_str()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<MidBossSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(event, count)| MidBossSummaryOutput {
                    event: event.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!("{:<12} {:>6}", "Event".bold(), "Count".bold());
        println!("{}", "-".repeat(20));

        for (event, count) in sorted.iter().take(limit) {
            println!("{:<12} {:>6}", event.green(), count);
        }

        println!(
            "\n{} mid boss events{}",
            rows.len(),
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
            "{:<8} {:>6} {}",
            "Tick".bold(),
            "Team".bold(),
            "Event".bold()
        );
        println!("{}", "-".repeat(30));

        for r in rows.iter().take(limit) {
            println!("{:<8} {:>6} {}", r.tick, r.team_num, r.event.green());
        }

        println!(
            "\n{} mid boss events{}",
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
