//! Small observations retained from the independent source2-demo comparison.
use std::collections::HashSet;

use boon::{EffectiveModifierState, ModifierClock, Parser};
use serde_json::{Value, json};

#[test]
#[ignore = "requires 111155916.dem and boon-data 6746; set BOON_COMPARISON_DEMO and BOON_COMPARISON_DATA"]
fn raw_modifier_samples_match_independent_parser() {
    let demo = std::env::var_os("BOON_COMPARISON_DEMO").expect("set BOON_COMPARISON_DEMO");
    let directory = std::env::var_os("BOON_COMPARISON_DATA").expect("set BOON_COMPARISON_DATA");
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/modifier-comparison.json")).unwrap();
    let samples = expected["samples"].as_array().unwrap();
    let parser = Parser::from_file(std::path::Path::new(&demo)).unwrap();
    let catalog =
        boon::hero_stats::StatCatalog::from_directory(std::path::Path::new(&directory)).unwrap();
    let initial = parser.parse_init().unwrap();
    let clock = ModifierClock::resolve(&initial);
    let mut state = EffectiveModifierState::with_catalog(&catalog);
    state.rebuild(&initial, clock.game_time(&initial));
    let end = samples.last().unwrap()["tick"].as_i64().unwrap() as i32 + 1;
    let classes = HashSet::from([
        "CCitadelPlayerPawn",
        "CCitadelPlayerController",
        "CCitadelGameRulesProxy",
    ]);
    let mut seen = 0;
    parser.decode_segment(None, end, &classes, |ctx| {
        state.update(ctx, clock.game_time(ctx));
        let Some(sample) = samples.iter().find(|row| row["tick"] == ctx.tick()) else { return };
        seen += 1;
        assert_eq!(state.raw_entries().len(), sample["raw_count"].as_u64().unwrap() as usize, "tick {}", ctx.tick());
        assert!(!state.raw_entries().contains_key(&486), "the zipline modifier was removed at tick 194");
        for serial in [4604, 4605] {
            let expected = sample["entries"].as_array().unwrap().iter().find(|row| row["serial"] == serial);
            let actual = state.raw_entries().get(&serial).map(|entry| json!({
                "serial": entry.serial_number, "parent": entry.parent,
                "modifier_id": entry.modifier_subclass, "ability_id": entry.ability_subclass,
                "ability_handle": entry.ability, "caster": entry.caster, "stacks": entry.stack_count,
                "duration": entry.duration.map(f64::from), "creation": entry.creation_time.map(f64::from),
                "last_applied": entry.last_applied_time.map(f64::from), "in_aura": entry.in_aura_range,
                "float1": entry.float1.map(f64::from), "float2": entry.float2.map(f64::from), "int1": entry.int1,
            }));
            assert_eq!(actual.as_ref(), expected, "tick {} serial {serial}", ctx.tick());
        }
    }).unwrap();
    assert_eq!(seen, samples.len());
}
