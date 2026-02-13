use std::path::Path;

use anyhow::Result;
use colored::Colorize;

#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    cmd_filter: Option<String>,
    tick_filter: Option<i32>,
    min_tick: Option<i32>,
    max_tick: Option<i32>,
    min_size: Option<u32>,
    max_size: Option<u32>,
    limit: Option<usize>,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)?;
    let messages = parser.messages()?;

    // Apply filters
    let filtered: Vec<_> = messages
        .iter()
        .filter(|msg| {
            // Command type filter (substring match, case-insensitive)
            if let Some(ref cmd) = cmd_filter
                && !msg.cmd_name.to_lowercase().contains(&cmd.to_lowercase())
            {
                return false;
            }
            // Exact tick filter
            if let Some(tick) = tick_filter
                && msg.tick != tick
            {
                return false;
            }
            // Min tick filter
            if let Some(min) = min_tick
                && msg.tick < min
            {
                return false;
            }
            // Max tick filter
            if let Some(max) = max_tick
                && msg.tick > max
            {
                return false;
            }
            // Min size filter
            if let Some(min) = min_size
                && msg.body_size < min
            {
                return false;
            }
            // Max size filter
            if let Some(max) = max_size
                && msg.body_size > max
            {
                return false;
            }
            true
        })
        .collect();

    let display_limit = limit.unwrap_or(filtered.len());

    println!(
        "{:<6} {:<8} {:<10} {:<8} {:<30}",
        "Index".bold(),
        "Tick".bold(),
        "Compress".bold(),
        "Size".bold(),
        "Command".bold(),
    );
    println!("{}", "-".repeat(70));

    for msg in filtered.iter().take(display_limit) {
        let compressed = if msg.compressed {
            "yes".yellow().to_string()
        } else {
            "no".to_string()
        };
        println!(
            "{:<6} {:<8} {:<10} {:<8} {}",
            msg.index, msg.tick, compressed, msg.body_size, msg.cmd_name,
        );
    }

    let filter_note = if cmd_filter.is_some()
        || tick_filter.is_some()
        || min_tick.is_some()
        || max_tick.is_some()
        || min_size.is_some()
        || max_size.is_some()
    {
        format!(" ({} matched filters)", filtered.len())
    } else {
        String::new()
    };

    println!(
        "\n{} messages total{}{}",
        messages.len(),
        filter_note,
        if display_limit < filtered.len() {
            format!(" (showing first {})", display_limit)
        } else {
            String::new()
        }
    );

    Ok(())
}
