use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

pub fn run(
    file: &PathBuf,
    tick: i32,
    filter: Option<String>,
    summary: bool,
    fields: usize,
    limit: Option<usize>,
) -> Result<()> {
    let parser = boon::Parser::from_file(file)?;
    let ctx = parser.parse_to_tick(tick)?;

    let mut entities: Vec<_> = ctx.entities.entities.iter().collect();
    entities.sort_by_key(|(idx, _)| *idx);

    if let Some(ref f) = filter {
        entities.retain(|(_, e)| e.class_name.contains(f.as_str()));
    }

    if summary {
        // Summary mode: count entities by class name
        let mut class_counts: HashMap<&str, usize> = HashMap::new();
        for (_, entity) in &entities {
            *class_counts.entry(entity.class_name.as_str()).or_insert(0) += 1;
        }

        let mut counts: Vec<_> = class_counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let limit = limit.unwrap_or(counts.len());

        println!(
            "{:<50} {:>6}",
            "Class".bold(),
            "Count".bold(),
        );
        println!("{}", "-".repeat(58));

        for (class_name, count) in counts.iter().take(limit) {
            println!("{:<50} {:>6}", class_name, count);
        }

        println!(
            "\n{} entities at tick {} ({} unique classes){}",
            entities.len(),
            tick,
            counts.len(),
            if limit < counts.len() {
                format!(" (showing {} classes)", limit)
            } else {
                String::new()
            }
        );
    } else {
        // Detailed mode: show entity fields
        let limit = limit.unwrap_or(entities.len());
        let mut count = 0;

        for (idx, entity) in &entities {
            if count >= limit {
                break;
            }

            println!(
                "{} #{} (class_id: {})",
                entity.class_name.green().bold(),
                idx,
                entity.class_id,
            );

            let mut field_keys: Vec<_> = entity.fields.keys().collect();
            field_keys.sort();

            for key in field_keys.iter().take(fields) {
                if let Some(value) = entity.fields.get(*key) {
                    println!("  {}: {}", key, format!("{:?}", value).dimmed());
                }
            }

            if entity.fields.len() > fields {
                println!("  ... and {} more fields", entity.fields.len() - fields);
            }

            println!();
            count += 1;
        }

        println!(
            "{} entities at tick {}{}",
            ctx.entities.entities.len(),
            tick,
            if limit < entities.len() {
                format!(" (showing {})", limit)
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
