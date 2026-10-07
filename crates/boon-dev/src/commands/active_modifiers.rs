use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

#[derive(Serialize)]
struct ActiveModifierOutput {
    tick: i32,
    hero_id: i64,
    event: String,
    serial: u32,
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
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let names = boon::CatalogNames::load(None)?;

    let directory = boon::data::catalog_dir(None)?;
    let catalog = boon::hero_stats::StatCatalog::from_directory(&directory)?;
    let mut state = boon::EffectiveModifierState::with_catalog(&catalog);
    let initial = parser.parse_init()?;
    let clock = boon::ModifierClock::resolve(&initial);
    state.rebuild(&initial, clock.game_time(&initial));
    let hero_key = initial
        .serializers()
        .get("CCitadelPlayerPawn")
        .and_then(|s| s.resolve_field_key("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID"));
    let mut class_filter: HashSet<&str> = [
        "CCitadelPlayerPawn",
        "CCitadelPlayerController",
        "CCitadelGameRulesProxy",
    ]
    .into_iter()
    .collect();
    class_filter.extend(
        initial
            .serializers()
            .iter()
            .map(|(name, _)| name)
            .filter(|name| name.contains("Ability")),
    );
    let mut identities = HashMap::new();
    let mut events_out = Vec::new();
    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            for change in state.update(ctx, clock.game_time(ctx)) {
                let modifier = change.entry;
                let hero = |handle: Option<u32>| {
                    handle
                        .and_then(|h| ctx.entities().get_by_handle(h))
                        .filter(|pawn| pawn.class_name.as_ref() == "CCitadelPlayerPawn")
                        .map(|pawn| pawn.get_i64(hero_key))
                        .filter(|&id| id != 0)
                };
                let identity = if change.kind == boon::ModifierChangeKind::Removed {
                    identities.remove(&change.serial)
                } else {
                    let identity = hero(modifier.parent)
                        .map(|id| (id, hero(modifier.caster).unwrap_or(0)))
                        .or_else(|| identities.get(&change.serial).copied());
                    if let Some(identity) = identity {
                        identities.insert(change.serial, identity);
                    }
                    identity
                };
                let Some((hero_id, caster_hero_id)) = identity else {
                    continue;
                };
                events_out.push(ActiveModifierOutput {
                    tick: ctx.tick(),
                    hero_id,
                    event: change.kind.as_str().into(),
                    serial: change.serial,
                    modifier: names
                        .modifier_name(modifier.modifier_subclass.unwrap_or(0))
                        .into(),
                    ability: names
                        .ability_name(modifier.ability_subclass.unwrap_or(0))
                        .into(),
                    duration: modifier.duration.unwrap_or(-1.0),
                    caster_hero_id,
                    stacks: modifier.stack_count.unwrap_or(0),
                });
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
                *counts.entry((e.hero_id, e.ability.as_str())).or_insert(0) += 1;
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
