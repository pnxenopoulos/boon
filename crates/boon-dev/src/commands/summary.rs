use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;

use boon_proto::proto::{
    CCitadelUserMsgPostMatchDetails, CMsgMatchMetaDataContents, CitadelUserMessageIds as Msg,
    c_msg_match_meta_data_contents::MatchInfo,
};

/// Print a post-match summary: overview, timing, the final per-player snapshot,
/// and objectives. Pass `json` to dump the full decoded metadata instead.
pub fn run(file: &Path, json: bool) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let events = parser.events(None)?;

    let event = events
        .iter()
        .find(|e| e.msg_type == Msg::KEUserMsgPostMatchDetails as u32)
        .context("no PostMatchDetails event found in demo")?;

    let outer = CCitadelUserMsgPostMatchDetails::decode(event.payload.as_slice())
        .context("failed to decode CCitadelUserMsgPostMatchDetails")?;

    let details_bytes = outer
        .match_details
        .as_ref()
        .context("PostMatchDetails has no match_details bytes")?;

    let contents = CMsgMatchMetaDataContents::decode(details_bytes.as_slice())
        .context("failed to decode CMsgMatchMetaDataContents")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&contents)?);
        return Ok(());
    }

    let info = contents
        .match_info
        .as_ref()
        .context("CMsgMatchMetaDataContents has no match_info")?;

    // Timing comes from the demo file itself, not the post-match metadata.
    let file_info = parser.file_info()?;
    let total_ticks = file_info.playback_ticks.unwrap_or(0);
    let playback_time = file_info.playback_time.unwrap_or(0.0);
    let game_over_tick = events
        .iter()
        .filter(|e| e.msg_type == Msg::KEUserMsgGameOver as u32)
        .map(|e| e.tick)
        .next_back();

    print_match_overview(info);
    print_timing(total_ticks, playback_time, game_over_tick, info);
    print_players(info);
    print_objectives(info);

    Ok(())
}

fn fmt_duration(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn team_name(team: i32) -> &'static str {
    match team {
        0 => "Team0",
        1 => "Team1",
        2 => "Spectator",
        _ => "Unknown",
    }
}

fn print_match_overview(info: &MatchInfo) {
    println!("{}", "Match Overview".green().bold());

    if let Some(id) = info.match_id {
        println!("  match_id:       {}", id);
    }
    if let Some(d) = info.duration_s {
        println!("  duration:       {} ({}s)", fmt_duration(d), d);
    }
    if let Some(v) = info.match_outcome {
        let label = match v {
            0 => "TeamWin",
            1 => "Error",
            2 => "MatchDraw",
            _ => "Unknown",
        };
        println!("  match_outcome:  {} ({})", label, v);
    }
    if let Some(v) = info.winning_team {
        println!("  winning_team:   {}", team_name(v));
    }
    if let Some(v) = info.game_mode {
        println!("  game_mode:      {}", v);
    }
    if let Some(v) = info.match_mode {
        println!("  match_mode:     {}", v);
    }
    if !info.team_score.is_empty() {
        println!(
            "  team_score:     {}",
            info.team_score
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(" / ")
        );
    }
    println!();
}

/// Print timing from the demo file (the full recording) alongside the
/// regulation (gameplay) duration from the post-match metadata.
fn print_timing(
    total_ticks: i32,
    playback_time: f32,
    game_over_tick: Option<i32>,
    info: &MatchInfo,
) {
    let tick_rate = if playback_time > 0.0 {
        (total_ticks as f32 / playback_time).round() as i32
    } else {
        0
    };

    println!("{}", "Timing".green().bold());
    println!("  total_ticks:      {}", total_ticks);
    println!(
        "  total_time:       {} ({:.0}s, full recording)",
        fmt_duration(playback_time as u32),
        playback_time
    );
    println!("  tick_rate:        {}", tick_rate);
    if let Some(t) = game_over_tick {
        println!("  game_over_tick:   {}", t);
    }
    if let Some(d) = info.duration_s {
        println!("  regulation_time:  {} ({}s, gameplay)", fmt_duration(d), d);
    }
    println!();
}

/// Print each player's final snapshot (the last `PlayerStats`), plus the
/// scoreboard last-hit total (`LH`) which is only recorded per match.
fn print_players(info: &MatchInfo) {
    if info.players.is_empty() {
        return;
    }

    let header = format!(
        "  {:<4} {:<6} {:<5} {:<4} {:<4} {:<4} {:<9} {:<5} {:<4} {:<4} {:<5} {:<7} {:<6} {:<9}",
        "Slot",
        "Team",
        "Hero",
        "K",
        "D",
        "A",
        "NetWorth",
        "LH",
        "Den",
        "Lvl",
        "Lane",
        "CreepK",
        "NeutK",
        "PlayerDmg",
    );
    println!("{}", "Players (final snapshot)".green().bold());
    println!("{}", header.bold());
    println!("  {}", "-".repeat(header.len()));

    for p in &info.players {
        let s = p.stats.last();
        println!(
            "  {:<4} {:<6} {:<5} {:<4} {:<4} {:<4} {:<9} {:<5} {:<4} {:<4} {:<5} {:<7} {:<6} {:<9}",
            p.player_slot.unwrap_or(0),
            team_name(p.team.unwrap_or(0)),
            p.hero_id.unwrap_or(0),
            s.map(|x| x.kills()).unwrap_or(0),
            s.map(|x| x.deaths()).unwrap_or(0),
            s.map(|x| x.assists()).unwrap_or(0),
            s.map(|x| x.net_worth()).unwrap_or(0),
            p.last_hits.unwrap_or(0),
            s.map(|x| x.denies()).unwrap_or(0),
            s.map(|x| x.level()).unwrap_or(0),
            p.assigned_lane.unwrap_or(0),
            s.map(|x| x.creep_kills()).unwrap_or(0),
            s.map(|x| x.neutral_kills()).unwrap_or(0),
            s.map(|x| x.player_damage()).unwrap_or(0),
        );
    }
    println!();
}

/// Print post-match objective records, mirroring the `objectives` DataFrame
/// from `Demo.summary()` in the Python bindings.
fn print_objectives(info: &MatchInfo) {
    if info.objectives.is_empty() {
        return;
    }

    let header = format!(
        "  {:<6} {:<6} {:<10} {:<10} {:<10} {:<10} {:<10}",
        "ObjID", "Team", "Destroyed", "FirstDmg", "CreepDmg", "PlayerDmg", "SpiritDmg",
    );
    println!("{}", "Objectives".green().bold());
    println!("{}", header.bold());
    println!("  {}", "-".repeat(header.len()));

    for obj in &info.objectives {
        let destroyed = obj
            .destroyed_time_s
            .map(fmt_duration)
            .unwrap_or_else(|| "-".to_string());
        let first_dmg = obj
            .first_damage_time_s
            .map(fmt_duration)
            .unwrap_or_else(|| "-".to_string());

        println!(
            "  {:<6} {:<6} {:<10} {:<10} {:<10} {:<10} {:<10}",
            obj.team_objective_id() as i32,
            team_name(obj.team.unwrap_or(0)),
            destroyed,
            first_dmg,
            obj.creep_damage.unwrap_or(0),
            obj.player_damage.unwrap_or(0),
            obj.player_spirit_damage.unwrap_or(0),
        );
    }
    println!();
}
