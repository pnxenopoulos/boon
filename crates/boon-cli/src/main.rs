use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use owo_colors::OwoColorize;

use boon::parser::Parser;
use boon_proto::generated as pb;

#[derive(ClapParser, Debug)]
#[command(
    name = "boon",
    version,
    about = "Deadlock demo file / replay utilities"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Verify and print header/info found in the prologue
    Check { file: PathBuf },

    /// Print framed messages or extracted packet events
    Debug {
        /// File: the demo to print
        file: PathBuf,

        /// Format: Uses CSV-styling
        #[arg(long)]
        csv: bool,

        /// Filter: start tick (inclusive)
        #[arg(long)]
        start_tick: Option<u32>,

        /// Filter: end tick (inclusive)
        #[arg(long)]
        end_tick: Option<u32>,

        /// Show framed messages (command, tick, size, compressed)
        #[arg(long, conflicts_with = "events")]
        messages: bool,

        /// Show (tick, EventName) extracted from packets
        #[arg(long, conflicts_with = "messages")]
        events: bool,
    },

    Kills { file: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Check { file } => cmd_check(file)?,
        Commands::Debug {
            file,
            csv,
            start_tick,
            end_tick,
            messages,
            events,
        } => cmd_debug(file, csv, start_tick, end_tick, messages, events)?,
        Commands::Kills { file } => cmd_kill(file)?,
    }
    Ok(())
}

fn cmd_check(path: PathBuf) -> Result<()> {
    println!("Reading {:#?}", path);
    let parser = Parser::new(&path)?;

    // Print demo file verification status
    if let Err(e) = parser.verify() {
        println!("Demo Verification: {}", "FAILURE".red());
        println!("{}", format!("Verification failed: {e}").red());
        return Ok(());
    } else {
        println!("Demo Verification: {}", "SUCCESS".green());
    }
    println!();

    // Parse to get the Demo Header and File Info
    let meta = parser.parse_metadata()?;

    if let Some(h) = meta.header {
        println!("Server Name : {}", h.server_name.unwrap());
        println!("Client Name : {}", h.client_name.unwrap());
        println!("Map Name    : {}", h.map_name.unwrap());
        println!("Build Num   : {}", h.build_num.unwrap());
    } else {
        println!("Error: {}", "No CDemoFileHeader found.".red());
    }

    if let Some(f) = meta.info {
        println!("Playback Time (s) : {}", f.playback_time.unwrap());
        println!("Playback Ticks    : {}", f.playback_ticks.unwrap());
        println!("Playback Frames   : {}", f.playback_frames.unwrap());
    } else {
        println!("Error: {}", "No CDemoFileInfo found.".red());
    }

    Ok(())
}

fn cmd_debug(
    path: PathBuf,
    csv: bool,
    start_tick: Option<u32>,
    end_tick: Option<u32>,
    messages: bool,
    events: bool,
) -> Result<()> {
    let parser = Parser::new(&path)?;
    parser.verify()?;

    // default to messages if neither flag is provided
    if messages || !events {
        // Scan and print framed messages
        let entries = parser.scan_messages()?;

        if csv {
            println!("idx,command,tick,size,compressed");
        }

        for (idx, (cmd, tick, size, compressed)) in entries.into_iter().enumerate() {
            // Filter by tick range, if provided
            if let Some(start) = start_tick
                && tick < start as i32
            {
                continue;
            }
            if let Some(end) = end_tick
                && tick > end as i32
            {
                continue;
            }

            // Get the command name from the protos
            let cmd_name = pb::EDemoCommands::try_from(cmd)
                .map(|k| format!("{k:?}"))
                .unwrap_or_else(|_| format!("Unknown({cmd})"));

            if csv {
                println!("{},{},{},{},{}", idx, cmd_name, tick, size, compressed);
            } else {
                println!(
                    "{:05}  {:<22}  tick={}  size={}  compressed={}",
                    idx, cmd_name, tick, size, compressed
                );
            }
        }
    } else if events {
        // Scan and print (tick, EventName)
        let evs = parser.scan_packet_events()?;

        if csv {
            println!("idx,tick,event");
        }

        for (idx, (tick, name)) in evs.into_iter().enumerate() {
            // Filter by tick range, if provided
            if let Some(start) = start_tick
                && tick < start as i32
            {
                continue;
            }
            if let Some(end) = end_tick
                && tick > end as i32
            {
                continue;
            }

            if csv {
                println!("{},{},{}", idx, tick, name);
            } else {
                println!("{:05}  tick={}  {}", idx, tick, name);
            }
        }
    }

    Ok(())
}

fn cmd_kill(
    path: PathBuf,
) -> Result<()> {
    let parser = Parser::new(&path)?;
    parser.verify()?;
    parser.scan_kill_events()?;

    Ok(())
}
