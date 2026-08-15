use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

// eValType (the game's `EModifierValue` enum) → stat name mapping.
//
// EModifierValue's numeric values are reassigned between game builds as new
// entries are inserted, so each permanent-stat pickup is identified by more
// than one value across the builds boon supports. Values confirmed from demos:
//   * build <= 10725: health 31, spirit 51, fire_rate 79, weapon 18,
//                     cooldown 109, ammo 172
//   * build 10854:    health 43, spirit 158, fire_rate 91, weapon 19,
//                     cooldown 98, ammo 63
// A pickup only ever populates one of these six stats and the value sets are
// disjoint, so recognising every value is unambiguous.
const EVAL_HEALTH: &[u32] = &[31, 43];
const EVAL_SPIRIT_POWER: &[u32] = &[51, 158];
const EVAL_FIRE_RATE: &[u32] = &[79, 91];
const EVAL_WEAPON_DAMAGE: &[u32] = &[18, 19];
const EVAL_COOLDOWN_REDUCTION: &[u32] = &[109, 98];
const EVAL_AMMO: &[u32] = &[172, 63];

/// Index into the 6-stat array by eValType
fn stat_index(val_type: u32) -> Option<usize> {
    if EVAL_HEALTH.contains(&val_type) {
        Some(0)
    } else if EVAL_SPIRIT_POWER.contains(&val_type) {
        Some(1)
    } else if EVAL_FIRE_RATE.contains(&val_type) {
        Some(2)
    } else if EVAL_WEAPON_DAMAGE.contains(&val_type) {
        Some(3)
    } else if EVAL_COOLDOWN_REDUCTION.contains(&val_type) {
        Some(4)
    } else if EVAL_AMMO.contains(&val_type) {
        Some(5)
    } else {
        None
    }
}

#[derive(Serialize)]
struct StatModifierOutput {
    tick: i32,
    hero_id: i64,
    stat: String,
    value: f32,
}

#[derive(Serialize)]
struct StatModifierSummary {
    hero_id: i64,
    health: f32,
    spirit_power: f32,
    fire_rate: f32,
    weapon_damage: f32,
    cooldown_reduction: f32,
    ammo: f32,
}

const STAT_NAMES: [&str; 6] = [
    "health",
    "spirit_power",
    "fire_rate",
    "weapon_damage",
    "cooldown_reduction",
    "ammo",
];

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
    // StatViewerModifierValues keys for indices 0..20: (modifier_id, val_type, value)
    let mut sv_keys: Vec<(Option<u64>, Option<u64>, Option<u64>)> = Vec::new();

    // Previous state per hero: [health, spirit_power, fire_rate, weapon_damage, cooldown_reduction, ammo]
    let mut prev_state: HashMap<i64, [f32; 6]> = HashMap::new();
    let mut events_out: Vec<StatModifierOutput> = Vec::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved {
                if let Some(s) = ctx.serializers().get("CCitadelPlayerController") {
                    ck_hero_id = s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                    for i in 0..20 {
                        let mid = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_SourceModifierID"
                        ));
                        let vt = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_eValType"
                        ));
                        let val = s.resolve_field_key(&format!(
                            "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_flValue"
                        ));
                        sv_keys.push((mid, vt, val));
                    }
                }
                keys_resolved = true;
            }

            for (_, entity) in ctx.entities().iter() {
                if entity.class_name.as_ref() != "CCitadelPlayerController" {
                    continue;
                }
                let hero_id = entity.get_i64(ck_hero_id);
                if hero_id == 0 {
                    continue;
                }

                // Sum values by eValType
                let mut sums = [0.0f32; 6];
                for (mid_key, vt_key, val_key) in &sv_keys {
                    let mid = entity.get_u32(*mid_key);
                    let vt = entity.get_u32(*vt_key);
                    let val = entity.get_f32(*val_key);
                    if mid == 0 && vt == 0 && val == 0.0 {
                        continue;
                    }
                    if let Some(idx) = stat_index(vt) {
                        sums[idx] += val;
                    }
                }

                // Compare to previous state, emit changes
                let prev = prev_state.entry(hero_id).or_insert([0.0f32; 6]);
                for i in 0..6 {
                    if sums[i] != prev[i] && sums[i] > prev[i] {
                        events_out.push(StatModifierOutput {
                            tick: ctx.tick(),
                            hero_id,
                            stat: STAT_NAMES[i].to_string(),
                            value: sums[i],
                        });
                    }
                }
                *prev = sums;
            }
        })
        .with_context(|| "failed to parse demo")?;

    // Apply filter
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events_out.retain(|e| e.stat.to_lowercase().contains(&f_lower));
    }
    if let Some(min) = min_tick {
        events_out.retain(|e| e.tick >= min);
    }
    if let Some(max) = max_tick {
        events_out.retain(|e| e.tick <= max);
    }

    if summary {
        // Build per-hero final values from prev_state
        let mut summaries: Vec<StatModifierSummary> = prev_state
            .iter()
            .map(|(&hero_id, sums)| StatModifierSummary {
                hero_id,
                health: sums[0],
                spirit_power: sums[1],
                fire_rate: sums[2],
                weapon_damage: sums[3],
                cooldown_reduction: sums[4],
                ammo: sums[5],
            })
            .collect();
        summaries.sort_by_key(|s| s.hero_id);

        let limit = limit.unwrap_or(summaries.len());

        if json {
            let output: Vec<_> = summaries.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!(
            "{:>8} {:>8} {:>14} {:>10} {:>15} {:>18} {:>6}",
            "Hero ID".bold(),
            "Health".bold(),
            "Spirit Power".bold(),
            "Fire Rate".bold(),
            "Weapon Damage".bold(),
            "Cooldown Reduction".bold(),
            "Ammo".bold()
        );
        println!("{}", "-".repeat(80));

        for s in summaries.iter().take(limit) {
            println!(
                "{:>8} {:>8.1} {:>14.1} {:>10.3} {:>15.3} {:>18.3} {:>6.3}",
                s.hero_id,
                s.health,
                s.spirit_power,
                s.fire_rate,
                s.weapon_damage,
                s.cooldown_reduction,
                s.ammo
            );
        }

        println!(
            "\n{} heroes{}",
            summaries.len(),
            if limit < summaries.len() {
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
            "{:<8} {:>8} {:<20} {:>10}",
            "Tick".bold(),
            "Hero ID".bold(),
            "Stat".bold(),
            "Value".bold()
        );
        println!("{}", "-".repeat(50));

        for e in events_out.iter().take(limit) {
            println!(
                "{:<8} {:>8} {:<20} {:>10.3}",
                e.tick,
                e.hero_id,
                e.stat.green(),
                e.value
            );
        }

        println!(
            "\n{} stat modifier changes{}",
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

#[cfg(test)]
mod tests {
    use super::{STAT_NAMES, stat_index};

    #[test]
    fn maps_pre_10854_evaltypes() {
        // EModifierValue values used up to build 10725.
        assert_eq!(stat_index(31), Some(0)); // health
        assert_eq!(stat_index(51), Some(1)); // spirit_power
        assert_eq!(stat_index(79), Some(2)); // fire_rate
        assert_eq!(stat_index(18), Some(3)); // weapon_damage
        assert_eq!(stat_index(109), Some(4)); // cooldown_reduction
        assert_eq!(stat_index(172), Some(5)); // ammo
    }

    #[test]
    fn maps_build_10854_evaltypes() {
        // EModifierValue was renumbered in build 10854. Before this fix these
        // values were unmapped, so stat_modifier_events was silently empty.
        assert_eq!(stat_index(43), Some(0)); // MODIFIER_VALUE_HEALTH_MAX
        assert_eq!(stat_index(158), Some(1)); // MODIFIER_VALUE_TECH_POWER
        assert_eq!(stat_index(91), Some(2)); // MODIFIER_VALUE_FIRE_RATE
        assert_eq!(stat_index(19), Some(3)); // MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE
        assert_eq!(stat_index(98), Some(4)); // MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE
        assert_eq!(stat_index(63), Some(5)); // MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT
    }

    #[test]
    fn ammo_pickup_resolves_to_ammo_stat() {
        // Regression for demo 99471204 (build 10854): the golden-statue +5% max
        // ammo pickup carries eValType 63, which must resolve to the ammo stat.
        assert_eq!(stat_index(63), Some(5));
        assert_eq!(STAT_NAMES[5], "ammo");
    }

    #[test]
    fn unknown_evaltype_is_ignored() {
        assert_eq!(stat_index(0), None);
        assert_eq!(stat_index(255), None);
    }
}
