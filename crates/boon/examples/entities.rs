//! Entity Snapshot — parses to a specific tick and inspects entity state.
//!
//! Usage:
//!   cargo run -p boon-deadlock --example entities -- <demo.dem> [tick]
//!
//! Defaults to tick 1000 if not specified.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: entities <demo.dem> [tick]");
    let target_tick: i32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);

    let parser = boon::Parser::from_file(Path::new(path)).expect("failed to open demo");
    let ctx = parser
        .parse_to_tick(target_tick)
        .expect("failed to parse to tick");

    println!("Parsed to tick {}  ({} active entities)", ctx.tick, ctx.entities.len());
    println!();

    // Find all CCitadelPlayerPawn entities
    let mut pawns: Vec<_> = ctx
        .entities
        .iter()
        .filter(|(_, e)| e.class_name == "CCitadelPlayerPawn")
        .collect();
    pawns.sort_by_key(|(idx, _)| *idx);

    println!("CCitadelPlayerPawn entities: {}", pawns.len());
    println!("{}", "-".repeat(70));

    for (idx, entity) in &pawns {
        let serializer = ctx.serializers.get(&entity.class_name);
        let ser = match serializer {
            Some(s) => s,
            None => continue,
        };

        // Read basic fields using get_by_name
        let health = entity.get_by_name("m_iHealth", ser);
        let max_health = entity.get_by_name("m_iMaxHealth", ser);
        let team = entity.get_by_name("m_iTeamNum", ser);

        // Position via CBodyComponent
        let x = entity.get_by_name("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX", ser);
        let y = entity.get_by_name("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY", ser);
        let z = entity.get_by_name("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ", ser);

        println!(
            "  Entity #{:<5} team={:<4} health={}/{}  pos=({}, {}, {})",
            idx,
            team.map_or("-".into(), |v| format!("{:?}", v)),
            health.map_or("-".into(), |v| format!("{:?}", v)),
            max_health.map_or("-".into(), |v| format!("{:?}", v)),
            x.map_or("-".into(), |v| format!("{:.1}", v)),
            y.map_or("-".into(), |v| format!("{:.1}", v)),
            z.map_or("-".into(), |v| format!("{:.1}", v)),
        );

        // Show first ability slot as an example of ability_name() lookup
        let ability_field = entity.get_by_name(
            "m_vecAbilities.0000",
            ser,
        );
        if let Some(boon::FieldValue::U32(ability_id)) = ability_field {
            let name = boon::ability_name(*ability_id);
            println!("    ability[0]: {} (id={})", name, ability_id);
        }
    }
}
