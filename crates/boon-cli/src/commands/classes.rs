use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

pub fn run(file: &Path, filter: Option<String>, limit: Option<usize>, json: bool) -> Result<()> {
    let parser = boon::Parser::from_file(file)
        .with_context(|| format!("failed to open {}", file.display()))?;
    let class_info = parser.parse_class_info()?;

    let mut classes: Vec<_> = class_info.classes.iter().collect();
    classes.sort_by_key(|c| c.class_id);

    if let Some(ref f) = filter {
        classes.retain(|c| c.network_name.contains(f.as_str()));
    }

    let limit = limit.unwrap_or(classes.len());

    if json {
        let output: Vec<_> = classes.iter().take(limit).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "{:<8} {:<50} {}",
        "ID".bold(),
        "Network Name".bold(),
        "Table Name".bold(),
    );
    println!("{}", "-".repeat(90));

    for class in classes.iter().take(limit) {
        println!(
            "{:<8} {:<50} {}",
            class.class_id, class.network_name, class.table_name,
        );
    }

    println!(
        "\n{} classes total (encoding bits: {}){}",
        class_info.classes.len(),
        class_info.bits,
        if limit < classes.len() {
            format!(" (showing {})", limit)
        } else {
            String::new()
        }
    );

    Ok(())
}
