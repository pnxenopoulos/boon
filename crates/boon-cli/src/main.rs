use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use owo_colors::OwoColorize;

use boon::parser::core::Parser;
use boon::parser::sendtables::Serializer as STSerializer;
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

    /// Parse and print SendTables (flattened serializers)
    SendTables {
        /// File: the demo to inspect
        file: PathBuf,

        /// Print one line per field (for 'show' or filtered list)
        #[arg(long)]
        fields: bool,

        /// CSV output
        #[arg(long)]
        csv: bool,

        /// Exact class name (mutually exclusive with --id). Can be repeated.
        #[arg(long = "class", value_name = "NAME")]
        classes: Vec<String>,

        /// Class id (mutually exclusive with --class). Can be repeated.
        #[arg(long = "id", value_name = "N")]
        ids: Vec<u16>,

        /// Case-insensitive substring filter on class name for list mode
        #[arg(long)]
        like: Option<String>,

        /// Also list serializers that have no class id
        #[arg(long)]
        include_orphans: bool,
    },
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
        Commands::SendTables {
            file,
            fields,
            csv,
            classes,
            ids,
            like,
            include_orphans,
        } => cmd_sendtables(file, fields, csv, classes, ids, like, include_orphans)?,
    }
    Ok(())
}

/* ------------------- check ------------------- */

fn cmd_check(path: PathBuf) -> Result<()> {
    println!("Reading {:#?}", path);
    let parser = Parser::from_path(&path)?;

    // Verify file
    if let Err(e) = parser.verify() {
        println!("Demo Verification: {}", "FAILURE".red());
        println!("{}", format!("Verification failed: {e}").red());
        return Ok(());
    } else {
        println!("Demo Verification: {}", "SUCCESS".green());
    }
    println!();

    // Parse metadata
    let meta = parser.parse_metadata()?;

    if let Some(h) = meta.header {
        let server = h.server_name.as_deref().unwrap_or("<unknown>");
        let client = h.client_name.as_deref().unwrap_or("<unknown>");
        let map = h.map_name.as_deref().unwrap_or("<unknown>");
        let build = h.build_num.unwrap_or_default();

        println!("Server Name : {server}");
        println!("Client Name : {client}");
        println!("Map Name    : {map}");
        println!("Build Num   : {build}");
    } else {
        println!("Error: {}", "No CDemoFileHeader found.".red());
    }

    if let Some(f) = meta.info {
        let t = f.playback_time.unwrap_or_default();
        let ticks = f.playback_ticks.unwrap_or_default();
        let frames = f.playback_frames.unwrap_or_default();

        println!("Playback Time (s) : {t}");
        println!("Playback Ticks    : {ticks}");
        println!("Playback Frames   : {frames}");
    } else {
        println!("Error: {}", "No CDemoFileInfo found.".red());
    }

    Ok(())
}

/* ------------------- debug ------------------- */

fn cmd_debug(
    path: PathBuf,
    csv: bool,
    start_tick: Option<u32>,
    end_tick: Option<u32>,
    messages: bool,
    events: bool,
) -> Result<()> {
    let parser = Parser::from_path(&path)?;
    parser.verify()?; // harmless here

    // default to messages if neither flag is provided
    if messages || !events {
        let entries = parser.scan_messages()?;

        if csv {
            println!("idx,command,tick,size,compressed");
        }

        for (idx, (cmd, tick, size, compressed)) in entries.into_iter().enumerate() {
            if let Some(start) = start_tick
                && tick < start
            {
                continue;
            }
            if let Some(end) = end_tick
                && tick > end
            {
                continue;
            }

            // EDemoCommands::try_from takes i32; guard the cast
            let cmd_name = i32::try_from(cmd)
                .ok()
                .and_then(|v| pb::EDemoCommands::try_from(v).ok())
                .map(|k| format!("{k:?}"))
                .unwrap_or_else(|| format!("Unknown({cmd})"));

            if csv {
                println!("{},{},{},{},{}", idx, cmd_name, tick, size, compressed);
            } else {
                println!(
                    "{:05}  {:<22}  tick={}  size={}  compressed={}",
                    idx, cmd_name, tick, size, compressed
                );
            }
        }
    } else {
        // events == true
        let evs = parser.scan_packet_events()?;

        if csv {
            println!("idx,tick,event");
        }

        for (idx, (tick, name)) in evs.into_iter().enumerate() {
            if let Some(start) = start_tick
                && tick < start
            {
                continue;
            }
            if let Some(end) = end_tick
                && tick > end
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

/* ------------------- sendtables ------------------- */

fn cmd_sendtables(
    path: PathBuf,
    fields: bool,
    csv: bool,
    classes: Vec<String>,
    ids: Vec<u16>,
    like: Option<String>,
    include_orphans: bool,
) -> Result<()> {
    let parser = Parser::from_path(&path)?;
    parser.verify()?; // quick sanity check
    let reg = parser.sendtables()?; // uses Parser::sendtables you added

    // If user targeted specific entries (by --class/--id), print those and exit.
    if !classes.is_empty() || !ids.is_empty() {
        // IDs first
        for id in ids {
            if let Some(ser) = reg.by_class.get(&id) {
                print_one_serializer(ser, csv, fields)?;
            } else {
                eprintln!("{}", format!("No serializer for class id {}", id).red());
            }
        }
        // Exact names next; also allow match via by_class values
        let class_set: HashSet<_> = classes.into_iter().collect();
        for name in class_set {
            if let Some(ser) = reg
                .by_name
                .get(&name)
                .or_else(|| reg.by_class.values().find(|s| s.name == name))
            {
                print_one_serializer(ser, csv, fields)?;
            } else {
                eprintln!("{}", format!("No serializer named '{}'", name).red());
            }
        }
        return Ok(());
    }

    // List mode (possibly filtered by --like).
    let needle = like.as_deref().map(|s| s.to_ascii_lowercase());

    // by_class (stable numeric order)
    let mut items: Vec<(&u16, &_)> = reg.by_class.iter().collect();
    items.sort_by_key(|(id, _)| **id);

    if csv {
        println!("class_id,class_name,version,field_count");
    } else {
        println!("{}", "SendTables".bold());
    }

    for (class_id, ser) in items {
        if let Some(ref n) = needle
            && !ser.name.to_ascii_lowercase().contains(n)
        {
            continue;
        }
        if csv {
            println!(
                "{},{},{},{}",
                class_id,
                ser.name,
                ser.version,
                ser.fields.len()
            );
        } else {
            println!(
                "[{:4}] {:<40} v{}  fields={}",
                class_id,
                ser.name,
                ser.version,
                ser.fields.len()
            );
            if fields {
                print_fields_block(ser);
            }
        }
    }

    // Optionally include name-only serializers
    if include_orphans {
        let mut orphans: Vec<_> = reg
            .by_name
            .values()
            .filter(|s| s.class_id.is_none())
            .collect();
        orphans.sort_by(|a, b| a.name.cmp(&b.name));

        if !orphans.is_empty() {
            if !csv {
                println!();
                println!("{}", "(name-only serializers)".italic());
            }
            for ser in orphans {
                if let Some(ref n) = needle
                    && !ser.name.to_ascii_lowercase().contains(n)
                {
                    continue;
                }
                if csv {
                    println!("-,{},{},{}", ser.name, ser.version, ser.fields.len());
                } else {
                    println!(
                        "[{:>4}] {:<40} v{}  fields={}",
                        "-",
                        ser.name,
                        ser.version,
                        ser.fields.len()
                    );
                    if fields {
                        print_fields_block(ser);
                    }
                }
            }
        }
    }

    Ok(())
}

/* ------------------- helpers ------------------- */

fn print_one_serializer(ser: &STSerializer, csv: bool, fields: bool) -> Result<()> {
    let class_id_str = ser
        .class_id
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".into());
    if csv {
        println!("class_id,class_name,version,field_count");
        println!(
            "{},{},{},{}",
            class_id_str,
            ser.name,
            ser.version,
            ser.fields.len()
        );
        if fields {
            println!(
                "class_id,idx,field_name,var_type,encoder,bit_count,flags,low,high,polymorphic_count"
            );
            for (idx, f) in ser.fields.iter().enumerate() {
                let low = f.low.map(|x| x.to_string()).unwrap_or_default();
                let high = f.high.map(|x| x.to_string()).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{},{}",
                    class_id_str,
                    idx,
                    f.name,
                    f.var_type,
                    f.encoder,
                    f.bit_count,
                    f.flags,
                    low,
                    high,
                    f.polymorphic.len()
                );
            }
        }
    } else {
        println!(
            "{}",
            format!(
                "[{}] {:<40} v{}  fields={}",
                class_id_str,
                ser.name,
                ser.version,
                ser.fields.len()
            )
            .bold()
        );
        if fields {
            print_fields_block(ser);
        }
    }
    Ok(())
}

fn print_fields_block(ser: &STSerializer) {
    println!("  {}", "Fields:".underline());
    for (idx, f) in ser.fields.iter().enumerate() {
        let low = f.low.map(|x| x.to_string()).unwrap_or_default();
        let high = f.high.map(|x| x.to_string()).unwrap_or_default();
        println!(
            "  {:3}  {:<36}  type={:<18} enc={:<18} bits={:<3} flags={:<5} low={} high={} poly={}",
            idx,
            f.name,
            f.var_type,
            f.encoder,
            f.bit_count,
            f.flags,
            low,
            high,
            f.polymorphic.len()
        );
    }
}
