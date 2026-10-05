//! Entity Snapshot — parses to a specific tick and inspects entity state.
//!
//! Usage:
//!   cargo run -p boon-deadlock --example entities -- <demo.dem> [tick]
//!
//! Defaults to tick 1000 if not specified.

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or("usage: entities <demo.dem> [tick]")?;
    let target_tick: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);

    let parser = boon::Parser::from_file(Path::new(path))?;
    let names = boon::CatalogNames::load(None)?;
    let ctx = parser.parse_to_tick(target_tick)?;

    println!(
        "Parsed to tick {}  ({} active entities)",
        ctx.tick(),
        ctx.entities().len()
    );
    println!();

    // Find all CCitadelPlayerPawn entities
    let mut pawns: Vec<_> = ctx
        .entities()
        .iter()
        .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerPawn")
        .collect();
    pawns.sort_by_key(|(idx, _)| *idx);

    println!("CCitadelPlayerPawn entities: {}", pawns.len());
    println!("{}", "-".repeat(70));

    for (idx, entity) in &pawns {
        let serializer = ctx.serializers().get(&entity.class_name);
        let Some(ser) = serializer else {
            continue;
        };

        // Read basic fields using get_by_name
        let health = entity.get_by_name("m_iHealth", ser);
        let max_health = entity.get_by_name("m_iMaxHealth", ser);
        let team = entity.get_by_name("m_iTeamNum", ser);

        // Get the position from CBodyComponent. Source 2 splits the position
        // into a cell index and an offset in the cell. `boon::world_position`
        // combines m_cellX/Y/Z with m_vecOrigin.m_vec{X,Y,Z}. The result uses
        // Hammer units.
        let cell_keys = [
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX"),
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY"),
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ"),
        ];
        let offset_keys = [
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX"),
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY"),
            ser.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ"),
        ];
        let [x, y, z] = boon::world_position(entity, cell_keys, offset_keys);

        println!(
            "  Entity #{:<5} team={:<4} health={}/{}  pos=({:.1}, {:.1}, {:.1})",
            idx,
            team.map_or("-".into(), |v| format!("{:?}", v)),
            health.map_or("-".into(), |v| format!("{:?}", v)),
            max_health.map_or("-".into(), |v| format!("{:?}", v)),
            x,
            y,
            z,
        );

        // The slot contains an entity handle. Read the catalog ID from that entity.
        let ability = entity
            .get_handle(ser.resolve_field_key("m_CCitadelAbilityComponent.m_vecAbilities.0"))
            .and_then(|handle| ctx.entities().get_by_handle(handle));
        if let Some(ability) = ability
            && let Some(serializer) = ctx.serializers().get(&ability.class_name)
            && let Some(id) = ability.get_u64(serializer.resolve_field_key("m_nSubclassID"))
            && let Ok(id) = u32::try_from(id)
        {
            println!("    ability[0]: {} (id={})", names.ability_name(id), id);
        }
    }
    Ok(())
}
