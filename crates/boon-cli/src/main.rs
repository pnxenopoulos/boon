use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use owo_colors::OwoColorize;

use boon::parser::{Parser}; // only import your public API
use boon_proto::generated as pb; // optional: only for enum names in debug listing

#[derive(ClapParser, Debug)]
#[command(name="boon", version, about="Deadlock demo file / replay utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Verify and print header/info found in the prologue
    Check { file: PathBuf },

    /// Print each framed message: command, tick, size, compressed
    Debug {
        file: PathBuf,
        #[arg(long)]
        csv: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Check { file } => cmd_check(file)?,
        Commands::Debug { file, csv } => cmd_debug(file, csv)?,
    }
    Ok(())
}

fn cmd_check(path: PathBuf) -> Result<()> {
    println!("Reading {:#?}", path);
    let parser = Parser::open(&path)?;

    if let Err(e) = parser.verify() {
        println!("Demo Verification: {}", "FAILURE".red());
        println!("{}", format!("Verification failed: {e}").red());
        return Ok(());
    } else {
        println!("Demo Verification: {}", "SUCCESS".green());
    }
    println!();

    let meta = parser.prologue_meta()?;

    if let Some(h) = meta.header {
        println!("Server Name : {}", h.server_name.unwrap());
        println!("Client Name : {}", h.client_name.unwrap());
        println!("Map Name    : {}", h.map_name.unwrap());
        println!("Build Num   : {}", h.build_num.unwrap());
    } else {
        println!("Error: {}", "No CDemoFileHeader found.".red());
    }

    println!();

    if let Some(f) = meta.info {
        println!("Playback Time   : {}", f.playback_time.unwrap());
        println!("Playback Ticks  : {}", f.playback_ticks.unwrap());
        println!("Playback Frames : {}", f.playback_frames.unwrap());
    } else {
        println!("Error: {}", "No CDemoFileInfo found.".red());
    }

    Ok(())
}

fn cmd_debug(path: PathBuf, csv: bool) -> Result<()> {
    let parser = Parser::open(&path)?;
    parser.verify()?;

    let events = parser.scan()?;

    if csv {
        println!("idx,command,tick,size,compressed");
    }

    for (idx, (cmd, tick, size, compressed)) in events.into_iter().enumerate() {
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

    Ok(())
}