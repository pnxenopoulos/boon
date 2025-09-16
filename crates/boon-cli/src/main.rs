use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use owo_colors::OwoColorize;

// Boon-specific imports
use boon::reader::Reader;
use boon_proto::generated as pb;

/// Boon CLI
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
    /// Verify a demo file and print CDemoFileHeader if found in prologue
    Check {
        /// Path to the demo file (.dem / replay)
        file: PathBuf,
    },

    /// Print each framed message: command, tick, and size (after verify)
    Debug {
        /// Path to the demo file (.dem / replay)
        file: PathBuf,

        /// Print CSV (header: idx,command,tick,size) to stdout
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
    // Use your library parser
    let mut parser = boon::parser::Parser::open(&path)?;
    parser.verify()?; // checks magic, advances past prologue bytes
    println!("Reading {:#?}", path);
    println!("Demo Verification: {}", "SUCCESS".green());
    println!(" ");

    // Reads until the header
    parser.prologue()?;

    if let Some(h) = &parser.file_header {
        println!(
            "{} {}",
            "Server Name :".bold(),
            h.server_name.clone().unwrap()
        );
        println!(
            "{} {}",
            "Client Name :".bold(),
            h.client_name.clone().unwrap()
        );
        println!("{} {}", "Map Name    :".bold(), h.map_name.clone().unwrap());
        println!("{} {}", "Build Num   :".bold(), h.build_num.unwrap());
    } else {
        println!(
            "Error: {}",
            "No CDemoFileHeader found during prologue.".red()
        );
    }

    println!(" ");
    let _ = parser.read_demo_file_info();

    // Reads until file info
    if let Some(f) = &parser.file_info {
        println!(
            "{} {}",
            "Playback Time   :".bold(),
            f.playback_time.unwrap()
        );
        println!(
            "{} {}",
            "Playback Ticks  :".bold(),
            f.playback_ticks.unwrap()
        );
        println!(
            "{} {}",
            "Playback Frames :".bold(),
            f.playback_frames.unwrap()
        );
    } else {
        println!("Error: {}", "No CDemoFileInfo found.".red());
    }

    Ok(())
}

fn cmd_debug(path: PathBuf, csv_flag: bool) -> Result<()> {
    enum Mode {
        Stdout,
        Csv,
    }
    let mode = if csv_flag {
        Mode::Csv
    } else {
        // default to stdout if --csv isn't provided
        Mode::Stdout
    };

    // 1) Create a parser and verify the file
    let mut parser = boon::parser::Parser::open(&path)?;
    parser.verify()?; // sets framing start to byte 16 (8 magic + 8 prologue)

    // 2) Create a Reader on the file bytes and seek to the first frame
    //    (We read the file bytes again here since Parser's internal buffer is private.)
    let data = std::fs::read(&path)?;
    let mut r = Reader::new(&data);
    r.seek(16)?; // after verify(), first frame starts at offset 16

    // CSV header if requested
    if let Mode::Csv = mode {
        println!("idx,command,tick,size");
    }

    // 3) Iterate through frames and print command, tick, size
    let mut idx: usize = 0;
    while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
        // Advance past the payload to reach the next header.
        let _ = r.read_message_bytes(size)?;

        // Decode command id (mask off compression flag)
        let cmd_i32 = cmd_raw as i32;
        let flag = pb::EDemoCommands::DemIsCompressed as i32;
        let id = cmd_i32 & !flag;

        let cmd_name = pb::EDemoCommands::try_from(id)
            .map(|k| format!("{k:?}"))
            .unwrap_or_else(|_| format!("Unknown({id})"));

        match mode {
            Mode::Stdout => {
                println!("{:05}  {:<22}  tick={}  size={}", idx, cmd_name, tick, size);
            }
            Mode::Csv => {
                // simple CSV (fields here won't include commas)
                println!("{},{},{},{}", idx, cmd_name, tick, size);
            }
        }

        idx += 1;
    }

    Ok(())
}
