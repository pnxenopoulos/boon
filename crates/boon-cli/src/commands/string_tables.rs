use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

pub fn run(
    file: &PathBuf,
    filter: Option<String>,
    summary: bool,
    limit: Option<usize>,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)?;
    let ctx = parser.parse_init()?;

    let mut tables: Vec<_> = ctx.string_tables.tables().iter().collect();

    if let Some(ref f) = filter {
        tables.retain(|t| t.name.contains(f.as_str()));
    }

    let limit = limit.unwrap_or(tables.len());

    if summary {
        // Summary mode: just names and entry counts
        println!("{:<40} {:>8}", "Table".bold(), "Entries".bold(),);
        println!("{}", "-".repeat(50));

        for table in tables.iter().take(limit) {
            println!("{:<40} {:>8}", table.name, table.entries.len());
        }
    } else {
        // Detailed mode: show sample entries
        let mut count = 0;

        for table in &tables {
            if count >= limit {
                break;
            }

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
            count += 1;
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
