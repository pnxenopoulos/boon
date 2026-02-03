use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

pub fn run(file: &PathBuf) -> Result<()> {
    let parser = boon::Parser::from_file(file)?;

    // File header
    let header = parser.file_header()?;
    println!("{}", "File Header".green().bold());
    println!("  demo_file_stamp:  {}", header.demo_file_stamp);
    if let Some(v) = header.patch_version {
        println!("  patch_version:    {}", v);
    }
    if let Some(ref v) = header.server_name {
        println!("  server_name:      {}", v);
    }
    if let Some(ref v) = header.client_name {
        println!("  client_name:      {}", v);
    }
    if let Some(ref v) = header.map_name {
        println!("  map_name:         {}", v);
    }
    if let Some(ref v) = header.game_directory {
        println!("  game_directory:   {}", v);
    }
    if let Some(ref v) = header.demo_version_name {
        println!("  demo_version:     {}", v);
    }
    if let Some(ref v) = header.demo_version_guid {
        println!("  demo_guid:        {}", v);
    }
    if let Some(v) = header.build_num {
        println!("  build_num:        {}", v);
    }
    if let Some(ref v) = header.game {
        println!("  game:             {}", v);
    }
    if let Some(v) = header.server_start_tick {
        println!("  server_start:     {}", v);
    }

    // File info
    println!();
    match parser.file_info() {
        Ok(info) => {
            println!("{}", "File Info".green().bold());
            if let Some(v) = info.playback_time {
                let minutes = v as u32 / 60;
                let seconds = v as u32 % 60;
                println!("  playback_time:    {:.1}s ({}:{:02})", v, minutes, seconds);
            }
            if let Some(v) = info.playback_ticks {
                println!("  playback_ticks:   {}", v);
            }
            if let Some(v) = info.playback_frames {
                println!("  playback_frames:  {}", v);
            }
            if let Some(ref gi) = info.game_info {
                if let Some(ref dota_info) = gi.dota {
                    if let Some(v) = dota_info.match_id {
                        println!("  match_id:         {}", v);
                    }
                    if let Some(v) = dota_info.game_mode {
                        println!("  game_mode:        {}", v);
                    }
                    if let Some(v) = dota_info.game_winner {
                        println!("  game_winner:      {}", v);
                    }
                    for player in &dota_info.player_info {
                        if let Some(ref name) = player.player_name {
                            let hero = player.hero_name.as_deref().unwrap_or("?");
                            println!("  player:           {} ({})", name, hero);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("{}: {}", "File Info".yellow().bold(), e);
        }
    }

    Ok(())
}
