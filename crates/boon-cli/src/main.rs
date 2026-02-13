use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "boon", about = "Boon — Deadlock demo file parser")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify a demo file's magic bytes and header
    Verify {
        /// Path to the demo file
        file: PathBuf,
    },
    /// List all demo messages with metadata
    Messages {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by command/packet type (substring match)
        #[arg(long, value_name = "TYPE")]
        cmd: Option<String>,
        /// Filter by exact tick
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
        /// Filter by minimum size (bytes)
        #[arg(long, value_name = "BYTES")]
        min_size: Option<u32>,
        /// Filter by maximum size (bytes)
        #[arg(long, value_name = "BYTES")]
        max_size: Option<u32>,
        /// Maximum number of messages to display
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Print file header and file info
    Info {
        /// Path to the demo file
        file: PathBuf,
    },
    /// Display flattened serializers (entity field definitions)
    SendTables {
        /// Path to the demo file
        file: PathBuf,
        /// Filter serializers by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only names and field counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of serializers to display
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Display entity class info
    Classes {
        /// Path to the demo file
        file: PathBuf,
        /// Filter classes by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Maximum number of classes to display
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Display string tables
    StringTables {
        /// Path to the demo file
        file: PathBuf,
        /// Filter tables by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only names and entry counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of tables to display
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List game events from a demo
    Events {
        /// Path to the demo file
        file: PathBuf,
        /// Filter events by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only event names and counts
        #[arg(long)]
        summary: bool,
        /// Maximum tick to parse up to
        #[arg(long)]
        tick: Option<i32>,
        /// Maximum number of events to display
        #[arg(long)]
        limit: Option<usize>,
        /// Decode and display full message contents
        #[arg(long)]
        inspect: bool,
    },
    /// Inspect entity state at a given tick
    Entities {
        /// Path to the demo file
        file: PathBuf,
        /// Game tick to parse to
        #[arg(long)]
        tick: i32,
        /// Filter entities by class name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only class names and counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of fields to display per entity
        #[arg(long, default_value = "20")]
        fields: usize,
        /// Maximum number of entities to display
        #[arg(long)]
        limit: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify { file } => commands::verify(&file),
        Commands::Messages {
            file,
            cmd,
            tick,
            min_tick,
            max_tick,
            min_size,
            max_size,
            limit,
        } => commands::messages(
            &file, cmd, tick, min_tick, max_tick, min_size, max_size, limit,
        ),
        Commands::Info { file } => commands::info(&file),
        Commands::SendTables {
            file,
            filter,
            summary,
            limit,
        } => commands::send_tables(&file, filter, summary, limit),
        Commands::Classes {
            file,
            filter,
            limit,
        } => commands::classes(&file, filter, limit),
        Commands::StringTables {
            file,
            filter,
            summary,
            limit,
        } => commands::string_tables(&file, filter, summary, limit),
        Commands::Events {
            file,
            filter,
            summary,
            tick,
            limit,
            inspect,
        } => commands::events(&file, filter, summary, tick, limit, inspect),
        Commands::Entities {
            file,
            tick,
            filter,
            summary,
            fields,
            limit,
        } => commands::entities(&file, tick, filter, summary, fields, limit),
    }
}
