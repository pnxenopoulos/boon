//! Match Info — prints metadata from a demo file header.
//!
//! Usage: cargo run -p boon-deadlock --example info -- <demo.dem>

use std::path::Path;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: info <demo.dem>");
    let parser = boon::Parser::from_file(Path::new(&path)).expect("failed to open demo");
    parser.verify().expect("invalid demo file");

    // File header: map name, build number, etc.
    let header = parser.file_header().expect("failed to read file header");
    println!("Demo:           {}", path);
    if let Some(ref map) = header.map_name {
        println!("Map:            {}", map);
    }
    if let Some(ref server) = header.server_name {
        println!("Server name:    {}", server);
    }
    if let Some(build) = header.build_num {
        println!("Build number:   {}", build);
    }
    if let Some(ref game_dir) = header.game_directory {
        println!("Game directory: {}", game_dir);
    }

    // File info: match duration, player names, etc.
    let info = parser.file_info().expect("failed to read file info");
    if let Some(v) = info.playback_time {
        let minutes = v as u32 / 60;
        let seconds = v as u32 % 60;
        println!("Playback time:  {:.1}s ({}:{:02})", v, minutes, seconds);
    }
    if let Some(v) = info.playback_ticks {
        println!("Playback ticks: {}", v);
    }

    if let Some(ref gi) = info.game_info {
        if let Some(ref dota) = gi.dota {
            if let Some(match_id) = dota.match_id {
                println!("Match ID:       {}", match_id);
            }
            if let Some(game_mode) = dota.game_mode {
                println!("Game mode:      {}", game_mode);
            }
            if let Some(winner) = dota.game_winner {
                println!("Game winner:    {}", winner);
            }

            if !dota.player_info.is_empty() {
                println!("\nPlayers ({}):", dota.player_info.len());
                for p in &dota.player_info {
                    let name = p.player_name.as_deref().unwrap_or("?");
                    let hero = p.hero_name.as_deref().unwrap_or("?");
                    let team = p.game_team.unwrap_or(-1);
                    println!("  [team {}] {} (hero: {})", team, name, hero);
                }
            }
        }
    }
}
