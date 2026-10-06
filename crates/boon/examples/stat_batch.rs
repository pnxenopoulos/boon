//! Query hero stats, ability bonuses and imbues in one replay pass.
//! Usage: cargo run -p boon-deadlock --example stat_batch -- match.dem VERSION TICK
use boon::{
    Parser,
    ability_stats::{AbilityRuleset, AbilityStat, AbilityStatQuery, ImbueQuery},
    hero_stats::{HeroStat, HeroStatQuery, Ruleset, StatBatch, StatCatalog},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: stat_batch match.dem DATA_VERSION TICK".into());
    }
    let parser = Parser::from_file(std::path::Path::new(&args[1]))?;
    let catalog = StatCatalog::load(&args[2])?;
    let tick = args[3].parse::<i32>()?;
    let hero = HeroStatQuery::new([tick], [HeroStat::ClipSize])
        .explain(true)
        .strict(false);
    let hero_rules = Ruleset::new().with(HeroStat::ClipSize.rule());
    let abilities = AbilityStatQuery::new([tick], AbilityStat::ALL)
        .explain(true)
        .strict(false);
    let ability_rules = AbilityStat::ALL
        .iter()
        .fold(AbilityRuleset::new(), |rules, stat| rules.with(stat.rule()));
    let imbues = ImbueQuery::new([tick]);
    let batch = StatBatch::new()
        .hero_stats(&hero, &hero_rules)
        .ability_stats(&abilities, &ability_rules)
        .imbues(&imbues);
    println!(
        "{}",
        serde_json::to_string_pretty(&parser.calculate_stats(&batch, &catalog)?)?
    );
    Ok(())
}
