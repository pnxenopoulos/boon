use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

#[derive(Serialize)]
struct AbilityUpgradeOutput {
    tick: i32,
    hero_id: i64,
    ability_id: u32,
    ability: String,
    tier: i32,
}

#[derive(Serialize)]
struct AbilityUpgradeSummaryOutput {
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

    let class_filter: HashSet<&str> = ["CCitadelPlayerController"].into_iter().collect();

    let mut keys_resolved = false;
    let mut ck_hero_id: Option<u64> = None;
    // For indices 0..7: (item_id_key, upgrade_bits_key)
    let mut slot_keys: Vec<(Option<u64>, Option<u64>)> = Vec::new();

    // Change detection: (controller_entity_index, slot_index) → previous upgrade_bits
    let mut prev_bits: HashMap<(i32, usize), i32> = HashMap::new();

    let mut upgrades: Vec<AbilityUpgradeOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            // Resolve field keys once
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerController") {
                    ck_hero_id = s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                    for i in 0..8 {
                        let item_key = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_ItemID"
                        ));
                        let bits_key = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_nUpgradeInfo"
                        ));
                        slot_keys.push((item_key, bits_key));
                    }
                }
                keys_resolved = true;
            }

            for (&idx, entity) in ctx.entities.iter() {
                if entity.class_name != "CCitadelPlayerController" {
                    continue;
                }

                let hero_id = ck_hero_id
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon::FieldValue::U32(n) => Some(*n as i64),
                        boon::FieldValue::U64(n) => Some(*n as i64),
                        boon::FieldValue::I32(n) => Some(*n as i64),
                        boon::FieldValue::I64(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);

                if hero_id == 0 {
                    continue;
                }

                for (slot_idx, (item_key, bits_key)) in slot_keys.iter().enumerate() {
                    let ability_id = item_key
                        .and_then(|k| entity.fields.get(&k))
                        .and_then(|v| match v {
                            boon::FieldValue::U32(n) => Some(*n),
                            boon::FieldValue::U64(n) => Some(*n as u32),
                            boon::FieldValue::I32(n) => Some(*n as u32),
                            boon::FieldValue::I64(n) => Some(*n as u32),
                            _ => None,
                        })
                        .unwrap_or(0);

                    if ability_id == 0 {
                        continue;
                    }

                    // m_nUpgradeInfo packs upgrade bits in bits 17+
                    let upgrade_bits = bits_key
                        .and_then(|k| entity.fields.get(&k))
                        .and_then(|v| match v {
                            boon::FieldValue::I32(n) => Some(*n >> 17),
                            boon::FieldValue::I64(n) => Some((*n >> 17) as i32),
                            boon::FieldValue::U32(n) => Some((*n >> 17) as i32),
                            boon::FieldValue::U64(n) => Some((*n >> 17) as i32),
                            _ => None,
                        })
                        .unwrap_or(0);

                    let key = (idx, slot_idx);
                    let prev = prev_bits.get(&key).copied().unwrap_or(0);
                    if upgrade_bits != prev {
                        prev_bits.insert(key, upgrade_bits);
                        // Only emit when bits increased (actual upgrade, not reset)
                        if upgrade_bits > prev {
                            upgrades.push(AbilityUpgradeOutput {
                                tick: ctx.tick,
                                hero_id,
                                ability_id,
                                ability: boon::ability_name(ability_id).to_string(),
                                tier: upgrade_bits.count_ones() as i32 - 1,
                            });
                        }
                    }
                }
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        upgrades.retain(|a| a.ability.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        upgrades.retain(|a| a.tick >= min);
    }
    if let Some(max) = max_tick {
        upgrades.retain(|a| a.tick <= max);
    }

    if summary {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for a in &upgrades {
            *counts.entry(a.ability.as_str()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<AbilityUpgradeSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(ability, count)| AbilityUpgradeSummaryOutput {
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
            "\n{} ability upgrade events ({} unique abilities){}",
            upgrades.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(upgrades.len());

        if json {
            let output: Vec<_> = upgrades.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:<8} {:>8} {:<12} {:<40} {}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Ability ID".bold(),
            "Ability".bold(),
            "Bits".bold()
        );
        println!("{}", "-".repeat(80));

        for a in upgrades.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<12} {:<40} {}",
                a.tick,
                a.hero_id,
                a.ability_id,
                a.ability.green(),
                a.tier
            );
        }

        println!(
            "\n{} ability upgrade events{}",
            upgrades.len(),
            if limit < upgrades.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
