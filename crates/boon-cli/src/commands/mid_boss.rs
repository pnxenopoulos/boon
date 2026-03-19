use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
struct MidBossOutput {
    tick: i32,
    hero_id: i64,
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

    let class_filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();

    let mut pk_hero_id: Option<u64> = None;
    let mut keys_resolved = false;
    let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut entity_to_hero_built = false;

    let mut rows: Vec<MidBossOutput> = Vec::new();

    parser
        .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerPawn") {
                    pk_hero_id =
                        s.resolve_field_key("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID");
                }
                keys_resolved = true;
            }

            if !entity_to_hero_built {
                for (&idx, entity) in ctx.entities.iter() {
                    if entity.class_name == "CCitadelPlayerPawn" {
                        let hid = pk_hero_id
                            .and_then(|k| entity.fields.get(&k))
                            .and_then(|v| match v {
                                boon::FieldValue::U32(n) => Some(*n as i64),
                                boon::FieldValue::U64(n) => Some(*n as i64),
                                boon::FieldValue::I32(n) => Some(*n as i64),
                                boon::FieldValue::I64(n) => Some(*n),
                                _ => None,
                            })
                            .unwrap_or(0);
                        if hid != 0 {
                            entity_to_hero.insert(idx, hid);
                        }
                    }
                }
                entity_to_hero_built = true;
            }

            for event in events {
                // MidBossSpawned (msg_type 349)
                if event.msg_type == 349 {
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        hero_id: 0,
                        team_num: 0,
                        event: "spawned".to_string(),
                    });
                }
                // BossKilled for mid_boss (msg_type 347, entity_killed_class == 8)
                if event.msg_type == 347
                    && let Ok(msg) = boon_proto::proto::CCitadelUserMsgBossKilled::decode(
                        event.payload.as_slice(),
                    )
                    && msg.entity_killed_class.unwrap_or(0) == 8
                {
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        hero_id: 0,
                        team_num: msg.objective_team.unwrap_or(0),
                        event: "killed".to_string(),
                    });
                }
                // RejuvStatus (msg_type 350)
                if event.msg_type == 350
                    && let Ok(msg) = boon_proto::proto::CCitadelUserMsgRejuvStatus::decode(
                        event.payload.as_slice(),
                    )
                {
                    let pawn_idx = (msg.player_pawn.unwrap_or(0) & 0x3FFF) as i32;
                    let hero_id = entity_to_hero.get(&pawn_idx).copied().unwrap_or(0);
                    let team_num = msg.user_team.unwrap_or(0);
                    let event_name = match msg.event_type.unwrap_or(0) {
                        6 => "picked_up",
                        7 => "used",
                        8 => "expired",
                        _ => "unknown",
                    };
                    rows.push(MidBossOutput {
                        tick: event.tick,
                        hero_id,
                        team_num,
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
            "{:<8} {:>8} {:>6} {}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Team".bold(),
            "Event".bold()
        );
        println!("{}", "-".repeat(40));

        for r in rows.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:>6} {}",
                r.tick,
                r.hero_id,
                r.team_num,
                r.event.green()
            );
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
