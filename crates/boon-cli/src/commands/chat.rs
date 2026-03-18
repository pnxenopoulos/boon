use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
struct ChatOutput {
    tick: i32,
    hero_id: i64,
    text: String,
    chat_type: String,
}

#[derive(Serialize)]
struct ChatSummaryOutput {
    hero_id: i64,
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

    let class_filter: HashSet<&str> = ["CCitadelPlayerController"].into_iter().collect();

    let mut keys_resolved = false;
    let mut ck_hero_id: Option<u64> = None;
    let mut slot_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut slot_to_hero_built = false;

    let mut messages: Vec<ChatOutput> = Vec::new();

    parser
        .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerController") {
                    ck_hero_id = s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                }
                keys_resolved = true;
            }

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
                            slot_to_hero.insert(idx - 1, hid);
                        }
                    }
                }
                slot_to_hero_built = true;
            }

            for event in events {
                if event.msg_type == 314
                    && let Ok(msg) =
                        boon_proto::proto::CCitadelUserMsgChatMsg::decode(
                            event.payload.as_slice(),
                        )
                {
                    let player_slot = msg.player_slot.unwrap_or(-1);
                    let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                    let chat_type = if msg.all_chat.unwrap_or(false) {
                        "all"
                    } else {
                        "team"
                    };

                    messages.push(ChatOutput {
                        tick: event.tick,
                        hero_id,
                        text: msg.text.unwrap_or_default(),
                        chat_type: chat_type.to_string(),
                    });
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        messages.retain(|m| {
            m.text.to_lowercase().contains(&f_lower)
                || m.chat_type.to_lowercase().contains(&f_lower)
        });
    }
    if let Some(min) = min_tick {
        messages.retain(|m| m.tick >= min);
    }
    if let Some(max) = max_tick {
        messages.retain(|m| m.tick <= max);
    }

    if summary {
        let mut counts: HashMap<i64, usize> = HashMap::new();
        for m in &messages {
            *counts.entry(m.hero_id).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<ChatSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(hero_id, count)| ChatSummaryOutput {
                    hero_id: *hero_id,
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!("{:>8} {:>6}", "Hero ID".bold(), "Count".bold());
        println!("{}", "-".repeat(16));

        for (hero_id, count) in sorted.iter().take(limit) {
            println!("{:>8} {:>6}", hero_id, count);
        }

        println!(
            "\n{} chat messages ({} unique heroes){}",
            messages.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(messages.len());

        if json {
            let output: Vec<_> = messages.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:>8} {:<6} {}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Type".bold(),
            "Text".bold()
        );
        println!("{}", "-".repeat(70));

        for m in messages.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<6} {}",
                m.tick, m.hero_id, m.chat_type, m.text.green()
            );
        }

        println!(
            "\n{} chat messages{}",
            messages.len(),
            if limit < messages.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
