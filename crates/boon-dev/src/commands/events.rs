use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

#[derive(Serialize)]
struct EventSummaryOutput {
    name: String,
    count: usize,
}

pub fn run(
    file: &Path,
    filter: Option<String>,
    summary: bool,
    tick: Option<i32>,
    limit: Option<usize>,
    inspect: bool,
    json: bool,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let mut events = parser.events(tick)?;

    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        events.retain(|e| e.name.to_lowercase().contains(&f_lower));
    }

    if summary {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for event in &events {
            *counts.entry(event.name.as_str()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(sorted.len());

        if json {
            let output: Vec<EventSummaryOutput> = sorted
                .iter()
                .take(limit)
                .map(|(name, count)| EventSummaryOutput {
                    name: name.to_string(),
                    count: *count,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        println!("{:<50} {:>6}", "Event".bold(), "Count".bold());
        println!("{}", "-".repeat(58));

        for (name, count) in sorted.iter().take(limit) {
            println!("{:<50} {:>6}", name, count);
        }

        println!(
            "\n{} events total ({} unique types){}",
            events.len(),
            sorted.len(),
            if limit < sorted.len() {
                format!(" (showing {} types)", limit)
            } else {
                String::new()
            }
        );
    } else {
        let limit = limit.unwrap_or(events.len());

        if json {
            let output: Vec<_> = events.iter().take(limit).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        for event in events.iter().take(limit) {
            if event.keys.is_empty() {
                println!(
                    "[tick {}] {} {}",
                    event.tick,
                    event.name.green().bold(),
                    format!("(UserMessage {})", event.msg_type).dimmed()
                );
            } else {
                println!("[tick {}] {}", event.tick, event.name.green().bold());
                for (key, value) in &event.keys {
                    println!("  {}: {}", key, value.dimmed());
                }
            }

            if inspect
                && !event.payload.is_empty()
                && let Some(decoded) = boon::decode_event_payload(event.msg_type, &event.payload)
            {
                for line in decoded.lines() {
                    println!("  {}", line);
                }
            }

            println!();
        }

        println!(
            "{} events{}",
            events.len(),
            if limit < events.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
