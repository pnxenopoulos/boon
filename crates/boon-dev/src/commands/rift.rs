use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

/// Known Rift ("Koth") cash-in sites, as `([x, y], lane)`.
///
/// The Rift entities carry no `m_iLane` field, so the lane comes from the
/// cash-in location. Each site was cross-checked against the lane of the buffed
/// trooper cohort that spawns for the winning team after a capture.
const RIFT_LANE_SITES: &[([f32; 2], i64)] = &[([-7560.0, 0.0], 1), ([7612.0, 0.0], 6)];

/// Match radius, in Hammer units, for associating a location with a known site.
const RIFT_LANE_TOLERANCE: f32 = 1024.0;

/// Upper bound, in Hammer units, on a plausible map coordinate.
///
/// The game clears `m_vKothCashInCurrentLocation` to `FLT_MAX` rather than to
/// zero once a Rift resolves, and `FLT_MAX` is finite — so an `is_finite` check
/// does not reject it, but this bound does.
const RIFT_COORD_SANITY: f32 = 1.0e6;

/// The lane for a Rift cash-in location, or `0` when it is not a known site.
fn rift_lane_for(x: f32, y: f32) -> i64 {
    if !x.is_finite() || !y.is_finite() {
        return 0;
    }
    for ([sx, sy], lane) in RIFT_LANE_SITES {
        if (x - sx).abs() <= RIFT_LANE_TOLERANCE && (y - sy).abs() <= RIFT_LANE_TOLERANCE {
            return *lane;
        }
    }
    0
}

/// One completed Rift.
#[derive(Serialize)]
struct RiftOutput {
    rift_num: i32,
    announce_tick: Option<i32>,
    active_tick: i32,
    capture_tick: Option<i32>,
    expire_tick: Option<i32>,
    winning_team: Option<i32>,
    lane: i64,
    x: f32,
    y: f32,
    z: f32,
}

/// List Rift (Koth) lifecycle rows — one per Rift.
pub fn run(
    file: &Path,
    summary: bool,
    limit: Option<usize>,
    min_tick: Option<i32>,
    max_tick: Option<i32>,
    json: bool,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;

    let class_filter: HashSet<&str> = ["CCitadelGameRulesProxy", "CCitadelItemKothSpawner"]
        .into_iter()
        .collect();

    let mut keys_resolved = false;
    let mut rk_cashin_started: Option<u64> = None;
    let mut rk_scoring_team: Option<u64> = None;
    let mut rk_location: Option<u64> = None;

    let mut rows: Vec<RiftOutput> = Vec::new();
    let mut counter: i32 = 0;
    let mut live = false;
    let mut pending_announce: Option<i32> = None;
    let mut cur_announce: Option<i32> = None;
    let mut cur_active_tick: i32 = 0;
    let mut cur_capture_tick: Option<i32> = None;
    let mut cur_winning_team: Option<i32> = None;
    let mut cur_loc: [f32; 3] = [0.0; 3];
    let mut seen_contested = false;
    let mut spawners_prev: HashSet<i32> = HashSet::new();
    let mut spawners_cur: HashSet<i32> = HashSet::new();

    parser
        .run_to_end_filtered(&class_filter, |ctx| {
            if !keys_resolved && let Some(s) = ctx.serializers().get("CCitadelGameRulesProxy") {
                rk_cashin_started = s.resolve_field_key("m_pGameRules.m_timeKothCashInStarted");
                rk_scoring_team = s.resolve_field_key("m_pGameRules.m_nKothScoringTeam");
                rk_location = s.resolve_field_key("m_pGameRules.m_vKothCashInCurrentLocation");
                keys_resolved = true;
            }

            // A spawner absent last tick announces the next Rift. Entity indices
            // are recycled and m_flCreateTime is not transmitted, so
            // presence-diffing is the only reliable spawn signal.
            for (idx, entity) in ctx.entities().iter() {
                if entity.class_name.as_ref() == "CCitadelItemKothSpawner" {
                    spawners_cur.insert(idx);
                    if !spawners_prev.contains(&idx) && !live {
                        pending_announce = Some(ctx.tick());
                    }
                }
            }
            std::mem::swap(&mut spawners_prev, &mut spawners_cur);
            spawners_cur.clear();

            let Some((_, entity)) = ctx
                .entities()
                .iter()
                .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
            else {
                return;
            };

            // m_timeKothCashInStarted holds a real GameTime_t while a Rift is
            // contestable and 0 otherwise. It is re-armed mid-Rift (resetting the
            // give-up timer), so only the 0 -> non-zero edge marks the start.
            let cashin_started = entity.get_f32(rk_cashin_started);
            let is_live = cashin_started > 0.0 && cashin_started.is_finite();
            let scoring_team = entity.get_i64(rk_scoring_team) as i32;

            if is_live && !live {
                live = true;
                cur_announce = pending_announce.take();
                cur_active_tick = ctx.tick();
                cur_capture_tick = None;
                cur_winning_team = None;
                cur_loc = [0.0; 3];
                seen_contested = scoring_team <= 0;
            }

            if live {
                // Only read the location while the cash-in is still live: it is
                // cleared to FLT_MAX on the same tick the Rift resolves, which
                // would otherwise overwrite the real position.
                if is_live {
                    let loc = entity.get_vector3(rk_location);
                    if loc != [0.0; 3] && loc.iter().all(|c| c.abs() < RIFT_COORD_SANITY) {
                        cur_loc = loc;
                    }
                }
                // m_nKothScoringTeam keeps the previous Rift's winner until the
                // next one opens, so only count a positive value once this Rift
                // has been seen contested (-1).
                if scoring_team <= 0 {
                    seen_contested = true;
                } else if seen_contested && cur_capture_tick.is_none() {
                    cur_capture_tick = Some(ctx.tick());
                    cur_winning_team = Some(scoring_team);
                }
            }

            if !is_live && live {
                live = false;
                counter += 1;
                rows.push(RiftOutput {
                    rift_num: counter,
                    announce_tick: cur_announce,
                    active_tick: cur_active_tick,
                    capture_tick: cur_capture_tick,
                    // No winner by the time the Rift clears => it timed out.
                    expire_tick: if cur_capture_tick.is_none() {
                        Some(ctx.tick())
                    } else {
                        None
                    },
                    winning_team: cur_winning_team,
                    lane: rift_lane_for(cur_loc[0], cur_loc[1]),
                    x: cur_loc[0],
                    y: cur_loc[1],
                    z: cur_loc[2],
                });
            }
        })
        .with_context(|| "failed to parse demo")?;

    if let Some(min) = min_tick {
        rows.retain(|r| r.active_tick >= min);
    }
    if let Some(max) = max_tick {
        rows.retain(|r| r.active_tick <= max);
    }

    if summary {
        let captured = rows.iter().filter(|r| r.capture_tick.is_some()).count();
        let expired = rows.len() - captured;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "rifts": rows.len(),
                    "captured": captured,
                    "expired": expired,
                }))?
            );
            return Ok(());
        }
        println!("{:<12} {:>6}", "Outcome".bold(), "Count".bold());
        println!("{}", "-".repeat(20));
        println!("{:<12} {:>6}", "captured".green(), captured);
        println!("{:<12} {:>6}", "expired".yellow(), expired);
        println!("\n{} rifts", rows.len());
        return Ok(());
    }

    let limit = limit.unwrap_or(rows.len());

    if json {
        let output: Vec<_> = rows.iter().take(limit).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "{:>4} {:>9} {:>8} {:>9} {:>9} {:>7} {:>5} {:>10} {:>10} {:>8}",
        "#".bold(),
        "announce".bold(),
        "active".bold(),
        "capture".bold(),
        "expire".bold(),
        "winner".bold(),
        "lane".bold(),
        "x".bold(),
        "y".bold(),
        "z".bold()
    );
    println!("{}", "-".repeat(90));

    let dash = |v: Option<i32>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
    for r in rows.iter().take(limit) {
        println!(
            "{:>4} {:>9} {:>8} {:>9} {:>9} {:>7} {:>5} {:>10.1} {:>10.1} {:>8.1}",
            r.rift_num,
            dash(r.announce_tick),
            r.active_tick,
            dash(r.capture_tick).green(),
            dash(r.expire_tick).yellow(),
            dash(r.winning_team),
            r.lane,
            r.x,
            r.y,
            r.z
        );
    }

    println!(
        "\n{} rifts{}",
        rows.len(),
        if limit < rows.len() {
            format!(" (showing {limit})")
        } else {
            String::new()
        }
    );

    Ok(())
}
