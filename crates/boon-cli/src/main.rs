use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => cmd_check(file)?,
    }

    Ok(())
}

fn cmd_check(path: PathBuf) -> Result<()> {
    // Use your library parser
    let mut parser = boon::parser::Parser::open(&path)?;
    parser.verify()?; // checks magic, advances past prologue bytes
    parser.prologue()?; // reads frames until CDemoSyncTick; stores CDemoFileHeader if seen

    if let Some(h) = &parser.file_header {
        println!("CDemoFileHeader:");
        // prost messages implement Debug by default; pretty-print it
        println!("{:#?}", h);
    } else {
        println!("No CDemoFileHeader found during prologue.");
    }

    Ok(())
}
