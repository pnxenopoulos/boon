use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use prost::Message;

use boon_proto::proto::{
    CCitadelUserMsgPostMatchDetails, CMsgMatchMetaDataContents,
    c_msg_match_meta_data_contents::{self, MatchInfo},
};

pub fn run(file: &Path, json: bool) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let events = parser.events(None)?;

    let event = events
        .iter()
        .find(|e| e.msg_type == 316)
        .context("no PostMatchDetails event (msg_type 316) found in demo")?;

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

    print_match_overview(info);
    print_players(info);
    print_objectives(info);
    print_mid_bosses(info);
    print_damage_matrix(info);

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

fn gold_source_name(source: i32) -> &'static str {
    match source {
        1 => "Players",
        2 => "LaneCreeps",
        3 => "Neutrals",
        4 => "Bosses",
        5 => "Treasure",
        6 => "Assists",
        7 => "Denies",
        8 => "TeamBonus",
        9 => "Assassinate",
        10 => "TrophyCollector",
        11 => "CultistSacrifice",
        12 => "Breakable",
        _ => "Unknown",
    }
}

fn print_players(info: &MatchInfo) {
    if info.players.is_empty() {
        return;
    }

    println!("{}", "Players".green().bold());
    println!(
        "  {:<4} {:<6} {:<8} {:<5} {:<6} {:<7} {:<10} {:<4} {:<6} {:<5} {:<4}",
        "Slot".bold(),
        "Team".bold(),
        "Hero".bold(),
        "K".bold(),
        "D".bold(),
        "A".bold(),
        "NetWorth".bold(),
        "LH".bold(),
        "Denies".bold(),
        "Level".bold(),
        "Lane".bold(),
    );
    println!("  {}", "-".repeat(75));

    for p in &info.players {
        println!(
            "  {:<4} {:<6} {:<8} {:<5} {:<6} {:<7} {:<10} {:<4} {:<6} {:<5} {:<4}",
            p.player_slot.unwrap_or(0),
            team_name(p.team.unwrap_or(0)),
            p.hero_id.unwrap_or(0),
            p.kills.unwrap_or(0),
            p.deaths.unwrap_or(0),
            p.assists.unwrap_or(0),
            p.net_worth.unwrap_or(0),
            p.last_hits.unwrap_or(0),
            p.denies.unwrap_or(0),
            p.level.unwrap_or(0),
            p.assigned_lane.unwrap_or(0),
        );

        println!(
            "    items: {}, deaths: {}, stat snapshots: {}",
            p.items.len(),
            p.death_details.len(),
            p.stats.len(),
        );

        if let Some(final_stats) = p.stats.last() {
            print_final_stats(final_stats);
        }

        println!();
    }
}

fn print_final_stats(s: &c_msg_match_meta_data_contents::PlayerStats) {
    println!(
        "    {} creep_kills={} neutral_kills={} player_damage={} denies={}",
        "final stats:".dimmed(),
        s.creep_kills.unwrap_or(0),
        s.neutral_kills.unwrap_or(0),
        s.player_damage.unwrap_or(0),
        s.denies.unwrap_or(0),
    );

    let gold_fields = [
        ("player", s.gold_player),
        ("player_orbs", s.gold_player_orbs),
        ("lane_creep", s.gold_lane_creep),
        ("lane_creep_orbs", s.gold_lane_creep_orbs),
        ("neutral_creep", s.gold_neutral_creep),
        ("neutral_creep_orbs", s.gold_neutral_creep_orbs),
        ("boss", s.gold_boss),
        ("boss_orb", s.gold_boss_orb),
        ("treasure", s.gold_treasure),
        ("denied", s.gold_denied),
        ("death_loss", s.gold_death_loss),
    ];

    let parts: Vec<String> = gold_fields
        .iter()
        .filter_map(|(name, val)| val.map(|v| format!("{}={}", name, v)))
        .collect();

    if !parts.is_empty() {
        println!("    {} {}", "gold breakdown:".dimmed(), parts.join(", "));
    }

    if !s.gold_sources.is_empty() {
        let src_parts: Vec<String> = s
            .gold_sources
            .iter()
            .map(|gs| {
                format!(
                    "{}(gold={}, orbs={})",
                    gold_source_name(gs.source.unwrap_or(0)),
                    gs.gold.unwrap_or(0),
                    gs.gold_orbs.unwrap_or(0),
                )
            })
            .collect();
        println!("    {} {}", "gold sources:".dimmed(), src_parts.join(", "));
    }
}

fn print_objectives(info: &MatchInfo) {
    if info.objectives.is_empty() {
        return;
    }

    println!("{}", "Objectives".green().bold());
    println!(
        "  {:<25} {:<6} {:<12} {:<12} {:<12}",
        "Objective".bold(),
        "Team".bold(),
        "Destroyed".bold(),
        "CreepDmg".bold(),
        "PlayerDmg".bold(),
    );
    println!("  {}", "-".repeat(70));

    for obj in &info.objectives {
        let obj_id = obj
            .team_objective_id
            .unwrap_or(obj.legacy_objective_id.unwrap_or(0));
        let time = obj
            .destroyed_time_s
            .map(fmt_duration)
            .unwrap_or_else(|| "-".to_string());

        println!(
            "  {:<25} {:<6} {:<12} {:<12} {:<12}",
            obj_id,
            team_name(obj.team.unwrap_or(0)),
            time,
            obj.creep_damage.unwrap_or(0),
            obj.player_damage.unwrap_or(0),
        );
    }
    println!();
}

fn print_mid_bosses(info: &MatchInfo) {
    if info.mid_boss.is_empty() {
        return;
    }

    println!("{}", "Mid Bosses".green().bold());
    println!(
        "  {:<12} {:<12} {:<12}",
        "Killed By".bold(),
        "Claimed By".bold(),
        "Time".bold(),
    );
    println!("  {}", "-".repeat(38));

    for mb in &info.mid_boss {
        let time = mb
            .destroyed_time_s
            .map(fmt_duration)
            .unwrap_or_else(|| "-".to_string());

        println!(
            "  {:<12} {:<12} {:<12}",
            team_name(mb.team_killed.unwrap_or(0)),
            team_name(mb.team_claimed.unwrap_or(0)),
            time,
        );
    }
    println!();
}

fn print_damage_matrix(info: &MatchInfo) {
    let matrix = match info.damage_matrix.as_ref() {
        Some(m) => m,
        None => return,
    };

    println!("{}", "Damage Matrix".green().bold());
    println!("  dealers:      {}", matrix.damage_dealers.len());
    println!("  time samples: {}", matrix.sample_time_s.len());

    if let Some(details) = &matrix.source_details {
        println!("  source names: {}", details.source_name.len());
        println!("  stat types:   {}", details.stat_type.len());
    }
    println!();
}
