use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

pub fn run(file: &PathBuf, filter: Option<String>, limit: Option<usize>) -> Result<()> {
    let parser = boon::Parser::from_file(file)?;
    let class_info = parser.parse_class_info()?;

    let mut classes: Vec<_> = class_info.classes.iter().collect();
    classes.sort_by_key(|c| c.class_id);

    if let Some(ref f) = filter {
        classes.retain(|c| c.network_name.contains(f.as_str()));
    }

    let limit = limit.unwrap_or(classes.len());

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
