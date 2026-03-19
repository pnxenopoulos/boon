use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
struct ShopEventOutput {
    tick: i32,
    hero_id: i64,
    ability_id: u32,
    ability: String,
    change: String,
}

#[derive(Serialize)]
struct ShopEventSummaryOutput {
    ability: String,
    change: String,
    count: usize,
}

fn change_name(change: i32) -> &'static str {
    match change {
        0 => "purchased",
        1 => "upgraded",
        2 => "sold",
        3 => "swapped",
        4 => "failure",
        _ => "unknown",
    }
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
    let mut slot_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut slot_to_hero_built = false;

    let mut events_out: Vec<ShopEventOutput> = Vec::new();

    parser
        .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
            // Resolve field keys once
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerController") {
                    ck_hero_id = s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                }
                keys_resolved = true;
            }

            // Build slot_to_hero map once
            if !slot_to_hero_built {
                for (&idx, entity) in ctx.entities.iter() {
                    if entity.class_name == "CCitadelPlayerController" {
                        let hid = ck_hero_id
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
                            // userid is 0-based, controller entity index is 1-based
                            slot_to_hero.insert(idx - 1, hid);
                        }
                    }
                }
                slot_to_hero_built = true;
            }

            // Collect AbilitiesChanged events (msg_type 309)
            for event in events {
                if event.msg_type == 309
                    && let Ok(msg) = boon_proto::proto::CCitadelUserMsgAbilitiesChanged::decode(
                        event.payload.as_slice(),
                    )
                {
                    let player_slot = msg.purchaser_player_slot.unwrap_or(-1);
                    let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                    let ability_id = msg.ability_id.unwrap_or(0);
                    let change = msg.change.unwrap_or(-1);

                    events_out.push(ShopEventOutput {
                        tick: event.tick,
                        hero_id,
                        ability_id,
                        ability: boon::ability_name(ability_id).to_string(),
                        change: change_name(change).to_string(),
                    });
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events_out.retain(|e| {
            e.ability.to_lowercase().contains(&f_lower)
                || e.change.to_lowercase().contains(&f_lower)
        });
    }
    if let Some(min) = min_tick {
        events_out.retain(|e| e.tick >= min);
    }
    if let Some(max) = max_tick {
        events_out.retain(|e| e.tick <= max);
    }

    if summary {
        let mut counts: HashMap<(&str, &str), usize> = HashMap::new();
        for e in &events_out {
            *counts
                .entry((e.ability.as_str(), e.change.as_str()))
                .or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<ShopEventSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|((ability, change), count)| ShopEventSummaryOutput {
                    ability: ability.to_string(),
                    change: change.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<40} {:<12} {:>6}",
            "Ability".bold(),
            "Change".bold(),
            "Count".bold()
        );
        println!("{}", "-".repeat(60));

        for ((ability, change), count) in sorted.iter().take(limit) {
            println!("{:<40} {:<12} {:>6}", ability.green(), change, count);
        }

        println!(
            "\n{} shop events ({} unique ability+change combos){}",
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
            "{:<8} {:>8} {:<12} {:<40} {}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Ability ID".bold(),
            "Ability".bold(),
            "Change".bold()
        );
        println!("{}", "-".repeat(80));

        for e in events_out.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<12} {:<40} {}",
                e.tick,
                e.hero_id,
                e.ability_id,
                e.ability.green(),
                e.change
            );
        }

        println!(
            "\n{} shop events{}",
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
