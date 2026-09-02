use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use boon_proto::proto::CitadelUserMessageIds as Msg;
use polars::prelude::*;
use prost::Message;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_polars::PyDataFrame;

pyo3::create_exception!(_boon, InvalidDemoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoHeaderError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoInfoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoMessageError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, NotStreetBrawlError, pyo3::exceptions::PyException);

/// Build a `DataFrame` from columns, inferring row count from the first column.
fn df_from_columns(columns: Vec<Column>) -> PolarsResult<DataFrame> {
    let height = columns.first().map_or(0, |c| c.len());
    DataFrame::new(height, columns)
}

/// Helper to convert boon errors to Python exceptions.
fn to_py_err(e: boon_parser::Error) -> PyErr {
    match e {
        boon_parser::Error::Io(io_err) => PyErr::from(io_err),
        boon_parser::Error::InvalidMagic { got } => {
            InvalidDemoError::new_err(format!("Invalid demo file: bad magic bytes {got:?}"))
        }
        boon_parser::Error::Parse { context } => {
            InvalidDemoError::new_err(format!("Invalid demo file: {context}"))
        }
        other => InvalidDemoError::new_err(format!("{other}")),
    }
}

mod api;
mod datasets;
mod getters;
mod loader;
mod names;
mod runtime;
mod snapshots;

use datasets::*;
use names::*;
use snapshots::*;

/// A Deadlock demo file.
///
/// Args:
///     path: Path to the demo file.
///
/// Raises:
///     FileNotFoundError: If the file does not exist.
///     InvalidDemoError: If the file is not a valid demo file.
#[pyclass]
struct Demo {
    parser: boon_parser::Parser,
    path: PathBuf,
    // Cached info from file_header
    build: i32,
    map_name: String,
    // Cached info from file_info
    total_ticks: i32,
    playback_time: f32,
    tick_rate: i32,
    // Cached info from first tick entities
    match_id: Option<u64>,
    game_mode: i64,
    // Sorted ticks where the game was paused (lazily built from world_ticks)
    paused_ticks: Option<Vec<i32>>,
    // Cached dataset DataFrames
    cached_player_ticks: Option<DataFrame>,
    cached_world_ticks: Option<DataFrame>,
    cached_kills: Option<DataFrame>,
    cached_damage: Option<DataFrame>,
    cached_summary: Option<SummaryFrames>,
    // Game over state: (winning_team_num, tick), None if no event found
    game_over: Option<(i32, i32)>,
    // Hero IDs from the `BannedHeroes` message.
    // `Some(vec![])` means no ban data. `None` means not scanned.
    banned_hero_ids: Option<Vec<u32>>,
    always_events_scanned: bool,
    // Flex slot unlock events
    cached_flex_slots: Option<DataFrame>,
    cached_abilities: Option<DataFrame>,
    cached_ability_upgrades: Option<DataFrame>,
    cached_item_purchases: Option<DataFrame>,
    cached_chat: Option<DataFrame>,
    cached_objectives: Option<DataFrame>,
    cached_mid_boss: Option<DataFrame>,
    cached_troopers: Option<DataFrame>,
    cached_neutrals: Option<DataFrame>,
    cached_breakables: Option<DataFrame>,
    cached_sinners_sacrifice: Option<DataFrame>,
    cached_stat_modifier_events: Option<DataFrame>,
    cached_active_modifiers: Option<DataFrame>,
    cached_ability_ticks: Option<DataFrame>,
    cached_players: Option<DataFrame>,
    cached_street_brawl_ticks: Option<DataFrame>,
    cached_street_brawl_rounds: Option<DataFrame>,
    cached_urn: Option<DataFrame>,
    cached_rift: Option<DataFrame>,
}

/// Python bindings for the Boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add_function(wrap_pyfunction!(hero_names, m)?)?;
    m.add_function(wrap_pyfunction!(team_names, m)?)?;
    m.add_function(wrap_pyfunction!(ability_names, m)?)?;
    m.add_function(wrap_pyfunction!(ability_display_names, m)?)?;
    m.add_function(wrap_pyfunction!(modifier_names, m)?)?;
    m.add_function(wrap_pyfunction!(game_mode_names, m)?)?;
    m.add_function(wrap_pyfunction!(patron_phase_names, m)?)?;
    m.add_function(wrap_pyfunction!(hitgroup_names, m)?)?;
    m.add_function(wrap_pyfunction!(lifestate_names, m)?)?;
    m.add("InvalidDemoError", m.py().get_type::<InvalidDemoError>())?;
    m.add("DemoHeaderError", m.py().get_type::<DemoHeaderError>())?;
    m.add("DemoInfoError", m.py().get_type::<DemoInfoError>())?;
    m.add("DemoMessageError", m.py().get_type::<DemoMessageError>())?;
    m.add(
        "NotStreetBrawlError",
        m.py().get_type::<NotStreetBrawlError>(),
    )?;
    Ok(())
}

#[cfg(test)]
mod barrier_state_tests {
    use super::{BARRIER_TRACKER_MODIFIER_ID, BarrierState};
    use boon_proto::proto::CModifierTableEntry;

    fn entry(float2: Option<f32>) -> CModifierTableEntry {
        CModifierTableEntry {
            entry_type: Some(1),
            parent: Some(42),
            serial_number: Some(7),
            modifier_subclass: Some(BARRIER_TRACKER_MODIFIER_ID),
            float2,
            ..Default::default()
        }
    }

    fn apply(state: &mut BarrierState, index: usize, entry: CModifierTableEntry) {
        for change in state.modifiers.apply_delta(index, entry) {
            match change.kind {
                boon_parser::ModifierChangeKind::Removed => {
                    state.remove_serial(change.serial);
                }
                boon_parser::ModifierChangeKind::Applied
                | boon_parser::ModifierChangeKind::Changed => {
                    state.apply_live_entry(change.serial, &change.entry);
                }
            }
        }
    }

    #[test]
    fn barrier_defaults_clamps_preserves_and_removes() {
        let mut state = BarrierState::default();
        assert_eq!(state.remaining(42), 0.0);

        apply(&mut state, 3, entry(Some(-1.0)));
        assert_eq!(state.remaining(42), 0.0);

        apply(&mut state, 3, entry(Some(123.5)));
        assert_eq!(state.remaining(42), 123.5);

        apply(&mut state, 3, entry(None));
        assert_eq!(state.remaining(42), 123.5);

        apply(
            &mut state,
            3,
            CModifierTableEntry {
                entry_type: Some(2),
                serial_number: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(state.remaining(42), 0.0);
    }
}

#[cfg(test)]
mod resistance_tests {
    use super::effective_resistances_from_values;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-4,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn includes_per_level_hero_resistance() {
        let [kelvin_bullet, kelvin_spirit] = effective_resistances_from_values(12, 34, [], []);
        assert_close(kelvin_bullet, 0.0);
        assert_close(kelvin_spirit, 20.625);

        let [bebop_bullet, bebop_spirit] = effective_resistances_from_values(15, 32, [], []);
        assert_close(bebop_bullet, 9.3);
        assert_close(bebop_spirit, 0.0);
    }

    #[test]
    fn includes_level_and_modifier_spirit_power_in_scaling() {
        // Venator at level 31 has 33 spirit power from level growth; the
        // network vector contributes another 15 in the reference demo.
        let [bullet, spirit] =
            effective_resistances_from_values(65, 31, [(158, 6.0), (158, 9.0)], []);
        assert_close(bullet, 5.84544);
        assert_close(spirit, 5.84544);
    }

    #[test]
    fn combines_equipped_item_resistance_multiplicatively() {
        // 18% from Battle Vest and 9% from Bullet Resist Shredder.
        let [bullet, spirit] =
            effective_resistances_from_values(72, 30, [], [1_235_347_618, 2_971_868_509]);
        assert_close(bullet, 25.38);
        assert_close(spirit, 0.0);
    }

    #[test]
    fn matches_paige_and_billy_baseline_resistance() {
        let [paige_bullet, paige_spirit] =
            effective_resistances_from_values(67, 30, [], [1_193_964_439]);
        assert_close(paige_bullet, 0.0);
        assert_close(paige_spirit, 10.0);

        let [billy_bullet, billy_spirit] = effective_resistances_from_values(
            72,
            30,
            [],
            [1_193_964_439, 1_235_347_618, 2_971_868_509, 3_731_635_960],
        );
        assert_close(billy_bullet, 25.38);
        assert_close(billy_spirit, 19.0);
    }
}
