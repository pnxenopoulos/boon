//! Inspect imbues and per-ability percentages from a selected catalog directory.
//! Usage: cargo run -p boon-deadlock --example ability_stats -- DEMO CATALOG_DIR TICK
use boon::{
    Parser,
    ability_stats::{AbilityRuleset, AbilityStat, AbilityStatQuery, ImbueQuery},
    hero_stats::StatCatalog,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: ability_stats DEMO CATALOG_DIR TICK".into());
    }
    let parser = Parser::from_file(std::path::Path::new(&args[1]))?;
    let catalog = StatCatalog::from_directory(std::path::Path::new(&args[2]))?;
    let tick = args[3].parse()?;
    let stats = [
        AbilityStat::CooldownReduction,
        AbilityStat::DurationBonus,
        AbilityStat::RangeBonus,
        AbilityStat::RadiusBonus,
    ];
    let rules = stats
        .iter()
        .fold(AbilityRuleset::new(), |rules, stat| rules.with(stat.rule()));
    let imbues = parser.imbues(&ImbueQuery::new([tick]), &catalog)?;
    let query = AbilityStatQuery::new([tick], stats)
        .explain(true)
        .strict(false);
    let result = parser.calculate_ability_stats(&query, &catalog, &rules)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"imbues":imbues,"stats":result}))?
    );
    Ok(())
}
