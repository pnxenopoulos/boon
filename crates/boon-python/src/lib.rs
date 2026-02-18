use std::collections::HashMap;
use std::path::PathBuf;

use polars::prelude::*;
use prost::Message;
use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;

pyo3::create_exception!(_boon, InvalidDemoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoHeaderError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoInfoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoMessageError, pyo3::exceptions::PyException);

fn hero_name(id: i64) -> &'static str {
    match id {
        0 => "Base",
        1 => "Infernus",
        2 => "Seven",
        3 => "Vindicta",
        4 => "Lady Geist",
        6 => "Abrams",
        7 => "Wraith",
        8 => "McGinnis",
        10 => "Paradox",
        11 => "Dynamo",
        12 => "Kelvin",
        13 => "Haze",
        14 => "Holliday",
        15 => "Bebop",
        16 => "Calico",
        17 => "Grey Talon",
        18 => "Mo and Krill",
        19 => "Shiv",
        20 => "Ivy",
        21 => "Kali",
        25 => "Warden",
        27 => "Yamato",
        31 => "Lash",
        35 => "Viscous",
        38 => "Gunslinger",
        39 => "The Boss",
        46 => "Generic Person",
        47 => "Tokamak",
        48 => "Wrecker",
        49 => "Rutger",
        50 => "Pocket",
        51 => "Thumper",
        52 => "Mirage",
        53 => "Fathom",
        54 => "Cadence",
        55 => "Target Dummy",
        56 => "Bomber",
        57 => "Shield Guy",
        58 => "Vyper",
        59 => "Vandal",
        60 => "Sinclair",
        61 => "Trapper",
        63 => "Mina",
        64 => "Drifter",
        65 => "Venator",
        66 => "Victor",
        67 => "Paige",
        68 => "Boho",
        69 => "The Doorman",
        70 => "Skyrunner",
        71 => "Swan",
        72 => "Billy",
        73 => "Druid",
        74 => "Graf",
        75 => "Fortuna",
        76 => "Graves",
        77 => "Apollo",
        78 => "Airheart",
        79 => "Rem",
        80 => "Silver",
        81 => "Celeste",
        82 => "Raven",
        _ => "NAME_NOT_FOUND",
    }
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

fn get_f32(e: &boon_parser::Entity, key: Option<u64>) -> f32 {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon_parser::FieldValue::F32(f) => Some(*f),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn get_i64(e: &boon_parser::Entity, key: Option<u64>) -> i64 {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon_parser::FieldValue::U32(n) => Some(*n as i64),
            boon_parser::FieldValue::U64(n) => Some(*n as i64),
            boon_parser::FieldValue::I32(n) => Some(*n as i64),
            boon_parser::FieldValue::I64(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0)
}

fn get_bool(e: &boon_parser::Entity, key: Option<u64>) -> bool {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon_parser::FieldValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false)
}

fn get_qangle(e: &boon_parser::Entity, key: Option<u64>) -> [f32; 3] {
    key.and_then(|k| e.fields.get(&k))
        .and_then(|v| match v {
            boon_parser::FieldValue::QAngle(a) => Some(*a),
            _ => None,
        })
        .unwrap_or([0.0; 3])
}

fn get_handle_index(e: &boon_parser::Entity, key: Option<u64>) -> Option<i32> {
    key.and_then(|k| e.fields.get(&k)).and_then(|v| match v {
        boon_parser::FieldValue::U32(n) => Some((*n & 0x7FFF) as i32),
        boon_parser::FieldValue::U64(n) => Some((*n as u32 & 0x7FFF) as i32),
        boon_parser::FieldValue::I32(n) => Some(*n & 0x7FFF),
        boon_parser::FieldValue::I64(n) => Some((*n as i32) & 0x7FFF),
        _ => None,
    })
}

const VALID_DATASETS: &[&str] = &[
    "player_ticks",
    "world_ticks",
    "kills",
    "damage",
    "flex_slots",
    "respawns",
    "purchases",
];

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
    match_id: u64,
    teams: DataFrame,
    // Sorted ticks where the game was paused (lazily built from world_ticks)
    paused_ticks: Option<Vec<i32>>,
    // Cached dataset DataFrames
    cached_player_ticks: Option<DataFrame>,
    cached_world_ticks: Option<DataFrame>,
    cached_kills: Option<DataFrame>,
    cached_damage: Option<DataFrame>,
    // Game over state: (winning_team_num, tick), None if no event found
    game_over: Option<(i32, i32)>,
    game_over_scanned: bool,
    // Banned heroes: list of hero IDs, None if not yet scanned
    banned_hero_ids: Option<Vec<u32>>,
    banned_heroes_scanned: bool,
    // Flex slot unlock events
    cached_flex_slots: Option<DataFrame>,
    cached_respawns: Option<DataFrame>,
    cached_purchases: Option<DataFrame>,
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
        let build = header
            .build_num
            .ok_or_else(|| DemoHeaderError::new_err("missing build number in file header"))?;
        let map_name = header
            .map_name
            .ok_or_else(|| DemoHeaderError::new_err("missing map name in file header"))?;

        // Parse file info
        let info = parser.file_info().map_err(to_py_err)?;
        let total_ticks = info
            .playback_ticks
            .ok_or_else(|| DemoInfoError::new_err("missing playback ticks in file info"))?;
        let playback_time = info
            .playback_time
            .ok_or_else(|| DemoInfoError::new_err("missing playback time in file info"))?;

        // Parse first tick to get match_id from CCitadelGameRulesProxy
        let ctx = parser.parse_to_tick(1).map_err(to_py_err)?;

        // Resolve the field key for m_pGameRules.m_unMatchID
        let match_id = ctx
            .entities
            .iter()
            .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy")
            .and_then(|(_, e)| {
                let serializer = ctx.serializers.get(&e.class_name)?;
                let key = serializer.resolve_field_key("m_pGameRules.m_unMatchID")?;
                e.fields.get(&key)
            })
            .and_then(|v| match v {
                boon_parser::FieldValue::U64(id) => Some(*id),
                boon_parser::FieldValue::I64(id) => Some(*id as u64),
                _ => None,
            })
            .ok_or_else(|| {
                DemoMessageError::new_err("could not resolve match ID from CCitadelGameRulesProxy")
            })?;

        let teams = DataFrame::new(vec![
            Column::new("team_num".into(), vec![1i64, 2, 3]),
            Column::new(
                "team_name".into(),
                vec!["Spectator", "Hidden King", "Archmother"],
            ),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;

        let tick_rate = if playback_time > 0.0 {
            (total_ticks as f32 / playback_time).round() as i32
        } else {
            0
        };

        Ok(Demo {
            parser,
            path,
            build,
            map_name,
            total_ticks,
            playback_time,
            tick_rate,
            match_id,
            teams,
            paused_ticks: None,
            cached_player_ticks: None,
            cached_world_ticks: None,
            cached_kills: None,
            cached_damage: None,
            game_over: None,
            game_over_scanned: false,
            banned_hero_ids: None,
            banned_heroes_scanned: false,
            cached_flex_slots: None,
            cached_respawns: None,
            cached_purchases: None,
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
    fn path(&self, py: Python<'_>) -> PyResult<PyObject> {
        let pathlib = py.import("pathlib")?;
        let path = pathlib
            .getattr("Path")?
            .call1((self.path.to_string_lossy().to_string(),))?;
        Ok(path.into())
    }

    /// The total number of ticks in the demo.
    #[getter]
    fn total_ticks(&self) -> i32 {
        self.total_ticks
    }

    /// The total duration of the demo in seconds.
    #[getter]
    fn total_seconds(&self) -> f32 {
        self.playback_time
    }

    /// The total duration of the demo as a formatted string (e.g., "12:34").
    #[getter]
    fn total_clock_time(&self) -> String {
        let total_seconds = self.playback_time as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{minutes}:{seconds:02}")
    }

    /// The build number of the game that recorded the demo.
    #[getter]
    fn build(&self) -> i32 {
        self.build
    }

    /// The name of the map the demo was recorded on.
    #[getter]
    fn map_name(&self) -> String {
        self.map_name.clone()
    }

    /// The match ID for this demo.
    #[getter]
    fn match_id(&self) -> u64 {
        self.match_id
    }

    /// The tick rate of the demo (ticks per second).
    #[getter]
    fn tick_rate(&self) -> i32 {
        self.tick_rate
    }

    /// Convert a tick number to seconds elapsed, excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    fn tick_to_seconds(&mut self, tick: i32) -> PyResult<f64> {
        if self.tick_rate == 0 {
            return Ok(0.0);
        }
        self.ensure_paused_ticks_built()?;
        let active_ticks = self.count_active_ticks(tick);
        Ok(active_ticks as f64 / self.tick_rate as f64)
    }

    /// Convert a tick number to a clock time string (e.g., ``"03:14"``),
    /// excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    fn tick_to_clock_time(&mut self, tick: i32) -> PyResult<String> {
        let secs = self.tick_to_seconds(tick)?;
        let total_seconds = secs as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(format!("{minutes}:{seconds:02}"))
    }

    /// Team number to team name mapping as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - team_num: The raw team number (1=Spectator, 2=Hidden King, 3=Archmother)
    /// - team_name: The team name
    #[getter]
    fn teams(&self) -> PyDataFrame {
        PyDataFrame(self.teams.clone())
    }

    /// Get player information as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - player_name: The player's display name
    /// - steam_id: The player's Steam ID
    /// - hero: The player's hero name
    /// - hero_id: The player's hero ID
    /// - team: The player's team ("Archmother", "Hidden King", or "Spectator")
    /// - team_num: The player's raw team number
    /// - start_lane: The player's original lane (1=left, 4=center, 6=right)
    #[getter]
    fn players(&self) -> PyResult<PyDataFrame> {
        // Parse to the last tick to get final game state
        let last_tick = self.total_ticks;
        let ctx = self.parser.parse_to_tick(last_tick).map_err(to_py_err)?;

        let mut player_names: Vec<String> = Vec::new();
        let mut steam_ids: Vec<u64> = Vec::new();
        let mut hero_ids: Vec<i64> = Vec::new();
        let mut heroes: Vec<String> = Vec::new();
        let mut team_nums: Vec<i64> = Vec::new();
        let mut teams: Vec<String> = Vec::new();
        let mut start_lanes: Vec<i64> = Vec::new();

        // Resolve field keys once for CCitadelPlayerController
        let player_serializer = ctx.serializers.get("CCitadelPlayerController");
        let key_player_name = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_iszPlayerName"));
        let key_steam_id = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_steamID"));
        let key_hero_id = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID"));
        let key_team_num = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_iTeamNum"));
        let key_start_lane = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_nOriginalLaneAssignment"));

        // Find all CCitadelPlayerController entities
        for (_idx, entity) in ctx.entities.iter() {
            if entity.class_name == "CCitadelPlayerController" {
                // Extract player name
                let player_name = key_player_name
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::String(bytes) => {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                // Extract steam ID
                let steam_id = key_steam_id
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(id) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or(0);

                // Skip players with no steam ID
                if steam_id == 0 {
                    continue;
                }

                // Extract hero ID and name
                let hero_id = key_hero_id
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(id) => Some(*id as i64),
                        boon_parser::FieldValue::I64(id) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or(0);
                let hero = hero_name(hero_id).to_string();

                // Extract team number and map to team name
                let team_num = key_team_num
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(n) => Some(*n as i64),
                        boon_parser::FieldValue::I64(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);
                let team = match team_num {
                    1 => "Spectator".to_string(),
                    2 => "Hidden King".to_string(),
                    3 => "Archmother".to_string(),
                    _ => "TEAM_NOT_FOUND".to_string(),
                };

                // Extract original lane assignment (I64)
                // Lane mapping (assuming Hidden King is at the bottom of the map):
                // 1 -> left, 4 -> center, 6 -> right
                let start_lane = key_start_lane
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::I64(n) => Some(*n),
                        boon_parser::FieldValue::U64(n) => Some(*n as i64),
                        _ => None,
                    })
                    .unwrap_or(0);

                player_names.push(player_name);
                steam_ids.push(steam_id);
                hero_ids.push(hero_id);
                heroes.push(hero);
                team_nums.push(team_num);
                teams.push(team);
                start_lanes.push(start_lane);
            }
        }

        // Build DataFrame
        let df = DataFrame::new(vec![
            Column::new("player_name".into(), player_names),
            Column::new("steam_id".into(), steam_ids),
            Column::new("hero".into(), heroes),
            Column::new("hero_id".into(), hero_ids),
            Column::new("team".into(), teams),
            Column::new("team_num".into(), team_nums),
            Column::new("start_lane".into(), start_lanes),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;

        Ok(PyDataFrame(df))
    }

    /// Load one or more datasets from the demo file in a single pass.
    ///
    /// Valid dataset names: ``"player_ticks"``, ``"world_ticks"``, ``"kills"``.
    /// Already-loaded datasets are skipped. Multiple datasets requested together
    /// share a single parse pass over the file for efficiency.
    ///
    /// Args:
    ///     *datasets: One or more dataset names to load.
    ///
    /// Raises:
    ///     ValueError: If an unknown dataset name is provided.
    #[pyo3(signature = (*datasets))]
    fn load(&mut self, datasets: Vec<String>) -> PyResult<()> {
        // Validate dataset names
        for name in &datasets {
            if !VALID_DATASETS.contains(&name.as_str()) {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unknown dataset: {name:?}. Valid datasets: {VALID_DATASETS:?}"
                )));
            }
        }

        // Determine what to load (skip already cached)
        let load_player_ticks =
            datasets.iter().any(|s| s == "player_ticks") && self.cached_player_ticks.is_none();
        let load_world_ticks =
            datasets.iter().any(|s| s == "world_ticks") && self.cached_world_ticks.is_none();
        let load_kills = datasets.iter().any(|s| s == "kills") && self.cached_kills.is_none();
        let load_damage = datasets.iter().any(|s| s == "damage") && self.cached_damage.is_none();
        let load_flex_slots =
            datasets.iter().any(|s| s == "flex_slots") && self.cached_flex_slots.is_none();
        let load_respawns =
            datasets.iter().any(|s| s == "respawns") && self.cached_respawns.is_none();
        let load_purchases =
            datasets.iter().any(|s| s == "purchases") && self.cached_purchases.is_none();

        if !load_player_ticks
            && !load_world_ticks
            && !load_kills
            && !load_damage
            && !load_flex_slots
            && !load_respawns
            && !load_purchases
        {
            return Ok(());
        }

        let need_events =
            load_kills || load_damage || load_flex_slots || load_respawns || load_purchases;

        // Build union class filter
        let mut class_names: Vec<&str> = Vec::new();
        if load_player_ticks {
            class_names.push("CCitadelPlayerPawn");
            class_names.push("CCitadelPlayerController");
        }
        if load_world_ticks {
            class_names.push("CCitadelGameRulesProxy");
        }
        if load_kills || load_damage || load_respawns {
            class_names.push("CCitadelPlayerPawn");
        }
        if load_purchases {
            class_names.push("CCitadelPlayerController");
        }
        let class_filter: std::collections::HashSet<&str> = class_names.into_iter().collect();

        // ── Column vectors for player_ticks ──
        let pt_capacity = if load_player_ticks {
            self.total_ticks as usize * 12
        } else {
            0
        };
        let mut pt_tick: Vec<i32> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_id: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_x: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_y: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_z: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_pitch: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_yaw: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_roll: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_in_regen_zone: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_death_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_last_spawn_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_respawn_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_health: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_max_health: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_lifestate: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_souls: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_spent_souls: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_last_dmg: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_last: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_last: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_time_revealed: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_build_id: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_is_alive: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_rebirth: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_rejuvenator: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_ultimate: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_health_regen: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ult_cd_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ult_cd_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ap_nw: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_gold_nw: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_denies: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_damage: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_healing: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_obj_damage: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_self_healing: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_kill_streak: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_last_hits: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_level: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_kills: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_deaths: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_assists: Vec<i64> = Vec::with_capacity(pt_capacity);

        // ── Column vectors for world_ticks ──
        let wt_capacity = if load_world_ticks {
            self.total_ticks as usize
        } else {
            0
        };
        let mut wt_tick: Vec<i32> = Vec::with_capacity(wt_capacity);
        let mut wt_is_paused: Vec<bool> = Vec::with_capacity(wt_capacity);
        let mut wt_next_midboss: Vec<f32> = Vec::with_capacity(wt_capacity);

        // ── Kill / damage event collection ──
        struct RawEvent {
            tick: i32,
            payload: Vec<u8>,
        }
        let mut raw_kill_events: Vec<RawEvent> = Vec::new();
        let mut raw_damage_events: Vec<RawEvent> = Vec::new();
        let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut entity_to_hero_built = false;
        let mut found_game_over: Option<(i32, i32)> = None;
        let mut found_banned_hero_ids: Option<Vec<u32>> = None;
        let mut flex_ticks: Vec<i32> = Vec::new();
        let mut flex_team_nums: Vec<i32> = Vec::new();
        let mut respawn_ticks: Vec<i32> = Vec::new();
        let mut respawn_hero_ids: Vec<i64> = Vec::new();
        let mut purchase_ticks: Vec<i32> = Vec::new();
        let mut purchase_hero_ids: Vec<i64> = Vec::new();
        let mut purchase_ability_ids: Vec<u32> = Vec::new();
        let mut purchase_abilities: Vec<String> = Vec::new();
        let mut purchase_sell: Vec<bool> = Vec::new();
        let mut purchase_quickbuy: Vec<bool> = Vec::new();
        let mut slot_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut slot_to_hero_built = false;

        // ── Field keys ──
        let mut keys_resolved = false;

        // Pawn keys (needed for player_ticks and kills entity_to_hero)
        let mut pk_hero_id: Option<u64> = None;
        let mut pk_vec_x: Option<u64> = None;
        let mut pk_vec_y: Option<u64> = None;
        let mut pk_vec_z: Option<u64> = None;
        let mut pk_camera: Option<u64> = None;
        let mut pk_in_regen: Option<u64> = None;
        let mut pk_death_time: Option<u64> = None;
        let mut pk_last_spawn: Option<u64> = None;
        let mut pk_respawn: Option<u64> = None;
        let mut pk_health: Option<u64> = None;
        let mut pk_max_health: Option<u64> = None;
        let mut pk_lifestate: Option<u64> = None;
        let mut pk_souls: Option<u64> = None;
        let mut pk_spent_souls: Option<u64> = None;
        let mut pk_combat_end: Option<u64> = None;
        let mut pk_combat_last_dmg: Option<u64> = None;
        let mut pk_combat_start: Option<u64> = None;
        let mut pk_dmg_dealt_end: Option<u64> = None;
        let mut pk_dmg_dealt_last: Option<u64> = None;
        let mut pk_dmg_dealt_start: Option<u64> = None;
        let mut pk_dmg_taken_end: Option<u64> = None;
        let mut pk_dmg_taken_last: Option<u64> = None;
        let mut pk_dmg_taken_start: Option<u64> = None;
        let mut pk_time_revealed: Option<u64> = None;
        let mut pk_build_id: Option<u64> = None;

        // Controller keys
        let mut ck_pawn_handle: Option<u64> = None;
        let mut ck_alive: Option<u64> = None;
        let mut ck_rebirth: Option<u64> = None;
        let mut ck_rejuvenator: Option<u64> = None;
        let mut ck_ultimate: Option<u64> = None;
        let mut ck_health_regen: Option<u64> = None;
        let mut ck_ult_cd_end: Option<u64> = None;
        let mut ck_ult_cd_start: Option<u64> = None;
        let mut ck_ap_nw: Option<u64> = None;
        let mut ck_gold_nw: Option<u64> = None;
        let mut ck_denies: Option<u64> = None;
        let mut ck_hero_damage: Option<u64> = None;
        let mut ck_hero_healing: Option<u64> = None;
        let mut ck_obj_damage: Option<u64> = None;
        let mut ck_self_healing: Option<u64> = None;
        let mut ck_kill_streak: Option<u64> = None;
        let mut ck_last_hits: Option<u64> = None;
        let mut ck_level: Option<u64> = None;
        let mut ck_kills: Option<u64> = None;
        let mut ck_deaths: Option<u64> = None;
        let mut ck_assists: Option<u64> = None;

        // Controller hero_id key (for purchases slot→hero mapping)
        let mut ck_hero_id: Option<u64> = None;

        // World keys
        let mut wk_is_paused: Option<u64> = None;
        let mut wk_next_midboss: Option<u64> = None;

        // ── Single-pass callback logic (shared between both code paths) ──
        //
        // We use a macro to avoid duplicating the entity extraction code across
        // the events-aware and entities-only branches.
        macro_rules! collect_entity_data {
            ($ctx:expr) => {
                if !keys_resolved {
                    if load_player_ticks || load_kills || load_damage || load_respawns {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerPawn") {
                            pk_hero_id = s.resolve_field_key(
                                "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID",
                            );
                            if load_player_ticks {
                                pk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                pk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                pk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                pk_camera = s.resolve_field_key("m_angClientCamera");
                                pk_in_regen = s.resolve_field_key("m_bInRegenerationZone");
                                pk_death_time = s.resolve_field_key("m_flDeathTime");
                                pk_last_spawn = s.resolve_field_key("m_flLastSpawnTime");
                                pk_respawn = s.resolve_field_key("m_flRespawnTime");
                                pk_health = s.resolve_field_key("m_iHealth");
                                pk_max_health = s.resolve_field_key("m_iMaxHealth");
                                pk_lifestate = s.resolve_field_key("m_lifeState");
                                pk_souls = s.resolve_field_key("m_nCurrencies.m_nCurrencies");
                                pk_spent_souls =
                                    s.resolve_field_key("m_nSpentCurrencies.m_nSpentCurrencies");
                                pk_combat_end = s.resolve_field_key("m_sInCombat.m_flEndTime");
                                pk_combat_last_dmg =
                                    s.resolve_field_key("m_sInCombat.m_flLastDamageTime");
                                pk_combat_start = s.resolve_field_key("m_sInCombat.m_flStartTime");
                                pk_dmg_dealt_end =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flEndTime");
                                pk_dmg_dealt_last =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flLastDamageTime");
                                pk_dmg_dealt_start =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flStartTime");
                                pk_dmg_taken_end =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flEndTime");
                                pk_dmg_taken_last =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flLastDamageTime");
                                pk_dmg_taken_start =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flStartTime");
                                pk_time_revealed =
                                    s.resolve_field_key("m_timeRevealedOnMinimapByNPC");
                                pk_build_id = s.resolve_field_key("m_unHeroBuildID");
                            }
                        }
                    }
                    if load_player_ticks {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerController") {
                            ck_pawn_handle = s.resolve_field_key("m_hPawn");
                            ck_alive = s.resolve_field_key("m_PlayerDataGlobal.m_bAlive");
                            ck_rebirth =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bHasRebirth");
                            ck_rejuvenator =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bHasRejuvenator");
                            ck_ultimate =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bUltimateTrained");
                            ck_health_regen =
                                s.resolve_field_key("m_PlayerDataGlobal.m_flHealthRegen");
                            ck_ult_cd_end = s
                                .resolve_field_key("m_PlayerDataGlobal.m_flUltimateCooldownEnd");
                            ck_ult_cd_start = s.resolve_field_key(
                                "m_PlayerDataGlobal.m_flUltimateCooldownStart",
                            );
                            ck_ap_nw =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iAPNetWorth");
                            ck_gold_nw =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iGoldNetWorth");
                            ck_denies = s.resolve_field_key("m_PlayerDataGlobal.m_iDenies");
                            ck_hero_damage =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iHeroDamage");
                            ck_hero_healing =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iHeroHealing");
                            ck_obj_damage =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iObjectiveDamage");
                            ck_self_healing =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iSelfHealing");
                            ck_kill_streak =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iKillStreak");
                            ck_last_hits =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iLastHits");
                            ck_level = s.resolve_field_key("m_PlayerDataGlobal.m_iLevel");
                            ck_kills =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iPlayerKills");
                            ck_deaths = s.resolve_field_key("m_PlayerDataGlobal.m_iDeaths");
                            ck_assists =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iPlayerAssists");
                        }
                    }
                    if load_purchases {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerController") {
                            ck_hero_id =
                                s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                        }
                    }
                    if load_world_ticks {
                        if let Some(s) = $ctx.serializers.get("CCitadelGameRulesProxy") {
                            wk_is_paused =
                                s.resolve_field_key("m_pGameRules.m_bGamePaused");
                            wk_next_midboss =
                                s.resolve_field_key("m_pGameRules.m_tNextMidBossSpawnTime");
                        }
                    }
                    keys_resolved = true;
                }

                // ── Collect player_ticks ──
                if load_player_ticks {
                    let controllers: Vec<&boon_parser::Entity> = $ctx
                        .entities
                        .iter()
                        .filter(|(_, e)| e.class_name == "CCitadelPlayerController")
                        .map(|(_, e)| e)
                        .collect();

                    for ctrl in &controllers {
                        let pawn_index = match get_handle_index(ctrl, ck_pawn_handle) {
                            Some(idx) => idx,
                            None => continue,
                        };

                        let pawn = match $ctx.entities.get(pawn_index) {
                            Some(p) if p.class_name == "CCitadelPlayerPawn" => p,
                            _ => continue,
                        };

                        let hid = get_i64(pawn, pk_hero_id);
                        if hid == 0 {
                            continue;
                        }

                        pt_tick.push($ctx.tick);
                        pt_hero_id.push(hid);
                        pt_x.push(get_f32(pawn, pk_vec_x));
                        pt_y.push(get_f32(pawn, pk_vec_y));
                        pt_z.push(get_f32(pawn, pk_vec_z));
                        let angles = get_qangle(pawn, pk_camera);
                        pt_pitch.push(angles[0]);
                        pt_yaw.push(angles[1]);
                        pt_roll.push(angles[2]);
                        pt_in_regen_zone.push(get_bool(pawn, pk_in_regen));
                        pt_death_time.push(get_f32(pawn, pk_death_time));
                        pt_last_spawn_time.push(get_f32(pawn, pk_last_spawn));
                        pt_respawn_time.push(get_f32(pawn, pk_respawn));
                        pt_health.push(get_i64(pawn, pk_health));
                        pt_max_health.push(get_i64(pawn, pk_max_health));
                        pt_lifestate.push(get_i64(pawn, pk_lifestate));
                        pt_souls.push(get_i64(pawn, pk_souls));
                        pt_spent_souls.push(get_i64(pawn, pk_spent_souls));
                        pt_combat_end.push(get_f32(pawn, pk_combat_end));
                        pt_combat_last_dmg.push(get_f32(pawn, pk_combat_last_dmg));
                        pt_combat_start.push(get_f32(pawn, pk_combat_start));
                        pt_dmg_dealt_end.push(get_f32(pawn, pk_dmg_dealt_end));
                        pt_dmg_dealt_last.push(get_f32(pawn, pk_dmg_dealt_last));
                        pt_dmg_dealt_start.push(get_f32(pawn, pk_dmg_dealt_start));
                        pt_dmg_taken_end.push(get_f32(pawn, pk_dmg_taken_end));
                        pt_dmg_taken_last.push(get_f32(pawn, pk_dmg_taken_last));
                        pt_dmg_taken_start.push(get_f32(pawn, pk_dmg_taken_start));
                        pt_time_revealed.push(get_f32(pawn, pk_time_revealed));
                        pt_build_id.push(get_i64(pawn, pk_build_id));
                        pt_is_alive.push(get_bool(ctrl, ck_alive));
                        pt_has_rebirth.push(get_bool(ctrl, ck_rebirth));
                        pt_has_rejuvenator.push(get_bool(ctrl, ck_rejuvenator));
                        pt_has_ultimate.push(get_bool(ctrl, ck_ultimate));
                        pt_health_regen.push(get_f32(ctrl, ck_health_regen));
                        // Note: column start → field CooldownEnd, column end → field CooldownStart
                        pt_ult_cd_start.push(get_f32(ctrl, ck_ult_cd_end));
                        pt_ult_cd_end.push(get_f32(ctrl, ck_ult_cd_start));
                        pt_ap_nw.push(get_i64(ctrl, ck_ap_nw));
                        pt_gold_nw.push(get_i64(ctrl, ck_gold_nw));
                        pt_denies.push(get_i64(ctrl, ck_denies));
                        pt_hero_damage.push(get_i64(ctrl, ck_hero_damage));
                        pt_hero_healing.push(get_i64(ctrl, ck_hero_healing));
                        pt_obj_damage.push(get_i64(ctrl, ck_obj_damage));
                        pt_self_healing.push(get_i64(ctrl, ck_self_healing));
                        pt_kill_streak.push(get_i64(ctrl, ck_kill_streak));
                        pt_last_hits.push(get_i64(ctrl, ck_last_hits));
                        pt_level.push(get_i64(ctrl, ck_level));
                        pt_kills.push(get_i64(ctrl, ck_kills));
                        pt_deaths.push(get_i64(ctrl, ck_deaths));
                        pt_assists.push(get_i64(ctrl, ck_assists));
                    }
                }

                // ── Collect world_ticks ──
                if load_world_ticks {
                    if let Some((_, entity)) = $ctx
                        .entities
                        .iter()
                        .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy")
                    {
                        wt_tick.push($ctx.tick);
                        wt_is_paused.push(get_bool(entity, wk_is_paused));
                        wt_next_midboss.push(get_f32(entity, wk_next_midboss));
                    }
                }

                // ── Build entity_to_hero map (for kills/damage resolution) ──
                if (load_kills || load_damage || load_respawns) && !entity_to_hero_built {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name == "CCitadelPlayerPawn" {
                            let hid = get_i64(entity, pk_hero_id);
                            if hid != 0 {
                                entity_to_hero.insert(idx, hid);
                            }
                        }
                    }
                    entity_to_hero_built = true;
                }

                // ── Build slot_to_hero map (for purchases: userid → hero_id) ──
                if load_purchases && !slot_to_hero_built {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name == "CCitadelPlayerController" {
                            let hid = get_i64(entity, ck_hero_id);
                            if hid != 0 {
                                // userid is 0-based, controller entity index is 1-based
                                slot_to_hero.insert(idx - 1, hid);
                            }
                        }
                    }
                    slot_to_hero_built = true;
                }
            };
        }

        // ── Run the parse pass ──
        if need_events {
            self.parser
                .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
                    collect_entity_data!(ctx);

                    for event in events {
                        if load_kills && event.msg_type == 319 {
                            raw_kill_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        if load_damage && event.msg_type == 300 {
                            raw_damage_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        // Always capture GameOver (msg_type 346)
                        if found_game_over.is_none()
                            && event.msg_type == 346
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMessageGameOver::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            found_game_over =
                                Some((msg.winning_team.unwrap_or(0), event.tick));
                        }
                        // Always capture BannedHeroes (msg_type 366)
                        if found_banned_hero_ids.is_none()
                            && event.msg_type == 366
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            found_banned_hero_ids = Some(msg.banned_hero_ids);
                        }
                        // Collect FlexSlotUnlocked events (msg_type 356)
                        if load_flex_slots
                            && event.msg_type == 356
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgFlexSlotUnlocked::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            flex_ticks.push(event.tick);
                            flex_team_nums.push(msg.team_number.unwrap_or(0));
                        }
                        // Collect PlayerRespawned events (msg_type 353)
                        if load_respawns
                            && event.msg_type == 353
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgPlayerRespawned::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let pawn_idx = (msg.player_pawn.unwrap_or(0) & 0x7FFF) as i32;
                            let hero_id = entity_to_hero
                                .get(&pawn_idx)
                                .copied()
                                .unwrap_or(0);
                            if hero_id != 0 {
                                respawn_ticks.push(event.tick);
                                respawn_hero_ids.push(hero_id);
                            }
                        }
                        // Collect ItemPurchaseNotification events (msg_type 360)
                        if load_purchases
                            && event.msg_type == 360
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMessageItemPurchaseNotification::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let userid = msg.userid.unwrap_or(-1);
                            let hero_id = slot_to_hero
                                .get(&userid)
                                .copied()
                                .unwrap_or(0);
                            let ability_id = msg.ability_id.unwrap_or(0);
                            purchase_ticks.push(event.tick);
                            purchase_hero_ids.push(hero_id);
                            purchase_ability_ids.push(ability_id);
                            purchase_abilities.push(
                                boon_parser::ability_name(ability_id).to_string(),
                            );
                            purchase_sell.push(msg.sell.unwrap_or(false));
                            purchase_quickbuy.push(msg.quickbuy.unwrap_or(false));
                        }
                    }
                })
                .map_err(to_py_err)?;
        } else {
            self.parser
                .run_to_end_filtered(&class_filter, |ctx| {
                    collect_entity_data!(ctx);
                })
                .map_err(to_py_err)?;
        }

        // ── Store always-scanned events if found during events pass ──
        if need_events {
            if !self.game_over_scanned {
                self.game_over = found_game_over;
                self.game_over_scanned = true;
            }
            if !self.banned_heroes_scanned {
                self.banned_hero_ids = found_banned_hero_ids;
                self.banned_heroes_scanned = true;
            }
        }

        // ── Build and cache DataFrames ──

        if load_player_ticks {
            let df = DataFrame::new(vec![
                Column::new("tick".into(), pt_tick),
                Column::new("hero_id".into(), pt_hero_id),
                Column::new("x".into(), pt_x),
                Column::new("y".into(), pt_y),
                Column::new("z".into(), pt_z),
                Column::new("pitch".into(), pt_pitch),
                Column::new("yaw".into(), pt_yaw),
                Column::new("roll".into(), pt_roll),
                Column::new("in_regen_zone".into(), pt_in_regen_zone),
                Column::new("death_time".into(), pt_death_time),
                Column::new("last_spawn_time".into(), pt_last_spawn_time),
                Column::new("respawn_time".into(), pt_respawn_time),
                Column::new("health".into(), pt_health),
                Column::new("max_health".into(), pt_max_health),
                Column::new("lifestate".into(), pt_lifestate),
                Column::new("souls".into(), pt_souls),
                Column::new("spent_souls".into(), pt_spent_souls),
                Column::new("in_combat_end_time".into(), pt_combat_end),
                Column::new("in_combat_last_damage_time".into(), pt_combat_last_dmg),
                Column::new("in_combat_start_time".into(), pt_combat_start),
                Column::new("player_damage_dealt_end_time".into(), pt_dmg_dealt_end),
                Column::new(
                    "player_damage_dealt_last_damage_time".into(),
                    pt_dmg_dealt_last,
                ),
                Column::new("player_damage_dealt_start_time".into(), pt_dmg_dealt_start),
                Column::new("player_damage_taken_end_time".into(), pt_dmg_taken_end),
                Column::new(
                    "player_damage_taken_last_damage_time".into(),
                    pt_dmg_taken_last,
                ),
                Column::new("player_damage_taken_start_time".into(), pt_dmg_taken_start),
                Column::new("time_revealed_by_npc".into(), pt_time_revealed),
                Column::new("build_id".into(), pt_build_id),
                Column::new("is_alive".into(), pt_is_alive),
                Column::new("has_rebirth".into(), pt_has_rebirth),
                Column::new("has_rejuvenator".into(), pt_has_rejuvenator),
                Column::new("has_ultimate_trained".into(), pt_has_ultimate),
                Column::new("health_regen".into(), pt_health_regen),
                Column::new("ultimate_cooldown_start".into(), pt_ult_cd_start),
                Column::new("ultimate_cooldown_end".into(), pt_ult_cd_end),
                Column::new("ap_net_worth".into(), pt_ap_nw),
                Column::new("gold_net_worth".into(), pt_gold_nw),
                Column::new("denies".into(), pt_denies),
                Column::new("hero_damage".into(), pt_hero_damage),
                Column::new("hero_healing".into(), pt_hero_healing),
                Column::new("objective_damage".into(), pt_obj_damage),
                Column::new("self_healing".into(), pt_self_healing),
                Column::new("kill_streak".into(), pt_kill_streak),
                Column::new("last_hits".into(), pt_last_hits),
                Column::new("level".into(), pt_level),
                Column::new("kills".into(), pt_kills),
                Column::new("deaths".into(), pt_deaths),
                Column::new("assists".into(), pt_assists),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_player_ticks = Some(df);
        }

        if load_world_ticks {
            let df = DataFrame::new(vec![
                Column::new("tick".into(), wt_tick),
                Column::new("is_paused".into(), wt_is_paused),
                Column::new("next_midboss".into(), wt_next_midboss),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_world_ticks = Some(df);
        }

        if load_kills {
            // Decode raw kill events and resolve entity indices to hero IDs
            let n = raw_kill_events.len();
            let mut kill_tick: Vec<i32> = Vec::with_capacity(n);
            let mut victim_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut attacker_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut assister_builder = ListPrimitiveChunkedBuilder::<Int64Type>::new(
                "assister_hero_ids".into(),
                n,
                4,
                DataType::Int64,
            );

            for raw in &raw_kill_events {
                let msg =
                    boon_proto::proto::CCitadelUserMsgHeroKilled::decode(raw.payload.as_slice())
                        .map_err(|e| {
                            DemoMessageError::new_err(format!(
                                "Failed to decode HeroKilled event: {e}"
                            ))
                        })?;

                kill_tick.push(raw.tick);
                victim_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_victim.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                attacker_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_attacker.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );

                let assister_ids: Vec<i64> = msg
                    .entindex_assisters
                    .iter()
                    .filter_map(|idx| entity_to_hero.get(idx).copied())
                    .collect();
                assister_builder.append_slice(&assister_ids);
            }

            let assister_series = assister_builder.finish().into_column();
            let df = DataFrame::new(vec![
                Column::new("tick".into(), kill_tick),
                Column::new("victim_hero_id".into(), victim_hero_id),
                Column::new("attacker_hero_id".into(), attacker_hero_id),
                assister_series,
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_kills = Some(df);
        }

        if load_damage {
            // Decode raw damage events and resolve entity indices to hero IDs
            let n = raw_damage_events.len();
            let mut dmg_tick: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_damage: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_pre_damage: Vec<f32> = Vec::with_capacity(n);
            let mut dmg_victim_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut dmg_attacker_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut dmg_victim_health_new: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_hitgroup_id: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_crit_damage: Vec<f32> = Vec::with_capacity(n);
            let mut dmg_attacker_class: Vec<u32> = Vec::with_capacity(n);
            let mut dmg_victim_class: Vec<u32> = Vec::with_capacity(n);

            for raw in &raw_damage_events {
                let msg =
                    boon_proto::proto::CCitadelUserMessageDamage::decode(raw.payload.as_slice())
                        .map_err(|e| {
                            DemoMessageError::new_err(format!("Failed to decode Damage event: {e}"))
                        })?;

                dmg_tick.push(raw.tick);
                dmg_damage.push(msg.damage.unwrap_or(0));
                dmg_pre_damage.push(msg.pre_damage.unwrap_or(0.0));
                dmg_victim_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_victim.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                dmg_attacker_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_attacker.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                dmg_victim_health_new.push(msg.victim_health_new.unwrap_or(0));
                dmg_hitgroup_id.push(msg.hitgroup_id.unwrap_or(0));
                dmg_crit_damage.push(msg.crit_damage.unwrap_or(0.0));
                dmg_attacker_class.push(msg.attacker_class.unwrap_or(0));
                dmg_victim_class.push(msg.victim_class.unwrap_or(0));
            }

            let df = DataFrame::new(vec![
                Column::new("tick".into(), dmg_tick),
                Column::new("damage".into(), dmg_damage),
                Column::new("pre_damage".into(), dmg_pre_damage),
                Column::new("victim_hero_id".into(), dmg_victim_hero_id),
                Column::new("attacker_hero_id".into(), dmg_attacker_hero_id),
                Column::new("victim_health_new".into(), dmg_victim_health_new),
                Column::new("hitgroup_id".into(), dmg_hitgroup_id),
                Column::new("crit_damage".into(), dmg_crit_damage),
                Column::new("attacker_class".into(), dmg_attacker_class),
                Column::new("victim_class".into(), dmg_victim_class),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_damage = Some(df);
        }

        if load_flex_slots {
            let df = DataFrame::new(vec![
                Column::new("tick".into(), flex_ticks),
                Column::new("team_num".into(), flex_team_nums),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_flex_slots = Some(df);
        }

        if load_respawns {
            let df = DataFrame::new(vec![
                Column::new("tick".into(), respawn_ticks),
                Column::new("hero_id".into(), respawn_hero_ids),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_respawns = Some(df);
        }

        if load_purchases {
            let df = DataFrame::new(vec![
                Column::new("tick".into(), purchase_ticks),
                Column::new("hero_id".into(), purchase_hero_ids),
                Column::new("ability_id".into(), purchase_ability_ids),
                Column::new("ability".into(), purchase_abilities),
                Column::new("sell".into(), purchase_sell),
                Column::new("quickbuy".into(), purchase_quickbuy),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_purchases = Some(df);
        }

        Ok(())
    }

    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns a DataFrame with 50 columns covering position, health, combat
    /// timers, kills, deaths, net worth, and more for every player at every tick.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn player_ticks(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_player_ticks.is_none() {
            self.load(vec!["player_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_player_ticks.clone().unwrap()))
    }

    /// World state at every tick as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``is_paused``, ``next_midboss``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn world_ticks(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_world_ticks.is_none() {
            self.load(vec!["world_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_world_ticks.clone().unwrap()))
    }

    /// Hero kill events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the kill occurred
    /// - victim_hero_id: The hero ID of the killed player
    /// - attacker_hero_id: The hero ID of the attacker
    /// - assister_hero_ids: List of hero IDs of players who assisted
    ///
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn kills(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_kills.is_none() {
            self.load(vec!["kills".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_kills.clone().unwrap()))
    }

    /// Damage events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the damage occurred
    /// - damage: The damage dealt
    /// - pre_damage: The damage before mitigation
    /// - victim_hero_id: The hero ID of the victim (0 if not a hero)
    /// - attacker_hero_id: The hero ID of the attacker (0 if not a hero)
    /// - victim_health_new: The victim's health after damage
    /// - hitgroup_id: The hitgroup that was hit
    /// - crit_damage: Critical damage amount
    /// - attacker_class: The attacker's entity class ID
    /// - victim_class: The victim's entity class ID
    ///
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn damage(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_damage.is_none() {
            self.load(vec!["damage".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_damage.clone().unwrap()))
    }

    /// Flex slot unlock events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn flex_slots(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_flex_slots.is_none() {
            self.load(vec!["flex_slots".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_flex_slots.clone().unwrap()))
    }

    /// Player respawn events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn respawns(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_respawns.is_none() {
            self.load(vec!["respawns".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_respawns.clone().unwrap()))
    }

    /// Item purchase events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``ability``, ``sell``, ``quickbuy``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn purchases(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_purchases.is_none() {
            self.load(vec!["purchases".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_purchases.clone().unwrap()))
    }

    /// The team number of the winning team.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn winning_team_num(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(team, _)| team))
    }

    /// The tick when the game ended.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn game_over_tick(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(_, tick)| tick))
    }

    /// The name of the winning team.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn winning_team(&mut self) -> PyResult<Option<String>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(team, _)| match team {
            1 => "Spectator".to_string(),
            2 => "Hidden King".to_string(),
            3 => "Archmother".to_string(),
            _ => "TEAM_NOT_FOUND".to_string(),
        }))
    }

    /// List of banned hero IDs.
    ///
    /// Scans for the ``k_EUserMsg_BannedHeroes`` event on first access.
    /// Returns an empty list if no banned heroes event was found.
    #[getter]
    fn banned_hero_ids(&mut self) -> PyResult<Vec<u32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.banned_hero_ids.clone().unwrap_or_default())
    }

    /// List of banned hero names.
    ///
    /// Scans for the ``k_EUserMsg_BannedHeroes`` event on first access.
    /// Returns an empty list if no banned heroes event was found.
    #[getter]
    fn banned_heroes(&mut self) -> PyResult<Vec<String>> {
        self.ensure_always_events_scanned()?;
        Ok(self
            .banned_hero_ids
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .map(|&id| hero_name(id as i64).to_string())
                    .collect()
            })
            .unwrap_or_default())
    }

    fn __repr__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }

    fn __str__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }
}

impl Demo {
    /// Build the paused_ticks cache from world_ticks if not already done.
    fn ensure_paused_ticks_built(&mut self) -> PyResult<()> {
        if self.paused_ticks.is_some() {
            return Ok(());
        }
        // Ensure world_ticks is loaded
        if self.cached_world_ticks.is_none() {
            self.load(vec!["world_ticks".to_string()])?;
        }
        let wt = self.cached_world_ticks.as_ref().unwrap();
        let tick_col = wt.column("tick").unwrap();
        let paused_col = wt.column("is_paused").unwrap();
        let ticks = tick_col.i32().unwrap();
        let paused = paused_col.bool().unwrap();

        let mut paused_ticks = Vec::new();
        for i in 0..ticks.len() {
            if paused.get(i).unwrap_or(false) {
                paused_ticks.push(ticks.get(i).unwrap());
            }
        }
        self.paused_ticks = Some(paused_ticks);
        Ok(())
    }

    /// Count non-paused ticks up to the given tick.
    fn count_active_ticks(&self, tick: i32) -> i32 {
        let paused = self
            .paused_ticks
            .as_ref()
            .map(|pts| pts.partition_point(|&t| t < tick) as i32)
            .unwrap_or(0);
        (tick - paused).max(0)
    }

    /// Scan for always-needed events (GameOver, BannedHeroes) if not already done.
    /// Uses the lightweight events-only parser pass.
    fn ensure_always_events_scanned(&mut self) -> PyResult<()> {
        let need_game_over = !self.game_over_scanned;
        let need_banned = !self.banned_heroes_scanned;
        if !need_game_over && !need_banned {
            return Ok(());
        }
        let events = self.parser.events(None).map_err(to_py_err)?;
        for event in &events {
            if need_game_over
                && event.msg_type == 346
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMessageGameOver::decode(event.payload.as_slice())
            {
                self.game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
            }
            if need_banned
                && event.msg_type == 366
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(event.payload.as_slice())
            {
                self.banned_hero_ids = Some(msg.banned_hero_ids);
            }
        }
        self.game_over_scanned = true;
        self.banned_heroes_scanned = true;
        Ok(())
    }
}

/// Python bindings for the boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add("InvalidDemoError", m.py().get_type::<InvalidDemoError>())?;
    m.add("DemoHeaderError", m.py().get_type::<DemoHeaderError>())?;
    m.add("DemoInfoError", m.py().get_type::<DemoInfoError>())?;
    m.add("DemoMessageError", m.py().get_type::<DemoMessageError>())?;
    Ok(())
}
