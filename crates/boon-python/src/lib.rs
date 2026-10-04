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
mod hero_stats;
mod loader;
mod names;
mod player_states;
mod runtime;
mod snapshots;

use datasets::*;
use names::*;
use snapshots::*;

/// A Deadlock demo file.
///
/// Args:
///     path: Path to the demo file.
///     preload: Load kills, damage, and abilities together (default True).
///         Set False for lightweight construction and loading on first access.
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
    cached_datasets: DatasetCache,
    cached_barriers: std::sync::OnceLock<BarrierTimeline>,
    cached_summary: Option<SummaryFrames>,
    // Game over state: (winning_team_num, tick), None if no event found
    game_over: Option<(i32, i32)>,
    // Match-clock seconds at game over. The flag distinguishes "not read"
    // from a demo that does not replicate the clock field.
    game_over_match_clock: Option<f32>,
    game_over_match_clock_scanned: bool,
    // Hero IDs from the `BannedHeroes` message.
    // `Some(vec![])` means no ban data. `None` means not scanned.
    banned_hero_ids: Option<Vec<u32>>,
    always_events_scanned: bool,
    cached_players: Option<DataFrame>,
}

/// Python bindings for the Boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add_function(wrap_pyfunction!(_read_catalog_names, m)?)?;
    m.add_function(wrap_pyfunction!(team_names, m)?)?;
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
    use super::{BARRIER_TRACKER_MODIFIER_ID, BarrierTimeline};
    use boon_proto::proto::CModifierTableEntry;
    use std::collections::HashMap;

    #[test]
    fn barrier_history_merges_changes_and_keeps_pawn_generations_separate() {
        let mut timeline = BarrierTimeline::default();
        let mut state = boon_parser::ModifierState::default();
        let mut serials = HashMap::new();
        let parent = 42;
        for (tick, entry) in [
            (
                10,
                CModifierTableEntry {
                    serial_number: Some(7),
                    parent: Some(parent),
                    modifier_subclass: Some(BARRIER_TRACKER_MODIFIER_ID),
                    float2: Some(123.5),
                    ..Default::default()
                },
            ),
            (
                11,
                CModifierTableEntry {
                    serial_number: Some(7),
                    stack_count: Some(2),
                    ..Default::default()
                },
            ),
            (
                12,
                CModifierTableEntry {
                    serial_number: Some(7),
                    float2: Some(90.0),
                    ..Default::default()
                },
            ),
            (
                12,
                CModifierTableEntry {
                    serial_number: Some(7),
                    float2: Some(80.0),
                    ..Default::default()
                },
            ),
            (
                13,
                CModifierTableEntry {
                    serial_number: Some(7),
                    parent: Some(43),
                    ..Default::default()
                },
            ),
            (
                14,
                CModifierTableEntry {
                    serial_number: Some(8),
                    parent: Some(43),
                    modifier_subclass: Some(BARRIER_TRACKER_MODIFIER_ID),
                    float2: Some(50.0),
                    ..Default::default()
                },
            ),
            (
                15,
                CModifierTableEntry {
                    entry_type: Some(2),
                    serial_number: Some(7),
                    ..Default::default()
                },
            ),
        ] {
            for change in state.apply_delta(0, entry) {
                timeline.apply(tick, change, &mut serials);
            }
        }
        for (tick, expected) in [
            (9, 0.0),
            (10, 123.5),
            (11, 123.5),
            (12, 80.0),
            (13, 0.0),
            (15, 0.0),
        ] {
            assert_eq!(timeline.remaining(tick, parent), expected);
        }
        assert_eq!(timeline.remaining(13, 43), 80.0);
        assert_eq!(timeline.remaining(14, 43), 50.0);
        assert_eq!(timeline.remaining(15, 43), 50.0);
        for (tick, serial, value) in [(16, 9, -1.0), (17, 10, f32::NAN)] {
            for change in state.apply_delta(
                0,
                CModifierTableEntry {
                    serial_number: Some(serial),
                    parent: Some(43),
                    modifier_subclass: Some(BARRIER_TRACKER_MODIFIER_ID),
                    float2: Some(value),
                    ..Default::default()
                },
            ) {
                timeline.apply(tick, change, &mut serials);
            }
            assert_eq!(timeline.remaining(tick, 43), 0.0);
        }
        assert_eq!(timeline.remaining(11, parent + (1 << 14)), 0.0);
    }
}
