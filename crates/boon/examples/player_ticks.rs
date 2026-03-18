//! Filtered Tick Streaming — streams the full demo with a class filter,
//! collecting per-tick player pawn data.
//!
//! Usage: cargo run -p boon-deadlock --example player_ticks -- <demo.dem>

use std::collections::HashSet;
use std::path::Path;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: player_ticks <demo.dem>");

    let parser = boon::Parser::from_file(Path::new(&path)).expect("failed to open demo");

    // Only track CCitadelPlayerPawn entities
    let filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();

    let mut total_ticks: u64 = 0;
    let mut total_snapshots: u64 = 0;
    let mut keys_resolved = false;
    let mut nk_health: Option<u64> = None;
    let mut nk_vec_x: Option<u64> = None;
    let mut nk_vec_y: Option<u64> = None;
    let mut nk_vec_z: Option<u64> = None;

    parser
        .run_to_end_filtered(&filter, |ctx| {
            // Resolve field keys once from the serializer
            if !keys_resolved {
                if let Some(s) = ctx.serializers.get("CCitadelPlayerPawn") {
                    nk_health = s.resolve_field_key("m_iHealth");
                    nk_vec_x = s.resolve_field_key(
                        "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                    );
                    nk_vec_y = s.resolve_field_key(
                        "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                    );
                    nk_vec_z = s.resolve_field_key(
                        "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                    );
                }
                keys_resolved = true;
            }

            total_ticks += 1;

            for (_, entity) in ctx.entities.iter() {
                if entity.class_name != "CCitadelPlayerPawn" {
                    continue;
                }

                let health = nk_health
                    .and_then(|k| entity.fields.get(&k))
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_else(|| "-".into());
                let x = nk_vec_x
                    .and_then(|k| entity.fields.get(&k))
                    .map(|v| format!("{:.1}", v))
                    .unwrap_or_else(|| "-".into());
                let y = nk_vec_y
                    .and_then(|k| entity.fields.get(&k))
                    .map(|v| format!("{:.1}", v))
                    .unwrap_or_else(|| "-".into());
                let z = nk_vec_z
                    .and_then(|k| entity.fields.get(&k))
                    .map(|v| format!("{:.1}", v))
                    .unwrap_or_else(|| "-".into());

                // Print first 20 ticks of data, then just count
                if total_ticks <= 20 {
                    println!(
                        "[tick {:>6}] pawn #{:<5} health={:<6} pos=({}, {}, {})",
                        ctx.tick, entity.index, health, x, y, z,
                    );
                }

                total_snapshots += 1;
            }
        })
        .expect("failed to parse demo");

    println!();
    println!("Summary:");
    println!("  Total ticks processed: {}", total_ticks);
    println!("  Total pawn snapshots:  {}", total_snapshots);
    println!(
        "  Avg pawns per tick:    {:.1}",
        total_snapshots as f64 / total_ticks.max(1) as f64
    );
}
