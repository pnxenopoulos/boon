use std::path::PathBuf;

use polars::prelude::*;
use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;

pyo3::create_exception!(_boon, InvalidDemoError, pyo3::exceptions::PyException);

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
    build: Option<i32>,
    map_name: Option<String>,
    // Cached info from file_info
    total_ticks: Option<i32>,
    playback_time: Option<f32>,
    // Cached info from first tick entities
    match_id: Option<u64>,
}

#[pymethods]
impl Demo {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);

        // Check if file exists first for a clear FileNotFoundError
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "Demo file not found: {}",
                path.display()
            )));
        }

        let parser = boon_parser::Parser::from_file(&path).map_err(to_py_err)?;

        // Verify the file is a valid demo
        parser.verify().map_err(to_py_err)?;

        // Parse header info
        let header = parser.file_header().map_err(to_py_err)?;
        let build = header.build_num;
        let map_name = header.map_name;

        // Parse file info
        let info = parser.file_info().map_err(to_py_err)?;
        let total_ticks = info.playback_ticks;
        let playback_time = info.playback_time;

        // Parse first tick to get match_id from CCitadelGameRulesProxy
        let ctx = parser.parse_to_tick(1).map_err(to_py_err)?;
        let match_id = ctx
            .entities
            .iter()
            .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy")
            .and_then(|(_, e)| e.fields.get("m_pGameRules.m_unMatchID"))
            .and_then(|v| match v {
                boon_parser::FieldValue::U64(id) => Some(*id),
                boon_parser::FieldValue::I64(id) => Some(*id as u64),
                _ => None,
            });

        Ok(Demo {
            parser,
            path,
            build,
            map_name,
            total_ticks,
            playback_time,
            match_id,
        })
    }

    /// Verify that the file is a valid demo file.
    ///
    /// Returns:
    ///     True if the file is valid.
    ///
    /// Note:
    ///     This is already called during construction, so it will always
    ///     return True for an existing Demo instance.
    fn verify(&self) -> PyResult<bool> {
        self.parser.verify().map_err(to_py_err)?;
        Ok(true)
    }

    /// The path to the demo file.
    #[getter]
    fn path(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    /// The total number of ticks in the demo.
    #[getter]
    fn total_ticks(&self) -> Option<i32> {
        self.total_ticks
    }

    /// The total duration of the demo in seconds.
    #[getter]
    fn total_seconds(&self) -> Option<f32> {
        self.playback_time
    }

    /// The total duration of the demo as a formatted string (e.g., "12:34").
    #[getter]
    fn total_clock_time(&self) -> Option<String> {
        self.playback_time.map(|t| {
            let total_seconds = t as u32;
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{minutes}:{seconds:02}")
        })
    }

    /// The build number of the game that recorded the demo.
    #[getter]
    fn build(&self) -> Option<i32> {
        self.build
    }

    /// The name of the map the demo was recorded on.
    #[getter]
    fn map_name(&self) -> Option<String> {
        self.map_name.clone()
    }

    /// The match ID for this demo.
    #[getter]
    fn match_id(&self) -> Option<u64> {
        self.match_id
    }

    /// The tick rate of the demo (ticks per second).
    #[getter]
    fn tick_rate(&self) -> Option<i32> {
        match (self.total_ticks, self.playback_time) {
            (Some(ticks), Some(seconds)) if seconds > 0.0 => {
                Some((ticks as f32 / seconds).round() as i32)
            }
            _ => None,
        }
    }

    /// Get player information as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - player_name: The player's display name
    /// - steam_id: The player's Steam ID
    /// - hero_id: The player's hero ID
    /// - team: The player's team ("Archmother" or "Hidden King")
    /// - start_lane: The player's original lane (1=left, 4=center, 6=right)
    #[getter]
    fn players(&self) -> PyResult<PyDataFrame> {
        // Parse to the last tick to get final game state
        let last_tick = self.total_ticks.unwrap_or(0);
        let ctx = self.parser.parse_to_tick(last_tick).map_err(to_py_err)?;

        let mut player_names: Vec<String> = Vec::new();
        let mut steam_ids: Vec<u64> = Vec::new();
        let mut hero_ids: Vec<i64> = Vec::new();
        let mut teams: Vec<String> = Vec::new();
        let mut start_lanes: Vec<i64> = Vec::new();

        // Find all CCitadelPlayerController entities
        for (_idx, entity) in ctx.entities.iter() {
            if entity.class_name == "CCitadelPlayerController" {
                // Extract player name
                let player_name = entity
                    .fields
                    .get("m_iszPlayerName")
                    .and_then(|v| match v {
                        boon_parser::FieldValue::String(bytes) => {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                // Extract steam ID
                let steam_id = entity
                    .fields
                    .get("m_steamID")
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(id) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or(0);

                // Skip players with no steam ID
                if steam_id == 0 {
                    continue;
                }

                // Extract hero ID (U64)
                let hero_id = entity
                    .fields
                    .get("m_PlayerDataGlobal.m_nHeroID")
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(id) => Some(*id as i64),
                        boon_parser::FieldValue::I64(id) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or(0);

                // Extract team number (U64) and map to team name
                let team = entity
                    .fields
                    .get("m_iTeamNum")
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(n) => Some(*n),
                        boon_parser::FieldValue::I64(n) => Some(*n as u64),
                        _ => None,
                    })
                    .map(|n| match n {
                        2 => "Hidden King".to_string(),
                        3 => "Archmother".to_string(),
                        _ => format!("Unknown ({})", n),
                    })
                    .unwrap_or_else(|| "Unknown".to_string());

                // Extract original lane assignment (I64)
                // Lane mapping (assuming Hidden King is at the bottom of the map):
                // 1 -> left, 4 -> center, 6 -> right
                let start_lane = entity
                    .fields
                    .get("  ")
                    .and_then(|v| match v {
                        boon_parser::FieldValue::I64(n) => Some(*n),
                        boon_parser::FieldValue::U64(n) => Some(*n as i64),
                        _ => None,
                    })
                    .unwrap_or(0);

                player_names.push(player_name);
                steam_ids.push(steam_id);
                hero_ids.push(hero_id);
                teams.push(team);
                start_lanes.push(start_lane);
            }
        }

        // Build DataFrame
        let df = DataFrame::new(vec![
            Column::new("player_name".into(), player_names),
            Column::new("steam_id".into(), steam_ids),
            Column::new("hero_id".into(), hero_ids),
            Column::new("team".into(), teams),
            Column::new("start_lane".into(), start_lanes),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;

        Ok(PyDataFrame(df))
    }

    /// Get world state at every tick as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick
    /// - is_paused: Whether the game is paused
    /// - next_midboss: Time until next midboss spawn
    fn get_world_ticks(&self) -> PyResult<PyDataFrame> {
        let mut tick_col: Vec<i32> = Vec::new();
        let mut is_paused_col: Vec<bool> = Vec::new();
        let mut next_midboss_col: Vec<f32> = Vec::new();

        // Only track CCitadelGameRulesProxy - skip processing all other entities
        let class_filter: std::collections::HashSet<&str> =
            ["CCitadelGameRulesProxy"].into_iter().collect();

        self.parser
            .run_to_end_filtered(&class_filter, |ctx| {
                // Find CCitadelGameRulesProxy entity
                if let Some((_, entity)) = ctx
                    .entities
                    .iter()
                    .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy")
                {
                    let is_paused = entity
                        .fields
                        .get("m_pGameRules.m_bGamePaused")
                        .and_then(|v| match v {
                            boon_parser::FieldValue::Bool(b) => Some(*b),
                            _ => None,
                        })
                        .unwrap_or(false);

                    let next_midboss = entity
                        .fields
                        .get("m_pGameRules.m_tNextMidBossSpawnTime")
                        .and_then(|v| match v {
                            boon_parser::FieldValue::F32(f) => Some(*f),
                            _ => None,
                        })
                        .unwrap_or(0.0);

                    tick_col.push(ctx.tick);
                    is_paused_col.push(is_paused);
                    next_midboss_col.push(next_midboss);
                }
            })
            .map_err(to_py_err)?;

        let df = DataFrame::new(vec![
            Column::new("tick".into(), tick_col),
            Column::new("is_paused".into(), is_paused_col),
            Column::new("next_midboss".into(), next_midboss_col),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;

        Ok(PyDataFrame(df))
    }

    fn __repr__(&self) -> String {
        let ticks = self.total_ticks.unwrap_or(0);
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }

    fn __str__(&self) -> String {
        let ticks = self.total_ticks.unwrap_or(0);
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }
}

/// Python bindings for the boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add("InvalidDemoError", m.py().get_type::<InvalidDemoError>())?;
    Ok(())
}
