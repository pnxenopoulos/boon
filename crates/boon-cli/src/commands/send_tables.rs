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
    let container = parser.parse_send_tables()?;

    let mut serializers: Vec<_> = container.serializers.values().collect();
    serializers.sort_by(|a, b| a.name.cmp(&b.name));

    if let Some(ref f) = filter {
        serializers.retain(|s| s.name.contains(f.as_str()));
    }

    let limit = limit.unwrap_or(serializers.len());
    let mut count = 0;

    if summary {
        // Summary mode: just names and field counts
        println!("{:<50} {:>6}", "Serializer".bold(), "Fields".bold(),);
        println!("{}", "-".repeat(58));

        for ser in &serializers {
            if count >= limit {
                break;
            }
            println!("{:<50} {:>6}", ser.name, ser.fields.len());
            count += 1;
        }
    } else {
        // Detailed mode: full field information
        for ser in &serializers {
            if count >= limit {
                break;
            }

            println!("{} ({} fields)", ser.name.green().bold(), ser.fields.len());

            for field in &ser.fields {
                let encoder_info = field
                    .var_encoder
                    .as_deref()
                    .map(|e| format!(" [encoder: {}]", e))
                    .unwrap_or_default();
                let bits_info = field
                    .bit_count
                    .map(|b| format!(" [bits: {}]", b))
                    .unwrap_or_default();

                println!(
                    "  {}: {}{}{}",
                    field.var_name,
                    field.var_type.dimmed(),
                    encoder_info,
                    bits_info,
                );
            }

            println!();
            count += 1;
        }
    }

    println!(
        "\n{} serializers total{}",
        container.serializers.len(),
        if limit < serializers.len() {
            format!(" (showing {})", limit)
        } else {
            String::new()
        }
    );

    Ok(())
}
