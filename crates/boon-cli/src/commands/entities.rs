use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

pub fn run(
    file: &Path,
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

        println!("{:<50} {:>6}", "Class".bold(), "Count".bold(),);
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

        for (idx, entity) in entities.iter().take(limit) {
            println!(
                "{} #{} (class_id: {})",
                entity.class_name.green().bold(),
                idx,
                entity.class_id,
            );

            // Resolve field names using the serializer
            let serializer = ctx.serializers.get(&entity.class_name);
            let mut resolved_fields: Vec<(String, &boon::FieldValue)> = entity
                .fields
                .iter()
                .map(|(&key, value)| {
                    let name = serializer
                        .as_ref()
                        .and_then(|s| s.field_name_for_key(key))
                        .unwrap_or_else(|| format!("{:#x}", key));
                    (name, value)
                })
                .collect();
            resolved_fields.sort_by(|a, b| a.0.cmp(&b.0));

            for (name, value) in resolved_fields.iter().take(fields) {
                println!("  {}: {}", name, format!("{:?}", value).dimmed());
            }

            if entity.fields.len() > fields {
                println!("  ... and {} more fields", entity.fields.len() - fields);
            }

            println!();
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
