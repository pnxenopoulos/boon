use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use boon_cli::commands;

#[derive(Parser)]
#[command(name = "boon", about = "Boon — Deadlock demo file parser", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List ability usage events from a demo
    Abilities {
        /// Path to the demo file
        file: PathBuf,
        /// Filter abilities by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only ability names and counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of abilities to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
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
    /// Print post-match summary from the last-tick game event
    Summary {
        /// Path to the demo file
        file: PathBuf,
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
    /// List chat messages from a demo
    Chat {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by text or chat type substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only hero message counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// List hero ability upgrade events (skill point spending)
    AbilityUpgrades {
        /// Path to the demo file
        file: PathBuf,
        /// Filter abilities by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only ability names and counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Track objective entity health per tick
    Objectives {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by objective type substring (walker, titan, barracks, mid_boss)
        #[arg(long)]
        filter: Option<String>,
        /// Show only objective type/team/lane counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire)
    MidBoss {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by event type substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only event type counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Track neutral creep state changes (only emits rows when state changes)
    Neutrals {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by neutral type substring (neutral, neutral_node_mover)
        #[arg(long)]
        filter: Option<String>,
        /// Show only neutral type/team counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Track alive lane trooper position and state per tick
    Troopers {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by trooper type substring (trooper, trooper_boss, neutral)
        #[arg(long)]
        filter: Option<String>,
        /// Show only trooper type/team/lane counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Track per-player cumulative permanent stat bonuses (idol/breakable pickups)
    StatModifiers {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by stat name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show per-hero final stat values
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// List item shop transactions (purchases, sells, swaps)
    ShopEvents {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by ability name or change type substring
        #[arg(long)]
        filter: Option<String>,
        /// Show only ability+change combos and counts
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
    /// Track active buff/debuff modifiers on players (applied/removed events)
    ActiveModifiers {
        /// Path to the demo file
        file: PathBuf,
        /// Filter by modifier name or ability name substring
        #[arg(long)]
        filter: Option<String>,
        /// Show applied event counts per ability per hero
        #[arg(long)]
        summary: bool,
        /// Maximum number of entries to display
        #[arg(long)]
        limit: Option<usize>,
        /// Filter by exact tick (equivalent to --min-tick N --max-tick N)
        #[arg(long)]
        tick: Option<i32>,
        /// Filter by minimum tick
        #[arg(long, value_name = "TICK")]
        min_tick: Option<i32>,
        /// Filter by maximum tick
        #[arg(long, value_name = "TICK")]
        max_tick: Option<i32>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.json;

    match cli.command {
        Commands::Abilities {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::abilities(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::Verify { file } => commands::verify(&file, json),
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
            &file, cmd, tick, min_tick, max_tick, min_size, max_size, limit, json,
        ),
        Commands::Info { file } => commands::info(&file, json),
        Commands::SendTables {
            file,
            filter,
            summary,
            limit,
        } => commands::send_tables(&file, filter, summary, limit, json),
        Commands::Classes {
            file,
            filter,
            limit,
        } => commands::classes(&file, filter, limit, json),
        Commands::StringTables {
            file,
            filter,
            summary,
            limit,
        } => commands::string_tables(&file, filter, summary, limit, json),
        Commands::Events {
            file,
            filter,
            summary,
            tick,
            limit,
            inspect,
        } => commands::events(&file, filter, summary, tick, limit, inspect, json),
        Commands::Summary { file } => commands::summary(&file, json),
        Commands::Entities {
            file,
            tick,
            filter,
            summary,
            fields,
            limit,
        } => commands::entities(&file, tick, filter, summary, fields, limit, json),
        Commands::Chat {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::chat(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::AbilityUpgrades {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::ability_upgrades(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::Objectives {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::objectives(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::MidBoss {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::mid_boss(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::Neutrals {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::neutrals(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::Troopers {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::troopers(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::StatModifiers {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::stat_modifiers(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::ShopEvents {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::shop_events(&file, filter, summary, limit, min_tick, max_tick, json)
        }
        Commands::ActiveModifiers {
            file,
            filter,
            summary,
            limit,
            tick,
            min_tick,
            max_tick,
        } => {
            let min_tick = tick.or(min_tick);
            let max_tick = tick.or(max_tick);
            commands::active_modifiers(&file, filter, summary, limit, min_tick, max_tick, json)
        }
    }
}
