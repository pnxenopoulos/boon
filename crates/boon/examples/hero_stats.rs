//! Calculate ammo with an explicit boon-data version.
//! Usage: cargo run -p boon-deadlock --example hero_stats -- match.dem VERSION TICK
use boon::{
    Parser,
    hero_stats::{HeroStat, HeroStatQuery, Ruleset, StatCatalog},
    rulesets,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: hero_stats match.dem DATA_VERSION TICK".into());
    }
    let parser = Parser::from_file(std::path::Path::new(&args[1]))?;
    let catalog = StatCatalog::load(&args[2])?;
    let query = HeroStatQuery::new([args[3].parse()?], [HeroStat::ClipSize])
        .explain(true)
        .strict(false);
    let rules = Ruleset::new().with(rulesets::clip_size::V1);
    println!(
        "{}",
        serde_json::to_string_pretty(&parser.calculate_hero_stats(&query, &catalog, &rules)?)?
    );
    Ok(())
}
