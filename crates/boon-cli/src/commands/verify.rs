use std::path::Path;

use anyhow::Result;
use colored::Colorize;

pub fn run(file: &Path) -> Result<()> {
    let result = boon::Parser::from_file(file).and_then(|p| p.verify());

    match result {
        Ok(_) => {
            println!("{} {}", "Valid demo file:".green().bold(), file.display());
            Ok(())
        }
        Err(e) => {
            println!("{} {}", "Invalid demo file:".red().bold(), file.display());
            println!("  {}", e.to_string().red());
            Ok(())
        }
    }
}
