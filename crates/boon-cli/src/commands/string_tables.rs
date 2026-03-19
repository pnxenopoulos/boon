use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

#[derive(Serialize)]
struct StringTableEntryOutput {
    index: usize,
    key: String,
    data_size: Option<usize>,
}

#[derive(Serialize)]
struct StringTableOutput {
    name: String,
    entry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<StringTableEntryOutput>>,
}

pub fn run(
    file: &Path,
    filter: Option<String>,
    summary: bool,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let ctx = parser.parse_init()?;

    let mut tables: Vec<_> = ctx.string_tables.tables().iter().collect();

    if let Some(ref f) = filter {
        tables.retain(|t| t.name.contains(f.as_str()));
    }

    let limit = limit.unwrap_or(tables.len());

    if json {
        let output: Vec<StringTableOutput> = tables
            .iter()
            .take(limit)
            .map(|table| StringTableOutput {
                name: table.name.clone(),
                entry_count: table.entries.len(),
                entries: if summary {
                    None
                } else {
                    Some(
                        table
                            .entries
                            .iter()
                            .enumerate()
                            .map(|(i, entry)| StringTableEntryOutput {
                                index: i,
                                key: entry.string.clone().unwrap_or_default(),
                                data_size: entry.user_data.as_ref().map(|d| d.len()),
                            })
                            .collect(),
                    )
                },
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if summary {
        // Summary mode: just names and entry counts
        println!("{:<40} {:>8}", "Table".bold(), "Entries".bold(),);
        println!("{}", "-".repeat(50));

        for table in tables.iter().take(limit) {
            println!("{:<40} {:>8}", table.name, table.entries.len());
        }
    } else {
        // Detailed mode: show sample entries
        for table in tables.iter().take(limit) {
            println!(
                "{} ({} entries)",
                table.name.green().bold(),
                table.entries.len()
            );

            // Show up to 5 sample entries
            for (i, entry) in table.entries.iter().enumerate().take(5) {
                let key = entry.string.as_deref().unwrap_or("<none>");
                let data_len = entry
                    .user_data
                    .as_ref()
                    .map(|d| format!("{} bytes", d.len()))
                    .unwrap_or_else(|| "no data".to_string());
                println!("  [{}] {} ({})", i, key, data_len.dimmed());
            }

            if table.entries.len() > 5 {
                println!("  ... and {} more", table.entries.len() - 5);
            }

            println!();
        }
    }

    println!(
        "\n{} string tables total{}",
        ctx.string_tables.tables().len(),
        if limit < tables.len() {
            format!(" (showing {})", limit)
        } else {
            String::new()
        }
    );

    Ok(())
}
