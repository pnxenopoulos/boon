use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;
use serde::Serialize;

struct CachedModifier {
    hero_id: i64,
    modifier: String,
    ability: String,
    duration: f32,
    caster_hero_id: i64,
    stacks: i32,
}

#[derive(Serialize)]
struct ActiveModifierOutput {
    tick: i32,
    hero_id: i64,
    event: String,
    modifier: String,
    ability: String,
    duration: f32,
    caster_hero_id: i64,
    stacks: i32,
}

#[derive(Serialize)]
struct ActiveModifierSummary {
    hero_id: i64,
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
    let parser =
        boon::Parser::from_file(file).with_context(|| format!("failed to open {}", file.display()))?;

    let class_filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();

    let mut keys_resolved = false;
    let mut pk_hero_id: Option<u64> = None;
    let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
    let mut entity_to_hero_built = false;

    // Track active modifiers by serial_number
    let mut prev_modifiers: HashMap<u32, CachedModifier> = HashMap::new();
    let mut events_out: Vec<ActiveModifierOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            // Resolve pawn hero_id key once (retry until serializers available)
            if !keys_resolved
                && let Some(s) = ctx.serializers.get("CCitadelPlayerPawn")
            {
                pk_hero_id = s.resolve_field_key(
                    "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID",
                );
                keys_resolved = true;
            }

            // Build entity_to_hero map (retry until populated)
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
                if !entity_to_hero.is_empty() {
                    entity_to_hero_built = true;
                }
            }

            // Scan ActiveModifiers string table
            if let Some(table) = ctx.string_tables.find_table("ActiveModifiers") {
                let mut current_serials: HashSet<u32> = HashSet::new();

                for entry in &table.entries {
                    let data = match &entry.user_data {
                        Some(d) if !d.is_empty() => d,
                        _ => continue,
                    };

                    let Ok(modifier) =
                        boon_proto::proto::CModifierTableEntry::decode(data.as_slice())
                    else {
                        continue;
                    };

                    let serial = match modifier.serial_number {
                        Some(s) => s,
                        None => continue,
                    };

                    let parent_handle = modifier.parent.unwrap_or(16777215);
                    if parent_handle == 16777215 {
                        continue;
                    }
                    let parent_idx = (parent_handle & 0x3FFF) as i32;

                    // Only track modifiers on player pawns
                    let hero_id = match entity_to_hero.get(&parent_idx) {
                        Some(&hid) => hid,
                        None => continue,
                    };

                    let entry_type = modifier.entry_type.unwrap_or(1);

                    // entry_type == 2 means explicitly removed
                    if entry_type == 2 {
                        if let Some(cached) = prev_modifiers.remove(&serial) {
                            events_out.push(ActiveModifierOutput {
                                tick: ctx.tick,
                                hero_id: cached.hero_id,
                                event: "removed".to_string(),
                                modifier: cached.modifier,
                                ability: cached.ability,
                                duration: cached.duration,
                                caster_hero_id: cached.caster_hero_id,
                                stacks: cached.stacks,
                            });
                        }
                        continue;
                    }

                    current_serials.insert(serial);

                    // New modifier (not seen before)
                    if let std::collections::hash_map::Entry::Vacant(e) = prev_modifiers.entry(serial) {
                        let modifier_name = boon::modifier_name(
                            modifier.modifier_subclass.unwrap_or(0),
                        )
                        .to_string();
                        let ability_name = boon::ability_name(
                            modifier.ability_subclass.unwrap_or(0),
                        )
                        .to_string();
                        let duration = modifier.duration.unwrap_or(-1.0);
                        let caster_handle = modifier.caster.unwrap_or(16777215);
                        let caster_hero_id = if caster_handle != 16777215 {
                            let caster_idx = (caster_handle & 0x3FFF) as i32;
                            entity_to_hero.get(&caster_idx).copied().unwrap_or(0)
                        } else {
                            0
                        };
                        let stacks = modifier.stack_count.unwrap_or(0);

                        events_out.push(ActiveModifierOutput {
                            tick: ctx.tick,
                            hero_id,
                            event: "applied".to_string(),
                            modifier: modifier_name.clone(),
                            ability: ability_name.clone(),
                            duration,
                            caster_hero_id,
                            stacks,
                        });

                        e.insert(CachedModifier {
                            hero_id,
                            modifier: modifier_name,
                            ability: ability_name,
                            duration,
                            caster_hero_id,
                            stacks,
                        });
                    }
                }

                // Detect removed modifiers: serials in prev but not in current
                let removed: Vec<u32> = prev_modifiers
                    .keys()
                    .filter(|s| !current_serials.contains(s))
                    .copied()
                    .collect();
                for serial in removed {
                    if let Some(cached) = prev_modifiers.remove(&serial) {
                        events_out.push(ActiveModifierOutput {
                            tick: ctx.tick,
                            hero_id: cached.hero_id,
                            event: "removed".to_string(),
                            modifier: cached.modifier,
                            ability: cached.ability,
                            duration: cached.duration,
                            caster_hero_id: cached.caster_hero_id,
                            stacks: cached.stacks,
                        });
                    }
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filters
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events_out.retain(|e| {
            e.modifier.to_lowercase().contains(&f_lower)
                || e.ability.to_lowercase().contains(&f_lower)
        });
    }
    if let Some(min) = min_tick {
        events_out.retain(|e| e.tick >= min);
    }
    if let Some(max) = max_tick {
        events_out.retain(|e| e.tick <= max);
    }

    if summary {
        let mut counts: HashMap<(i64, &str), usize> = HashMap::new();
        for e in &events_out {
            if e.event == "applied" {
                *counts
                    .entry((e.hero_id, e.ability.as_str()))
                    .or_insert(0) += 1;
            }
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<ActiveModifierSummary> = sorted
                .iter()
                .take(limit)
                .map(|((hero_id, ability), count)| ActiveModifierSummary {
                    hero_id: *hero_id,
                    ability: ability.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:>8} {:<40} {:>6}",
            "Hero ID".bold(),
            "Ability".bold(),
            "Count".bold()
        );
        println!("{}", "-".repeat(56));

        for ((hero_id, ability), count) in sorted.iter().take(limit) {
            println!("{:>8} {:<40} {:>6}", hero_id, ability.green(), count);
        }

        println!(
            "\n{} applied events ({} unique hero+ability combos){}",
            events_out.iter().filter(|e| e.event == "applied").count(),
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
            "{:<8} {:>8} {:<10} {:<40} {:<30} {:>8} {:>6} {:>10}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Event".bold(),
            "Modifier".bold(),
            "Ability".bold(),
            "Duration".bold(),
            "Stacks".bold(),
            "Caster ID".bold()
        );
        println!("{}", "-".repeat(120));

        for e in events_out.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<10} {:<40} {:<30} {:>8.1} {:>6} {:>10}",
                e.tick,
                e.hero_id,
                if e.event == "applied" {
                    e.event.green().to_string()
                } else {
                    e.event.red().to_string()
                },
                e.modifier,
                e.ability,
                e.duration,
                e.stacks,
                e.caster_hero_id
            );
        }

        println!(
            "\n{} active modifier events{}",
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
