use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use boon_proto::proto::CitadelUserMessageIds as Msg;
use colored::Colorize;
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
struct AbilityOutput {
    tick: i32,
    hero_id: i64,
    ability: String,
}

#[derive(Serialize)]
struct AbilitySummaryOutput {
    ability: String,
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

    let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut entity_to_hero_built = false;

    let mut pk_hero_id: Option<u64> = None;
    let mut keys_resolved = false;

    let mut abilities: Vec<AbilityOutput> = Vec::new();

    parser
        .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
            // Resolve field keys once
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerPawn") {
                    pk_hero_id =
                        s.resolve_field_key("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID");
                }
                keys_resolved = true;
            }

            // Build entity_to_hero map once
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
                if event.msg_type == Msg::KEUserMsgImportantAbilityUsed as u32
                    && let Ok(msg) =
                        boon_proto::proto::CCitadelUserMessageImportantAbilityUsed::decode(
                            event.payload.as_slice(),
                        )
                {
                    let pawn_idx = (msg.player.unwrap_or(0) & 0x3FFF) as i32;
                    let hero_id = entity_to_hero.get(&pawn_idx).copied().unwrap_or(0);
                    abilities.push(AbilityOutput {
                        tick: event.tick,
                        hero_id,
                        ability: msg.ability_name.unwrap_or_default(),
                    });
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        abilities.retain(|a| a.ability.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        abilities.retain(|a| a.tick >= min);
    }
    if let Some(max) = max_tick {
        abilities.retain(|a| a.tick <= max);
    }

    if summary {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for a in &abilities {
            *counts.entry(a.ability.as_str()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<AbilitySummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(ability, count)| AbilitySummaryOutput {
                    ability: ability.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!("{:<50} {:>6}", "Ability".bold(), "Count".bold());
        println!("{}", "-".repeat(58));

        for (ability, count) in sorted.iter().take(limit) {
            println!("{:<50} {:>6}", ability.green(), count);
        }

        println!(
            "\n{} ability events ({} unique abilities){}",
            abilities.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(abilities.len());

        if json {
            let output: Vec<_> = abilities.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:>8} {}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Ability".bold()
        );
        println!("{}", "-".repeat(58));

        for a in abilities.iter().take(limit) {
            println!("{:<8} {:>8} {}", a.tick, a.hero_id, a.ability.green());
        }

        println!(
            "\n{} ability events{}",
            abilities.len(),
            if limit < abilities.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
