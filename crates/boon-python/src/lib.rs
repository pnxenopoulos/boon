use std::collections::HashMap;
use std::path::PathBuf;

use boon_proto::proto::CitadelUserMessageIds as Msg;
use polars::prelude::*;
use prost::Message;
use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;

/// Source 2 null entity handle (0x00FFFFFF).
const INVALID_ENTITY_HANDLE: u32 = 0x00FF_FFFF;

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
    "abilities",
    "ability_upgrades",
    "boss_kills",
    "chat",
    "mid_boss",
    "objectives",
    "player_ticks",
    "world_ticks",
    "kills",
    "damage",
    "flex_slots",
    "item_purchases",
    "troopers",
    "neutrals",
    "stat_modifier_events",
    "active_modifiers",
    "urn",
];

const VALID_STREET_BRAWL_DATASETS: &[&str] = &["street_brawl_ticks", "street_brawl_rounds"];

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
    game_mode: i64,
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
    // Flex slot unlock events
    cached_flex_slots: Option<DataFrame>,
    cached_abilities: Option<DataFrame>,
    cached_ability_upgrades: Option<DataFrame>,
    cached_item_purchases: Option<DataFrame>,
    cached_chat: Option<DataFrame>,
    cached_objectives: Option<DataFrame>,
    cached_boss_kills: Option<DataFrame>,
    cached_mid_boss: Option<DataFrame>,
    cached_troopers: Option<DataFrame>,
    cached_neutrals: Option<DataFrame>,
    cached_stat_modifier_events: Option<DataFrame>,
    cached_active_modifiers: Option<DataFrame>,
    cached_players: Option<DataFrame>,
    cached_street_brawl_ticks: Option<DataFrame>,
    cached_street_brawl_rounds: Option<DataFrame>,
    cached_urn: Option<DataFrame>,
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

        // Resolve match_id and game_mode from CCitadelGameRulesProxy
        let game_rules = ctx
            .entities
            .iter()
            .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy");
        let (match_id, game_mode) = game_rules
            .and_then(|(_, e)| {
                let serializer = ctx.serializers.get(&e.class_name)?;
                let mid_key = serializer.resolve_field_key("m_pGameRules.m_unMatchID")?;
                let mid = match e.fields.get(&mid_key)? {
                    boon_parser::FieldValue::U64(id) => *id,
                    boon_parser::FieldValue::I64(id) => *id as u64,
                    _ => return None,
                };
                let gm = serializer
                    .resolve_field_key("m_pGameRules.m_eGameMode")
                    .and_then(|k| e.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(n) => Some(*n as i64),
                        boon_parser::FieldValue::I64(n) => Some(*n),
                        boon_parser::FieldValue::U32(n) => Some(*n as i64),
                        boon_parser::FieldValue::I32(n) => Some(*n as i64),
                        _ => None,
                    })
                    .unwrap_or(0);
                Some((mid, gm))
            })
            .ok_or_else(|| {
                DemoMessageError::new_err("could not resolve match ID from CCitadelGameRulesProxy")
            })?;

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
            game_mode,
            paused_ticks: None,
            cached_player_ticks: None,
            cached_world_ticks: None,
            cached_kills: None,
            cached_damage: None,
            game_over: None,
            game_over_scanned: false,
            cached_abilities: None,
            cached_flex_slots: None,
            cached_ability_upgrades: None,
            cached_item_purchases: None,
            cached_chat: None,
            cached_objectives: None,
            cached_boss_kills: None,
            cached_mid_boss: None,
            cached_troopers: None,
            cached_neutrals: None,
            cached_stat_modifier_events: None,
            cached_active_modifiers: None,
            cached_players: None,
            cached_street_brawl_ticks: None,
            cached_street_brawl_rounds: None,
            cached_urn: None,
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
    fn path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pathlib = py.import("pathlib")?;
        let path = pathlib
            .getattr("Path")?
            .call1((self.path.to_string_lossy().to_string(),))?;
        Ok(path.unbind())
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

    /// The game mode ID for this demo.
    ///
    /// Use ``game_mode_names()`` to resolve IDs to names.
    #[getter]
    fn game_mode(&self) -> i64 {
        self.game_mode
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

    /// Get player information as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - player_name: The player's display name
    /// - steam_id: The player's Steam ID
    /// - hero_id: The player's hero ID
    /// - team_num: The player's raw team number
    /// - start_lane: The player's original lane (1=left, 4=center, 6=right)
    #[getter]
    fn players(&mut self) -> PyResult<PyDataFrame> {
        if let Some(ref df) = self.cached_players {
            return Ok(PyDataFrame(df.clone()));
        }

        // Parse to the last tick to get final game state
        let last_tick = self.total_ticks;
        let ctx = self.parser.parse_to_tick(last_tick).map_err(to_py_err)?;

        let mut player_names: Vec<String> = Vec::new();
        let mut steam_ids: Vec<u64> = Vec::new();
        let mut hero_ids: Vec<i64> = Vec::new();
        let mut team_nums: Vec<i64> = Vec::new();
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
                // Extract team number
                let team_num = key_team_num
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(n) => Some(*n as i64),
                        boon_parser::FieldValue::I64(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);

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
                team_nums.push(team_num);
                start_lanes.push(start_lane);
            }
        }

        // Build DataFrame
        let df = df_from_columns(vec![
            Column::new("player_name".into(), player_names),
            Column::new("steam_id".into(), steam_ids),
            Column::new("hero_id".into(), hero_ids),
            Column::new("team_num".into(), team_nums),
            Column::new("start_lane".into(), start_lanes),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;

        self.cached_players = Some(df.clone());
        Ok(PyDataFrame(df))
    }

    /// Return the list of dataset names that can be passed to ``load()`` or accessed as properties.
    ///
    /// Returns:
    ///     A list of valid dataset name strings.
    #[staticmethod]
    fn available_datasets() -> Vec<&'static str> {
        let mut all = VALID_DATASETS.to_vec();
        all.extend_from_slice(VALID_STREET_BRAWL_DATASETS);
        all
    }

    /// Load one or more datasets from the demo file in a single pass.
    ///
    /// Valid dataset names: see ``available_datasets()``.
    /// Already-loaded datasets are skipped. Multiple datasets requested together
    /// share a single parse pass over the file for efficiency.
    ///
    /// Args:
    ///     *datasets: One or more dataset names to load.
    ///
    /// Raises:
    ///     ValueError: If an unknown dataset name is provided.
    ///     NotStreetBrawlError: If a street brawl dataset is requested on a non-street-brawl demo.
    #[pyo3(signature = (*datasets))]
    fn load(&mut self, datasets: Vec<String>) -> PyResult<()> {
        // Validate dataset names
        for name in &datasets {
            if !VALID_DATASETS.contains(&name.as_str())
                && !VALID_STREET_BRAWL_DATASETS.contains(&name.as_str())
            {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unknown dataset: {name:?}. Valid datasets: {VALID_DATASETS:?}"
                )));
            }
        }

        // Check game mode for street brawl datasets
        if datasets
            .iter()
            .any(|s| VALID_STREET_BRAWL_DATASETS.contains(&s.as_str()))
            && self.game_mode != 4
        {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }

        // Determine what to load (skip already cached)
        let load_abilities =
            datasets.iter().any(|s| s == "abilities") && self.cached_abilities.is_none();
        let load_player_ticks =
            datasets.iter().any(|s| s == "player_ticks") && self.cached_player_ticks.is_none();
        let load_world_ticks =
            datasets.iter().any(|s| s == "world_ticks") && self.cached_world_ticks.is_none();
        let load_kills = datasets.iter().any(|s| s == "kills") && self.cached_kills.is_none();
        let load_damage = datasets.iter().any(|s| s == "damage") && self.cached_damage.is_none();
        let load_flex_slots =
            datasets.iter().any(|s| s == "flex_slots") && self.cached_flex_slots.is_none();
        let load_ability_upgrades = datasets.iter().any(|s| s == "ability_upgrades")
            && self.cached_ability_upgrades.is_none();
        let load_item_purchases =
            datasets.iter().any(|s| s == "item_purchases") && self.cached_item_purchases.is_none();
        let load_chat = datasets.iter().any(|s| s == "chat") && self.cached_chat.is_none();
        let load_objectives =
            datasets.iter().any(|s| s == "objectives") && self.cached_objectives.is_none();
        let load_boss_kills =
            datasets.iter().any(|s| s == "boss_kills") && self.cached_boss_kills.is_none();
        let load_mid_boss =
            datasets.iter().any(|s| s == "mid_boss") && self.cached_mid_boss.is_none();
        let load_troopers =
            datasets.iter().any(|s| s == "troopers") && self.cached_troopers.is_none();
        let load_neutrals =
            datasets.iter().any(|s| s == "neutrals") && self.cached_neutrals.is_none();
        let load_stat_modifier_events = datasets.iter().any(|s| s == "stat_modifier_events")
            && self.cached_stat_modifier_events.is_none();
        let load_active_modifiers = datasets.iter().any(|s| s == "active_modifiers")
            && self.cached_active_modifiers.is_none();
        let load_urn = datasets.iter().any(|s| s == "urn") && self.cached_urn.is_none();
        let load_street_brawl_ticks = datasets.iter().any(|s| s == "street_brawl_ticks")
            && self.cached_street_brawl_ticks.is_none();
        let load_street_brawl_rounds = datasets.iter().any(|s| s == "street_brawl_rounds")
            && self.cached_street_brawl_rounds.is_none();

        if !load_abilities
            && !load_player_ticks
            && !load_world_ticks
            && !load_kills
            && !load_damage
            && !load_flex_slots
            && !load_ability_upgrades
            && !load_item_purchases
            && !load_chat
            && !load_objectives
            && !load_boss_kills
            && !load_mid_boss
            && !load_troopers
            && !load_neutrals
            && !load_stat_modifier_events
            && !load_active_modifiers
            && !load_urn
            && !load_street_brawl_ticks
            && !load_street_brawl_rounds
        {
            return Ok(());
        }

        let need_events = load_abilities
            || load_kills
            || load_damage
            || load_flex_slots
            || load_item_purchases
            || load_chat
            || load_boss_kills
            || load_mid_boss
            || load_street_brawl_rounds;

        // Build union class filter
        let mut class_names: Vec<&str> = Vec::new();
        if load_player_ticks {
            class_names.push("CCitadelPlayerPawn");
            class_names.push("CCitadelPlayerController");
        }
        if load_world_ticks || load_street_brawl_ticks {
            class_names.push("CCitadelGameRulesProxy");
        }
        if load_abilities
            || load_kills
            || load_damage
            || load_mid_boss
            || load_active_modifiers
            || load_urn
        {
            class_names.push("CCitadelPlayerPawn");
        }
        if load_ability_upgrades || load_item_purchases || load_chat || load_stat_modifier_events {
            class_names.push("CCitadelPlayerController");
        }
        if load_objectives {
            class_names.push("CNPC_Boss_Tier2");
            class_names.push("CNPC_Boss_Tier3");
            class_names.push("CNPC_BarrackBoss");
            class_names.push("CNPC_MidBoss");
            class_names.push("CCitadel_Destroyable_Building");
        }
        if load_troopers {
            class_names.push("CNPC_Trooper");
            class_names.push("CNPC_TrooperBoss");
        }
        if load_neutrals {
            class_names.push("CNPC_TrooperNeutral");
            class_names.push("CNPC_TrooperNeutralNodeMover");
        }
        if load_urn {
            class_names.push("CCitadelIdolReturnTrigger");
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
        let mut flex_ticks: Vec<i32> = Vec::new();
        let mut flex_team_nums: Vec<i32> = Vec::new();
        let mut ability_ticks: Vec<i32> = Vec::new();
        let mut ability_hero_ids: Vec<i64> = Vec::new();
        let mut ability_names: Vec<String> = Vec::new();
        let mut slot_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut slot_to_hero_built = false;

        // ── Column vectors for ability_upgrades ──
        let mut au_ticks: Vec<i32> = Vec::new();
        let mut au_hero_ids: Vec<i64> = Vec::new();
        let mut au_ability_ids: Vec<u32> = Vec::new();
        let mut au_tier: Vec<i32> = Vec::new();
        // Change detection: (controller_entity_index, slot_index) → previous upgrade_bits
        let mut au_prev_bits: HashMap<(i32, usize), i32> = HashMap::new();

        // ── Column vectors for chat ──
        let mut chat_ticks: Vec<i32> = Vec::new();
        let mut chat_hero_ids: Vec<i64> = Vec::new();
        let mut chat_texts: Vec<String> = Vec::new();
        let mut chat_types: Vec<String> = Vec::new();

        // ── Column vectors for objectives (change detection) ──
        let mut obj_tick: Vec<i32> = Vec::new();
        let mut obj_type: Vec<String> = Vec::new();
        let mut obj_team_num: Vec<i64> = Vec::new();
        let mut obj_lane: Vec<i64> = Vec::new();
        let mut obj_health: Vec<i64> = Vec::new();
        let mut obj_max_health: Vec<i64> = Vec::new();
        let mut obj_phase: Vec<i64> = Vec::new();
        let mut obj_x: Vec<f32> = Vec::new();
        let mut obj_y: Vec<f32> = Vec::new();
        let mut obj_z: Vec<f32> = Vec::new();
        // Change detection: entity_index → (health, max_health, phase)
        let mut obj_prev: HashMap<i32, (i64, i64, i64)> = HashMap::new();
        // Patron phase key
        let mut patron_phase_key: Option<u64> = None;
        // Patron phase change detection for boss_kills: entity_index → prev phase
        let mut patron_phase_prev: HashMap<i32, i64> = HashMap::new();

        // ── Column vectors for boss_kills ──
        // ── Column vectors for mid_boss ──
        let mut mb_ticks: Vec<i32> = Vec::new();
        let mut mb_hero_ids: Vec<i64> = Vec::new();
        let mut mb_team_nums: Vec<i32> = Vec::new();
        let mut mb_events: Vec<String> = Vec::new();

        let mut bk_ticks: Vec<i32> = Vec::new();
        let mut bk_objective_teams: Vec<i32> = Vec::new();
        let mut bk_objective_ids: Vec<i32> = Vec::new();
        let mut bk_entity_classes: Vec<String> = Vec::new();
        let mut bk_gametimes: Vec<f32> = Vec::new();

        // ── Column vectors for item_purchases ──
        let mut ip_ticks: Vec<i32> = Vec::new();
        let mut ip_hero_ids: Vec<i64> = Vec::new();
        let mut ip_ability_ids: Vec<u32> = Vec::new();
        let mut ip_changes: Vec<String> = Vec::new();

        // ── Column vectors for troopers (lane only) ──
        let mut tr_tick: Vec<i32> = Vec::new();
        let mut tr_type: Vec<String> = Vec::new();
        let mut tr_team_num: Vec<i64> = Vec::new();
        let mut tr_lane: Vec<i64> = Vec::new();
        let mut tr_health: Vec<i64> = Vec::new();
        let mut tr_max_health: Vec<i64> = Vec::new();
        let mut tr_x: Vec<f32> = Vec::new();
        let mut tr_y: Vec<f32> = Vec::new();
        let mut tr_z: Vec<f32> = Vec::new();

        // ── Column vectors for neutrals (change-detected) ──
        let mut nt_tick: Vec<i32> = Vec::new();
        let mut nt_type: Vec<String> = Vec::new();
        let mut nt_team_num: Vec<i64> = Vec::new();
        let mut nt_health: Vec<i64> = Vec::new();
        let mut nt_max_health: Vec<i64> = Vec::new();
        let mut nt_x: Vec<f32> = Vec::new();
        let mut nt_y: Vec<f32> = Vec::new();
        let mut nt_z: Vec<f32> = Vec::new();
        // Change detection: entity_index → (was_alive, health, max_health, x_bits, y_bits, z_bits)
        let mut nt_prev: HashMap<i32, (bool, i64, i64, u32, u32, u32)> = HashMap::new();

        // ── Column vectors for stat_modifiers (event-based change detection) ──
        let mut sm_tick: Vec<i32> = Vec::new();
        let mut sm_hero_id: Vec<i64> = Vec::new();
        let mut sm_stat_type: Vec<String> = Vec::new();
        let mut sm_amount: Vec<f32> = Vec::new();
        // Change detection: (controller_entity_index, eValType) → previous summed value
        let mut sm_prev: HashMap<(i32, u32), f32> = HashMap::new();

        // ── Column vectors for active_modifiers ──
        let mut am_tick: Vec<i32> = Vec::new();
        let mut am_hero_id: Vec<i64> = Vec::new();
        let mut am_event: Vec<String> = Vec::new();
        let mut am_modifier_id: Vec<u32> = Vec::new();
        let mut am_ability_id: Vec<u32> = Vec::new();
        let mut am_duration: Vec<f32> = Vec::new();
        let mut am_caster_hero_id: Vec<i64> = Vec::new();
        let mut am_stacks: Vec<i32> = Vec::new();
        // ── Column vectors for street_brawl_ticks ──
        let sbt_capacity = if load_street_brawl_ticks {
            self.total_ticks as usize
        } else {
            0
        };
        let mut sbt_tick: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_round: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_state: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_amber_score: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_sapphire_score: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_buy_countdown: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_next_state_time: Vec<f32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_state_start_time: Vec<f32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_non_combat_time: Vec<f32> = Vec::with_capacity(sbt_capacity);

        // ── Column vectors for street_brawl_rounds ──
        let mut sbr_round: Vec<i32> = Vec::new();
        let mut sbr_tick: Vec<i32> = Vec::new();
        let mut sbr_scoring_team: Vec<i32> = Vec::new();
        let mut sbr_amber_score: Vec<i32> = Vec::new();
        let mut sbr_sapphire_score: Vec<i32> = Vec::new();
        let mut sbr_round_counter: i32 = 0;

        // ── Column vectors for urn ──
        let mut urn_tick: Vec<i32> = Vec::new();
        let mut urn_event: Vec<String> = Vec::new();
        let mut urn_hero_id: Vec<i64> = Vec::new();
        let mut urn_team_num: Vec<i64> = Vec::new();
        let mut urn_x: Vec<f32> = Vec::new();
        let mut urn_y: Vec<f32> = Vec::new();
        let mut urn_z: Vec<f32> = Vec::new();

        // Track active modifiers by serial_number for change detection
        struct CachedMod {
            hero_id: i64,
            modifier_id: u32,
            ability_id: u32,
            duration: f32,
            caster_hero_id: i64,
            stacks: i32,
        }
        let mut am_prev: HashMap<u32, CachedMod> = HashMap::new();

        // Track idol modifiers for urn lifecycle
        const GOLDEN_IDOL_ABILITY: u32 = 2521299219;
        const IDOL_RETURN: u32 = 3388847715;

        // serial -> hero_id for golden_idol modifiers (carrying state)
        let mut urn_idol_serials: HashMap<u32, i64> = HashMap::new();
        // hero_id -> number of active golden_idol modifiers
        let mut urn_hero_count: HashMap<i64, i32> = HashMap::new();
        // serials for idol_return modifiers already emitted
        let mut urn_return_seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        // hero_id -> last tick a "returned" event was emitted (dedup flicker)
        let mut urn_last_return_tick: HashMap<i64, i32> = HashMap::new();
        // entity_idx -> (disabled, team_num) for delivery trigger change detection
        let mut urn_trigger_prev: HashMap<i32, (bool, i64)> = HashMap::new();

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

        // Controller hero_id key (for purchases/shop_events slot→hero mapping)
        let mut ck_hero_id: Option<u64> = None;

        // Ability upgrade slot keys: (item_id_key, upgrade_bits_key) for indices 0..7
        let mut au_slot_keys: Vec<(Option<u64>, Option<u64>)> = Vec::new();

        // Objective NPC keys (shared across all NPC classes)
        let mut nk_health: Option<u64> = None;
        let mut nk_max_health: Option<u64> = None;
        let mut nk_team_num: Option<u64> = None;
        let mut nk_lane: Option<u64> = None;
        let mut nk_vec_x: Option<u64> = None;
        let mut nk_vec_y: Option<u64> = None;
        let mut nk_vec_z: Option<u64> = None;
        // Shrine (CCitadel_Destroyable_Building) has different field keys
        let mut shrine_health: Option<u64> = None;
        let mut shrine_max_health: Option<u64> = None;
        let mut shrine_vec_x: Option<u64> = None;
        let mut shrine_vec_y: Option<u64> = None;
        let mut shrine_vec_z: Option<u64> = None;
        let mut shrine_team_num: Option<u64> = None;

        // Trooper NPC keys (lane troopers)
        let mut tk_health: Option<u64> = None;
        let mut tk_max_health: Option<u64> = None;
        let mut tk_team_num: Option<u64> = None;
        let mut tk_lane: Option<u64> = None;
        let mut tk_lifestate: Option<u64> = None;
        let mut tk_vec_x: Option<u64> = None;
        let mut tk_vec_y: Option<u64> = None;
        let mut tk_vec_z: Option<u64> = None;

        // Neutral NPC keys
        let mut ntk_health: Option<u64> = None;
        let mut ntk_max_health: Option<u64> = None;
        let mut ntk_team_num: Option<u64> = None;
        let mut ntk_lifestate: Option<u64> = None;
        let mut ntk_vec_x: Option<u64> = None;
        let mut ntk_vec_y: Option<u64> = None;
        let mut ntk_vec_z: Option<u64> = None;

        // StatViewerModifierValues keys for indices 0..20: (modifier_id, val_type, value)
        let mut smk_keys: Vec<(Option<u64>, Option<u64>, Option<u64>)> = Vec::new();

        // World keys
        let mut wk_is_paused: Option<u64> = None;
        let mut wk_next_midboss: Option<u64> = None;

        // Urn delivery trigger keys (CCitadelIdolReturnTrigger)
        let mut urnk_disabled: Option<u64> = None;
        let mut urnk_team_num: Option<u64> = None;
        let mut urnk_vec_x: Option<u64> = None;
        let mut urnk_vec_y: Option<u64> = None;
        let mut urnk_vec_z: Option<u64> = None;

        // Street brawl keys
        let mut sbk_round: Option<u64> = None;
        let mut sbk_state: Option<u64> = None;
        let mut sbk_amber_score: Option<u64> = None;
        let mut sbk_sapphire_score: Option<u64> = None;
        let mut sbk_buy_countdown: Option<u64> = None;
        let mut sbk_next_state_time: Option<u64> = None;
        let mut sbk_state_start_time: Option<u64> = None;
        let mut sbk_non_combat_time: Option<u64> = None;

        // ── Single-pass callback logic (shared between both code paths) ──
        //
        // We use a macro to avoid duplicating the entity extraction code across
        // the events-aware and entities-only branches.
        macro_rules! collect_entity_data {
            ($ctx:expr) => {
                if !keys_resolved {
                    if load_abilities || load_player_ticks || load_kills || load_damage || load_active_modifiers || load_urn {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerPawn") {
                            pk_hero_id = s.resolve_field_key(
                                "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID",
                            );
                            if load_player_ticks || load_urn {
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
                    if load_item_purchases || load_chat {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerController") {
                            ck_hero_id =
                                s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                        }
                    }
                    if load_ability_upgrades {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerController") {
                            if ck_hero_id.is_none() {
                                ck_hero_id =
                                    s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                            }
                            for i in 0..8usize {
                                let item_key = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_ItemID"
                                ));
                                let bits_key = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_nUpgradeInfo"
                                ));
                                au_slot_keys.push((item_key, bits_key));
                            }
                        }
                    }
                    if load_stat_modifier_events {
                        if let Some(s) = $ctx.serializers.get("CCitadelPlayerController") {
                            if ck_hero_id.is_none() {
                                ck_hero_id =
                                    s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                            }
                            for i in 0..20usize {
                                let mid = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_SourceModifierID"
                                ));
                                let vt = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_eValType"
                                ));
                                let val = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_flValue"
                                ));
                                smk_keys.push((mid, vt, val));
                            }
                        }
                    }
                    if load_objectives {
                        // NPC objective classes share field names; resolve from first found
                        for obj_class in &["CNPC_Boss_Tier2", "CNPC_Boss_Tier3", "CNPC_BarrackBoss", "CNPC_MidBoss"] {
                            if let Some(s) = $ctx.serializers.get(*obj_class) {
                                nk_health = s.resolve_field_key("m_iHealth");
                                nk_max_health = s.resolve_field_key("m_iMaxHealth");
                                nk_team_num = s.resolve_field_key("m_iTeamNum");
                                nk_lane = s.resolve_field_key("m_iLane");
                                nk_vec_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX");
                                nk_vec_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY");
                                nk_vec_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ");
                                break;
                            }
                        }
                        // Patron phase key
                        if let Some(s) = $ctx.serializers.get("CNPC_Boss_Tier3") {
                            patron_phase_key = s.resolve_field_key("m_ePhase");
                        }
                        // Shrine has a different serializer with different field keys
                        if let Some(s) = $ctx.serializers.get("CCitadel_Destroyable_Building") {
                            shrine_health = s.resolve_field_key("m_iHealth");
                            shrine_max_health = s.resolve_field_key("m_iMaxHealth");
                            shrine_team_num = s.resolve_field_key("m_iTeamNum");
                            shrine_vec_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX");
                            shrine_vec_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY");
                            shrine_vec_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ");
                        }
                    }
                    if load_troopers {
                        for tr_class in &["CNPC_Trooper", "CNPC_TrooperBoss"] {
                            if let Some(s) = $ctx.serializers.get(*tr_class) {
                                tk_health = s.resolve_field_key("m_iHealth");
                                tk_max_health = s.resolve_field_key("m_iMaxHealth");
                                tk_team_num = s.resolve_field_key("m_iTeamNum");
                                tk_lane = s.resolve_field_key("m_iLane");
                                tk_lifestate = s.resolve_field_key("m_lifeState");
                                tk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                tk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                tk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                break;
                            }
                        }
                    }
                    if load_neutrals {
                        for nt_class in &["CNPC_TrooperNeutral", "CNPC_TrooperNeutralNodeMover"] {
                            if let Some(s) = $ctx.serializers.get(*nt_class) {
                                ntk_health = s.resolve_field_key("m_iHealth");
                                ntk_max_health = s.resolve_field_key("m_iMaxHealth");
                                ntk_team_num = s.resolve_field_key("m_iTeamNum");
                                ntk_lifestate = s.resolve_field_key("m_lifeState");
                                ntk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                ntk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                ntk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                break;
                            }
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
                    if load_urn {
                        if let Some(s) = $ctx.serializers.get("CCitadelIdolReturnTrigger") {
                            urnk_disabled = s.resolve_field_key("m_bDisabled");
                            urnk_team_num = s.resolve_field_key("m_iTeamNum");
                            urnk_vec_x = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                            );
                            urnk_vec_y = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                            );
                            urnk_vec_z = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                            );
                        }
                    }
                    if load_street_brawl_ticks {
                        if let Some(s) = $ctx.serializers.get("CCitadelGameRulesProxy") {
                            sbk_round = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iRound");
                            sbk_state = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_eStreetBrawlState");
                            sbk_amber_score = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iTeamAmberScore");
                            sbk_sapphire_score = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iTeamSapphireScore");
                            sbk_buy_countdown = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iLastBuyCountDown");
                            sbk_next_state_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flNextStateTime");
                            sbk_state_start_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flStreetBrawlStateStartTime");
                            sbk_non_combat_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flStreetBrawlTotalNonCombatTime");
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

                // ── Collect world_ticks / street_brawl_ticks ──
                if load_world_ticks || load_street_brawl_ticks {
                    if let Some((_, entity)) = $ctx
                        .entities
                        .iter()
                        .find(|(_, e)| e.class_name == "CCitadelGameRulesProxy")
                    {
                        if load_world_ticks {
                            wt_tick.push($ctx.tick);
                            wt_is_paused.push(get_bool(entity, wk_is_paused));
                            wt_next_midboss.push(get_f32(entity, wk_next_midboss));
                        }
                        if load_street_brawl_ticks {
                            sbt_tick.push($ctx.tick);
                            sbt_round.push(get_i64(entity, sbk_round) as i32);
                            sbt_state.push(get_i64(entity, sbk_state) as i32);
                            sbt_amber_score.push(get_i64(entity, sbk_amber_score) as i32);
                            sbt_sapphire_score.push(get_i64(entity, sbk_sapphire_score) as i32);
                            sbt_buy_countdown.push(get_i64(entity, sbk_buy_countdown) as i32);
                            sbt_next_state_time.push(get_f32(entity, sbk_next_state_time));
                            sbt_state_start_time.push(get_f32(entity, sbk_state_start_time));
                            sbt_non_combat_time.push(get_f32(entity, sbk_non_combat_time));
                        }
                    }
                }

                // ── Build entity_to_hero map (for kills/damage/mid_boss resolution) ──
                if (load_abilities || load_kills || load_damage || load_mid_boss || load_active_modifiers || load_urn) && !entity_to_hero_built {
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

                // ── Build slot_to_hero map (for item_purchases/chat: userid → hero_id) ──
                if (load_item_purchases || load_chat) && !slot_to_hero_built {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name == "CCitadelPlayerController" {
                            let hid = get_i64(entity, ck_hero_id);
                            if hid != 0 {
                                // userid is 0-based, controller entity index is 1-based
                                slot_to_hero.insert(idx - 1, hid);
                            }
                        }
                    }
                    if !slot_to_hero.is_empty() {
                        slot_to_hero_built = true;
                    }
                }

                // ── Collect ability_upgrades (entity change detection) ──
                if load_ability_upgrades {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name != "CCitadelPlayerController" {
                            continue;
                        }
                        let hero_id = get_i64(entity, ck_hero_id);
                        if hero_id == 0 {
                            continue;
                        }
                        for (slot_idx, (item_key, bits_key)) in au_slot_keys.iter().enumerate() {
                            let ability_id = item_key
                                .and_then(|k| entity.fields.get(&k))
                                .and_then(|v| match v {
                                    boon_parser::FieldValue::U32(n) => Some(*n),
                                    boon_parser::FieldValue::U64(n) => Some(*n as u32),
                                    boon_parser::FieldValue::I32(n) => Some(*n as u32),
                                    boon_parser::FieldValue::I64(n) => Some(*n as u32),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            if ability_id == 0 {
                                continue;
                            }
                            // m_nUpgradeInfo packs upgrade bits in bits 17+
                            let upgrade_bits = bits_key
                                .and_then(|k| entity.fields.get(&k))
                                .and_then(|v| match v {
                                    boon_parser::FieldValue::I32(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::I64(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::U32(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::U64(n) => Some((*n >> 17) as i32),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            let key = (idx, slot_idx);
                            let prev = au_prev_bits.get(&key).copied().unwrap_or(0);
                            if upgrade_bits != prev {
                                au_prev_bits.insert(key, upgrade_bits);
                                if upgrade_bits > prev {
                                    au_ticks.push($ctx.tick);
                                    au_hero_ids.push(hero_id);
                                    au_ability_ids.push(ability_id);
                                    au_tier.push(upgrade_bits.count_ones() as i32 - 1);
                                }
                            }
                        }
                    }
                }

                // ── Collect objectives (change detection on health/max_health/phase) ──
                if load_objectives {
                    for (&idx, entity) in $ctx.entities.iter() {
                        let obj_class = entity.class_name.as_str();
                        let is_patron = obj_class == "CNPC_Boss_Tier3";
                        let (otype, hp_key, max_hp_key, team_key, lane_key, vx_key, vy_key, vz_key) = match obj_class {
                            "CNPC_Boss_Tier2" => ("walker", nk_health, nk_max_health, nk_team_num, nk_lane, nk_vec_x, nk_vec_y, nk_vec_z),
                            "CNPC_Boss_Tier3" => ("patron", nk_health, nk_max_health, nk_team_num, nk_lane, nk_vec_x, nk_vec_y, nk_vec_z),
                            "CNPC_BarrackBoss" => ("barracks", nk_health, nk_max_health, nk_team_num, nk_lane, nk_vec_x, nk_vec_y, nk_vec_z),
                            "CNPC_MidBoss" => ("mid_boss", nk_health, nk_max_health, nk_team_num, nk_lane, nk_vec_x, nk_vec_y, nk_vec_z),
                            "CCitadel_Destroyable_Building" => ("shrine", shrine_health, shrine_max_health, shrine_team_num, None, shrine_vec_x, shrine_vec_y, shrine_vec_z),
                            _ => continue,
                        };
                        let max_hp = get_i64(entity, max_hp_key);
                        if max_hp == 0 {
                            continue;
                        }
                        let hp = get_i64(entity, hp_key);
                        let phase = if is_patron { get_i64(entity, patron_phase_key) } else { 0 };
                        let cur = (hp, max_hp, phase);
                        let changed = match obj_prev.get(&idx) {
                            None => true,
                            Some(prev) => *prev != cur,
                        };
                        if changed {
                            obj_prev.insert(idx, cur);
                            obj_tick.push($ctx.tick);
                            obj_type.push(otype.to_string());
                            obj_team_num.push(get_i64(entity, team_key));
                            obj_lane.push(get_i64(entity, lane_key));
                            obj_health.push(hp);
                            obj_max_health.push(max_hp);
                            obj_phase.push(phase);
                            obj_x.push(get_f32(entity, vx_key));
                            obj_y.push(get_f32(entity, vy_key));
                            obj_z.push(get_f32(entity, vz_key));
                        }

                        // Detect patron phase changes for boss_kills
                        if is_patron && load_boss_kills {
                            let prev_phase = patron_phase_prev.get(&idx).copied().unwrap_or(0);
                            if phase != prev_phase {
                                patron_phase_prev.insert(idx, phase);
                                if phase == 2 {
                                    // Shrines destroyed → patron shields down
                                    bk_ticks.push($ctx.tick);
                                    bk_objective_teams.push(get_i64(entity, team_key) as i32);
                                    bk_objective_ids.push(0);
                                    bk_entity_classes.push("patron_shields_down".to_string());
                                    bk_gametimes.push(0.0);
                                }
                            }
                        }
                    }
                }

                // ── Collect troopers (lane troopers, per-tick alive only) ──
                if load_troopers {
                    for (_, entity) in $ctx.entities.iter() {
                        let ttype = match entity.class_name.as_str() {
                            "CNPC_Trooper" => "trooper",
                            "CNPC_TrooperBoss" => "trooper_boss",
                            _ => continue,
                        };
                        let max_hp = get_i64(entity, tk_max_health);
                        if max_hp == 0 {
                            continue;
                        }
                        let lifestate = get_i64(entity, tk_lifestate);
                        if lifestate != 0 {
                            continue;
                        }
                        tr_tick.push($ctx.tick);
                        tr_type.push(ttype.to_string());
                        tr_team_num.push(get_i64(entity, tk_team_num));
                        tr_lane.push(get_i64(entity, tk_lane));
                        tr_health.push(get_i64(entity, tk_health));
                        tr_max_health.push(max_hp);
                        tr_x.push(get_f32(entity, tk_vec_x));
                        tr_y.push(get_f32(entity, tk_vec_y));
                        tr_z.push(get_f32(entity, tk_vec_z));
                    }
                }

                // ── Collect stat_modifiers (event-based change detection) ──
                if load_stat_modifier_events {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name != "CCitadelPlayerController" {
                            continue;
                        }
                        let hero_id = get_i64(entity, ck_hero_id);
                        if hero_id == 0 {
                            continue;
                        }

                        // Sum values by eValType
                        let mut by_type: HashMap<u32, f32> = HashMap::new();
                        for (_mid_key, vt_key, val_key) in &smk_keys {
                            let vt_val = vt_key
                                .and_then(|k| entity.fields.get(&k))
                                .and_then(|v| match v {
                                    boon_parser::FieldValue::U32(n) => Some(*n),
                                    boon_parser::FieldValue::I32(n) => Some(*n as u32),
                                    boon_parser::FieldValue::U64(n) => Some(*n as u32),
                                    boon_parser::FieldValue::I64(n) => Some(*n as u32),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            if vt_val == 0 {
                                continue;
                            }
                            let fl_val = get_f32(entity, *val_key);
                            *by_type.entry(vt_val).or_insert(0.0) += fl_val;
                        }

                        // Emit events for changed stat types
                        for (vt_val, total) in &by_type {
                            let key = (idx, *vt_val);
                            let prev = sm_prev.get(&key).copied().unwrap_or(0.0);
                            if (*total - prev).abs() > f32::EPSILON {
                                sm_prev.insert(key, *total);
                                let stat_name = match *vt_val {
                                    31 => "health",
                                    51 => "spirit_power",
                                    79 => "fire_rate",
                                    18 => "weapon_damage",
                                    109 => "cooldown_reduction",
                                    172 => "ammo",
                                    _ => continue,
                                };
                                sm_tick.push($ctx.tick);
                                sm_hero_id.push(hero_id);
                                sm_stat_type.push(stat_name.to_string());
                                sm_amount.push(*total - prev);
                            }
                        }
                    }
                }

                // ── Collect active_modifiers (string table change detection) ──
                if load_active_modifiers {
                    if let Some(table) = $ctx.string_tables.find_table("ActiveModifiers") {
                        let mut current_serials: std::collections::HashSet<u32> = std::collections::HashSet::new();

                        for entry in &table.entries {
                            let data = match &entry.user_data {
                                Some(d) if !d.is_empty() => d,
                                _ => continue,
                            };

                            let Ok(modifier) =
                                boon_proto::proto::CModifierTableEntry::decode(data.as_slice())
                            else {
                                continue;
                            };

                            let serial = match modifier.serial_number {
                                Some(s) => s,
                                None => continue,
                            };

                            let parent_handle = modifier.parent.unwrap_or(INVALID_ENTITY_HANDLE);
                            if parent_handle == INVALID_ENTITY_HANDLE {
                                continue;
                            }
                            let parent_idx = (parent_handle & 0x3FFF) as i32;

                            let hero_id = match entity_to_hero.get(&parent_idx) {
                                Some(&hid) => hid,
                                None => continue,
                            };

                            let mod_entry_type = modifier.entry_type.unwrap_or(1);

                            if mod_entry_type == 2 {
                                if let Some(cached) = am_prev.remove(&serial) {
                                    am_tick.push($ctx.tick);
                                    am_hero_id.push(cached.hero_id);
                                    am_event.push("removed".to_string());
                                    am_modifier_id.push(cached.modifier_id);
                                    am_ability_id.push(cached.ability_id);
                                    am_duration.push(cached.duration);
                                    am_caster_hero_id.push(cached.caster_hero_id);
                                    am_stacks.push(cached.stacks);
                                }
                                continue;
                            }

                            current_serials.insert(serial);

                            if let std::collections::hash_map::Entry::Vacant(e) = am_prev.entry(serial) {
                                let mod_id = modifier.modifier_subclass.unwrap_or(0);
                                let abil_id = modifier.ability_subclass.unwrap_or(0);
                                let duration = modifier.duration.unwrap_or(-1.0);
                                let caster_handle = modifier.caster.unwrap_or(INVALID_ENTITY_HANDLE);
                                let caster_hero_id = if caster_handle != INVALID_ENTITY_HANDLE {
                                    let caster_idx = (caster_handle & 0x3FFF) as i32;
                                    entity_to_hero.get(&caster_idx).copied().unwrap_or(0)
                                } else {
                                    0
                                };
                                let stacks = modifier.stack_count.unwrap_or(0);

                                am_tick.push($ctx.tick);
                                am_hero_id.push(hero_id);
                                am_event.push("applied".to_string());
                                am_modifier_id.push(mod_id);
                                am_ability_id.push(abil_id);
                                am_duration.push(duration);
                                am_caster_hero_id.push(caster_hero_id);
                                am_stacks.push(stacks);

                                e.insert(CachedMod {
                                    hero_id,
                                    modifier_id: mod_id,
                                    ability_id: abil_id,
                                    duration,
                                    caster_hero_id,
                                    stacks,
                                });
                            }
                        }

                        // Detect removed: serials in prev but not in current
                        let removed: Vec<u32> = am_prev
                            .keys()
                            .filter(|s| !current_serials.contains(s))
                            .copied()
                            .collect();
                        for serial in removed {
                            if let Some(cached) = am_prev.remove(&serial) {
                                am_tick.push($ctx.tick);
                                am_hero_id.push(cached.hero_id);
                                am_event.push("removed".to_string());
                                am_modifier_id.push(cached.modifier_id);
                                am_ability_id.push(cached.ability_id);
                                am_duration.push(cached.duration);
                                am_caster_hero_id.push(cached.caster_hero_id);
                                am_stacks.push(cached.stacks);
                            }
                        }
                    }
                }

                // ── Collect urn (idol lifecycle tracking) ──
                if load_urn {
                    if let Some(table) = $ctx.string_tables.find_table("ActiveModifiers") {
                        let mut current_urn_serials: std::collections::HashSet<u32> =
                            std::collections::HashSet::new();

                        for entry in &table.entries {
                            let data = match &entry.user_data {
                                Some(d) if !d.is_empty() => d,
                                _ => continue,
                            };

                            let Ok(modifier) =
                                boon_proto::proto::CModifierTableEntry::decode(data.as_slice())
                            else {
                                continue;
                            };

                            let serial = match modifier.serial_number {
                                Some(s) => s,
                                None => continue,
                            };

                            let mod_entry_type = modifier.entry_type.unwrap_or(1);

                            // Handle explicit removal (entry_type == 2)
                            if mod_entry_type == 2 {
                                if let Some(hero_id) = urn_idol_serials.remove(&serial) {
                                    let count =
                                        urn_hero_count.entry(hero_id).or_insert(0);
                                    *count -= 1;
                                    if *count <= 0 {
                                        urn_hero_count.remove(&hero_id);
                                        let pawn = entity_to_hero.iter()
                                            .find(|(_, hid)| **hid == hero_id)
                                            .and_then(|(idx, _)| $ctx.entities.get(*idx));
                                        urn_tick.push($ctx.tick);
                                        urn_event.push("dropped".to_string());
                                        urn_hero_id.push(hero_id);
                                        urn_team_num.push(0);
                                        urn_x.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_x)));
                                        urn_y.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_y)));
                                        urn_z.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_z)));
                                    }
                                }
                                urn_return_seen.remove(&serial);
                                continue;
                            }

                            let mod_id = modifier.modifier_subclass.unwrap_or(0);
                            let abil_id = modifier.ability_subclass.unwrap_or(0);
                            let is_golden_idol = abil_id == GOLDEN_IDOL_ABILITY;
                            let is_idol_return = mod_id == IDOL_RETURN;

                            if !is_golden_idol && !is_idol_return {
                                continue;
                            }

                            let parent_handle = modifier.parent.unwrap_or(INVALID_ENTITY_HANDLE);
                            if parent_handle == INVALID_ENTITY_HANDLE {
                                continue;
                            }
                            let parent_idx = (parent_handle & 0x3FFF) as i32;

                            let hero_id = match entity_to_hero.get(&parent_idx) {
                                Some(&hid) => hid,
                                None => continue,
                            };

                            // Look up pawn position for hero events
                            let pawn = $ctx.entities.get(parent_idx);
                            let hero_x = pawn.map_or(0.0, |e| get_f32(e, pk_vec_x));
                            let hero_y = pawn.map_or(0.0, |e| get_f32(e, pk_vec_y));
                            let hero_z = pawn.map_or(0.0, |e| get_f32(e, pk_vec_z));

                            current_urn_serials.insert(serial);

                            if is_golden_idol
                                && !urn_idol_serials.contains_key(&serial)
                            {
                                let count =
                                    urn_hero_count.entry(hero_id).or_insert(0);
                                if *count == 0 {
                                    urn_tick.push($ctx.tick);
                                    urn_event.push("picked_up".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(hero_x);
                                    urn_y.push(hero_y);
                                    urn_z.push(hero_z);
                                }
                                *count += 1;
                                urn_idol_serials.insert(serial, hero_id);
                            }

                            if is_idol_return && urn_return_seen.insert(serial) {
                                let last = urn_last_return_tick
                                    .get(&hero_id)
                                    .copied()
                                    .unwrap_or(-999);
                                if $ctx.tick - last > 64 {
                                    urn_tick.push($ctx.tick);
                                    urn_event.push("returned".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(hero_x);
                                    urn_y.push(hero_y);
                                    urn_z.push(hero_z);
                                    urn_last_return_tick.insert(hero_id, $ctx.tick);
                                }
                            }
                        }

                        // Detect disappeared golden_idol modifiers
                        let removed: Vec<u32> = urn_idol_serials
                            .keys()
                            .filter(|s| !current_urn_serials.contains(s))
                            .copied()
                            .collect();
                        for serial in removed {
                            if let Some(hero_id) = urn_idol_serials.remove(&serial) {
                                let count =
                                    urn_hero_count.entry(hero_id).or_insert(0);
                                *count -= 1;
                                if *count <= 0 {
                                    urn_hero_count.remove(&hero_id);
                                    let pawn = entity_to_hero.iter()
                                        .find(|(_, hid)| **hid == hero_id)
                                        .and_then(|(idx, _)| $ctx.entities.get(*idx));
                                    urn_tick.push($ctx.tick);
                                    urn_event.push("dropped".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_x)));
                                    urn_y.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_y)));
                                    urn_z.push(pawn.map_or(0.0, |e| get_f32(e, pk_vec_z)));
                                }
                            }
                        }
                        // Clean up disappeared return serials
                        urn_return_seen
                            .retain(|s| current_urn_serials.contains(s));
                    }
                }

                // ── Collect urn delivery triggers ──
                if load_urn {
                    for (&idx, entity) in $ctx.entities.iter() {
                        if entity.class_name != "CCitadelIdolReturnTrigger" {
                            continue;
                        }
                        let disabled = get_bool(entity, urnk_disabled);
                        let team = get_i64(entity, urnk_team_num);
                        let cur = (disabled, team);
                        let prev = urn_trigger_prev.get(&idx).copied();
                        let changed = match prev {
                            None => true,
                            Some(p) => p != cur,
                        };
                        if changed {
                            urn_trigger_prev.insert(idx, cur);
                            if !disabled && team != 0 {
                                urn_tick.push($ctx.tick);
                                urn_event.push("delivery_active".to_string());
                                urn_hero_id.push(0);
                                urn_team_num.push(team);
                                urn_x.push(get_f32(entity, urnk_vec_x));
                                urn_y.push(get_f32(entity, urnk_vec_y));
                                urn_z.push(get_f32(entity, urnk_vec_z));
                            } else if disabled {
                                // Only emit inactive when transitioning from active
                                if let Some((prev_disabled, _)) = prev {
                                    if !prev_disabled {
                                        urn_tick.push($ctx.tick);
                                        urn_event.push("delivery_inactive".to_string());
                                        urn_hero_id.push(0);
                                        urn_team_num.push(team);
                                        urn_x.push(get_f32(entity, urnk_vec_x));
                                        urn_y.push(get_f32(entity, urnk_vec_y));
                                        urn_z.push(get_f32(entity, urnk_vec_z));
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Collect neutrals (change-detected, only emit on state change) ──
                if load_neutrals {
                    for (&idx, entity) in $ctx.entities.iter() {
                        let ntype = match entity.class_name.as_str() {
                            "CNPC_TrooperNeutral" => "neutral",
                            "CNPC_TrooperNeutralNodeMover" => "neutral_node_mover",
                            _ => continue,
                        };
                        let max_hp = get_i64(entity, ntk_max_health);
                        if max_hp == 0 {
                            continue;
                        }
                        let lifestate = get_i64(entity, ntk_lifestate);
                        let alive = lifestate == 0;
                        let x = get_f32(entity, ntk_vec_x);
                        let y = get_f32(entity, ntk_vec_y);
                        let z = get_f32(entity, ntk_vec_z);
                        let hp = get_i64(entity, ntk_health);

                        let cur = (alive, hp, max_hp, x.to_bits(), y.to_bits(), z.to_bits());
                        let changed = match nt_prev.get(&idx) {
                            None => true,
                            Some(prev) => {
                                alive != prev.0
                                    || (alive && (hp != prev.1 || max_hp != prev.2 || x.to_bits() != prev.3 || y.to_bits() != prev.4 || z.to_bits() != prev.5))
                            }
                        };
                        if changed {
                            nt_prev.insert(idx, cur);
                            if alive {
                                nt_tick.push($ctx.tick);
                                nt_type.push(ntype.to_string());
                                nt_team_num.push(get_i64(entity, ntk_team_num));
                                nt_health.push(hp);
                                nt_max_health.push(max_hp);
                                nt_x.push(x);
                                nt_y.push(y);
                                nt_z.push(z);
                            }
                        }
                    }
                }

            };
        }

        // ── Run the parse pass ──
        if need_events {
            self.parser
                .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
                    collect_entity_data!(ctx);

                    for event in events {
                        if load_kills && event.msg_type == Msg::KEUserMsgHeroKilled as u32 {
                            raw_kill_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        if load_damage && event.msg_type == Msg::KEUserMsgDamage as u32 {
                            raw_damage_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        if found_game_over.is_none()
                            && event.msg_type == Msg::KEUserMsgGameOver as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMessageGameOver::decode(
                                event.payload.as_slice(),
                            )
                        {
                            found_game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
                        }
                        // Collect FlexSlotUnlocked events (msg_type 356)
                        if load_flex_slots
                            && event.msg_type == Msg::KEUserMsgFlexSlotUnlocked as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgFlexSlotUnlocked::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            flex_ticks.push(event.tick);
                            flex_team_nums.push(msg.team_number.unwrap_or(0));
                        }
                        // Collect ImportantAbilityUsed events (msg_type 365)
                        if load_abilities
                            && event.msg_type == Msg::KEUserMsgImportantAbilityUsed as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMessageImportantAbilityUsed::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let pawn_idx = (msg.player.unwrap_or(0) & 0x3FFF) as i32;
                            let hero_id = entity_to_hero.get(&pawn_idx).copied().unwrap_or(0);
                            ability_ticks.push(event.tick);
                            ability_hero_ids.push(hero_id);
                            ability_names.push(msg.ability_name.unwrap_or_default());
                        }
                        // Collect AbilitiesChanged events (msg_type 309) for item_purchases
                        if load_item_purchases
                            && event.msg_type == Msg::KEUserMsgAbilitiesChanged as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgAbilitiesChanged::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let player_slot = msg.purchaser_player_slot.unwrap_or(-1);
                            let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                            let ability_id = msg.ability_id.unwrap_or(0);
                            let change = match msg.change.unwrap_or(-1) {
                                0 => "purchased",
                                1 => "upgraded",
                                2 => "sold",
                                3 => "swapped",
                                4 => "failure",
                                _ => "unknown",
                            };
                            ip_ticks.push(event.tick);
                            ip_hero_ids.push(hero_id);
                            ip_ability_ids.push(ability_id);
                            ip_changes.push(change.to_string());
                        }
                        // Collect ChatMsg events (msg_type 314)
                        if load_chat
                            && event.msg_type == Msg::KEUserMsgChatMsg as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMsgChatMsg::decode(
                                event.payload.as_slice(),
                            )
                        {
                            let player_slot = msg.player_slot.unwrap_or(-1);
                            let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                            let chat_type = if msg.all_chat.unwrap_or(false) {
                                "all"
                            } else {
                                "team"
                            };
                            chat_ticks.push(event.tick);
                            chat_hero_ids.push(hero_id);
                            chat_texts.push(msg.text.unwrap_or_default());
                            chat_types.push(chat_type.to_string());
                        }
                        // Collect BossKilled events (msg_type 347)
                        if load_boss_kills
                            && event.msg_type == Msg::KEUserMsgBossKilled as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMsgBossKilled::decode(
                                event.payload.as_slice(),
                            )
                        {
                            let class_id = msg.entity_killed_class.unwrap_or(0);
                            let entity_class = match class_id {
                                5 => "walker",
                                8 => "mid_boss",
                                28 => "shrine",
                                29 => "barracks",
                                30 => "barracks",
                                31 => "patron",
                                _ => "unknown",
                            };
                            bk_ticks.push(event.tick);
                            bk_objective_teams.push(msg.objective_team.unwrap_or(0));
                            bk_objective_ids.push(msg.objective_mask_change.unwrap_or(0));
                            bk_entity_classes.push(entity_class.to_string());
                            bk_gametimes.push(msg.gametime.unwrap_or(0.0));
                        }
                        // Collect mid_boss lifecycle events
                        if load_mid_boss {
                            if event.msg_type == Msg::KEUserMsgMidBossSpawned as u32 {
                                mb_ticks.push(event.tick);
                                mb_hero_ids.push(0);
                                mb_team_nums.push(0);
                                mb_events.push("spawned".to_string());
                            }
                            if event.msg_type == Msg::KEUserMsgBossKilled as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMsgBossKilled::decode(
                                        event.payload.as_slice(),
                                    )
                                && msg.entity_killed_class.unwrap_or(0) == 8
                            {
                                mb_ticks.push(event.tick);
                                mb_hero_ids.push(0);
                                mb_team_nums.push(msg.objective_team.unwrap_or(0));
                                mb_events.push("killed".to_string());
                            }
                            if event.msg_type == Msg::KEUserMsgRejuvStatus as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMsgRejuvStatus::decode(
                                        event.payload.as_slice(),
                                    )
                            {
                                let pawn_idx = (msg.player_pawn.unwrap_or(0) & 0x3FFF) as i32;
                                let hero_id = entity_to_hero.get(&pawn_idx).copied().unwrap_or(0);
                                let event_name = match msg.event_type.unwrap_or(0) {
                                    6 => "picked_up",
                                    7 => "used",
                                    8 => "expired",
                                    _ => "unknown",
                                };
                                mb_ticks.push(event.tick);
                                mb_hero_ids.push(hero_id);
                                mb_team_nums.push(msg.user_team.unwrap_or(0));
                                mb_events.push(event_name.to_string());
                            }
                        }
                        // Collect StreetBrawlScoring events (msg_type 362)
                        if load_street_brawl_rounds
                            && event.msg_type == Msg::KEUserMsgStreetBrawlScoring as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgStreetBrawlScoring::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            sbr_round_counter += 1;
                            sbr_round.push(sbr_round_counter);
                            sbr_tick.push(event.tick);
                            sbr_scoring_team.push(msg.scoring_team.unwrap_or(0));
                            sbr_amber_score.push(msg.amber_score.unwrap_or(0));
                            sbr_sapphire_score.push(msg.sapphire_score.unwrap_or(0));
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
        if need_events && !self.game_over_scanned {
            self.game_over = found_game_over;
            self.game_over_scanned = true;
        }

        // ── Build and cache DataFrames ──

        if load_player_ticks {
            let df = df_from_columns(vec![
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
            let df = df_from_columns(vec![
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
            let df = df_from_columns(vec![
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

            let df = df_from_columns(vec![
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

        if load_abilities {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ability_ticks),
                Column::new("hero_id".into(), ability_hero_ids),
                Column::new("ability".into(), ability_names),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_abilities = Some(df);
        }

        if load_flex_slots {
            let df = df_from_columns(vec![
                Column::new("tick".into(), flex_ticks),
                Column::new("team_num".into(), flex_team_nums),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_flex_slots = Some(df);
        }

        if load_ability_upgrades {
            let df = df_from_columns(vec![
                Column::new("tick".into(), au_ticks),
                Column::new("hero_id".into(), au_hero_ids),
                Column::new("ability_id".into(), au_ability_ids),
                Column::new("tier".into(), au_tier),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_ability_upgrades = Some(df);
        }

        if load_item_purchases {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ip_ticks),
                Column::new("hero_id".into(), ip_hero_ids),
                Column::new("ability_id".into(), ip_ability_ids),
                Column::new("change".into(), ip_changes),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_item_purchases = Some(df);
        }

        if load_chat {
            let df = df_from_columns(vec![
                Column::new("tick".into(), chat_ticks),
                Column::new("hero_id".into(), chat_hero_ids),
                Column::new("text".into(), chat_texts),
                Column::new("chat_type".into(), chat_types),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_chat = Some(df);
        }

        if load_objectives {
            let df = df_from_columns(vec![
                Column::new("tick".into(), obj_tick),
                Column::new("objective_type".into(), obj_type),
                Column::new("team_num".into(), obj_team_num),
                Column::new("lane".into(), obj_lane),
                Column::new("health".into(), obj_health),
                Column::new("max_health".into(), obj_max_health),
                Column::new("phase".into(), obj_phase),
                Column::new("x".into(), obj_x),
                Column::new("y".into(), obj_y),
                Column::new("z".into(), obj_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_objectives = Some(df);
        }

        if load_boss_kills {
            let df = df_from_columns(vec![
                Column::new("tick".into(), bk_ticks),
                Column::new("objective_team".into(), bk_objective_teams),
                Column::new("objective_id".into(), bk_objective_ids),
                Column::new("entity_class".into(), bk_entity_classes),
                Column::new("gametime".into(), bk_gametimes),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_boss_kills = Some(df);
        }

        if load_mid_boss {
            let df = df_from_columns(vec![
                Column::new("tick".into(), mb_ticks),
                Column::new("hero_id".into(), mb_hero_ids),
                Column::new("team_num".into(), mb_team_nums),
                Column::new("event".into(), mb_events),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_mid_boss = Some(df);
        }

        if load_troopers {
            let df = df_from_columns(vec![
                Column::new("tick".into(), tr_tick),
                Column::new("trooper_type".into(), tr_type),
                Column::new("team_num".into(), tr_team_num),
                Column::new("lane".into(), tr_lane),
                Column::new("health".into(), tr_health),
                Column::new("max_health".into(), tr_max_health),
                Column::new("x".into(), tr_x),
                Column::new("y".into(), tr_y),
                Column::new("z".into(), tr_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_troopers = Some(df);
        }

        if load_neutrals {
            let df = df_from_columns(vec![
                Column::new("tick".into(), nt_tick),
                Column::new("neutral_type".into(), nt_type),
                Column::new("team_num".into(), nt_team_num),
                Column::new("health".into(), nt_health),
                Column::new("max_health".into(), nt_max_health),
                Column::new("x".into(), nt_x),
                Column::new("y".into(), nt_y),
                Column::new("z".into(), nt_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_neutrals = Some(df);
        }

        if load_stat_modifier_events {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sm_tick),
                Column::new("hero_id".into(), sm_hero_id),
                Column::new("stat_type".into(), sm_stat_type),
                Column::new("amount".into(), sm_amount),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_stat_modifier_events = Some(df);
        }

        if load_active_modifiers {
            let df = df_from_columns(vec![
                Column::new("tick".into(), am_tick),
                Column::new("hero_id".into(), am_hero_id),
                Column::new("event".into(), am_event),
                Column::new("modifier_id".into(), am_modifier_id),
                Column::new("ability_id".into(), am_ability_id),
                Column::new("duration".into(), am_duration),
                Column::new("caster_hero_id".into(), am_caster_hero_id),
                Column::new("stacks".into(), am_stacks),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_active_modifiers = Some(df);
        }

        if load_urn {
            let df = df_from_columns(vec![
                Column::new("tick".into(), urn_tick),
                Column::new("event".into(), urn_event),
                Column::new("hero_id".into(), urn_hero_id),
                Column::new("team_num".into(), urn_team_num),
                Column::new("x".into(), urn_x),
                Column::new("y".into(), urn_y),
                Column::new("z".into(), urn_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_urn = Some(df);
        }

        if load_street_brawl_ticks {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sbt_tick),
                Column::new("round".into(), sbt_round),
                Column::new("state".into(), sbt_state),
                Column::new("amber_score".into(), sbt_amber_score),
                Column::new("sapphire_score".into(), sbt_sapphire_score),
                Column::new("buy_countdown".into(), sbt_buy_countdown),
                Column::new("next_state_time".into(), sbt_next_state_time),
                Column::new("state_start_time".into(), sbt_state_start_time),
                Column::new("non_combat_time".into(), sbt_non_combat_time),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_street_brawl_ticks = Some(df);
        }

        if load_street_brawl_rounds {
            let df = df_from_columns(vec![
                Column::new("round".into(), sbr_round),
                Column::new("tick".into(), sbr_tick),
                Column::new("scoring_team".into(), sbr_scoring_team),
                Column::new("amber_score".into(), sbr_amber_score),
                Column::new("sapphire_score".into(), sbr_sapphire_score),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_street_brawl_rounds = Some(df);
        }

        Ok(())
    }

    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns a DataFrame with 48 columns covering position, health, combat
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

    /// Ability usage events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn abilities(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_abilities.is_none() {
            self.load(vec!["abilities".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_abilities.clone().unwrap()))
    }

    /// Hero ability upgrade events (skill point spending) as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``tier``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn ability_upgrades(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_ability_upgrades.is_none() {
            self.load(vec!["ability_upgrades".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_ability_upgrades.clone().unwrap()))
    }

    /// Item purchase/sell/upgrade events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``change``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn item_purchases(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_item_purchases.is_none() {
            self.load(vec!["item_purchases".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_item_purchases.clone().unwrap()))
    }

    /// Chat messages as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``text``, ``chat_type``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn chat(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_chat.is_none() {
            self.load(vec!["chat".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_chat.clone().unwrap()))
    }

    /// Objective health state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``objective_type``, ``team_num``, ``lane``, ``health``, ``max_health``, ``phase``, ``x``, ``y``, ``z``.
    /// Emits a row when an objective's health or max_health changes.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn objectives(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_objectives.is_none() {
            self.load(vec!["objectives".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_objectives.clone().unwrap()))
    }

    /// Mid boss lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``team_num``, ``event``.
    /// Events: ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn mid_boss(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_mid_boss.is_none() {
            self.load(vec!["mid_boss".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_mid_boss.clone().unwrap()))
    }

    /// Objective destruction events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``objective_team``, ``objective_id``, ``entity_class``,
    /// ``gametime``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn boss_kills(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_boss_kills.is_none() {
            self.load(vec!["boss_kills".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_boss_kills.clone().unwrap()))
    }

    /// Per-tick alive lane trooper state as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``trooper_type``, ``team_num``, ``lane``,
    /// ``health``, ``max_health``, ``x``, ``y``, ``z``.
    ///
    /// Tracks ``CNPC_Trooper`` and ``CNPC_TrooperBoss`` only. Emits a row
    /// for every alive trooper at every tick.
    ///
    /// **Warning:** This dataset is large. It is not loaded by default.
    /// Access this property or call ``load("troopers")`` explicitly.
    #[getter]
    fn troopers(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_troopers.is_none() {
            self.load(vec!["troopers".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_troopers.clone().unwrap()))
    }

    /// Neutral creep state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``neutral_type``, ``team_num``,
    /// ``health``, ``max_health``, ``x``, ``y``, ``z``.
    ///
    /// Tracks ``CNPC_TrooperNeutral`` and ``CNPC_TrooperNeutralNodeMover``.
    /// Only emits a row when an alive neutral's state changes (health,
    /// position), significantly reducing data volume.
    ///
    /// **Note:** Not loaded by default. Access this property or call
    /// ``load("neutrals")`` explicitly.
    #[getter]
    fn neutrals(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_neutrals.is_none() {
            self.load(vec!["neutrals".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_neutrals.clone().unwrap()))
    }

    /// Permanent stat bonus change events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``stat_type``, ``amount``.
    ///
    /// ``stat_type`` is one of: ``"health"``, ``"spirit_power"``, ``"fire_rate"``,
    /// ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``.
    /// ``amount`` is the increase from this event.
    ///
    /// Emits a row whenever a stat total changes (idol/breakable pickups).
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn stat_modifier_events(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_stat_modifier_events.is_none() {
            self.load(vec!["stat_modifier_events".to_string()])?;
        }
        Ok(PyDataFrame(
            self.cached_stat_modifier_events.clone().unwrap(),
        ))
    }

    /// Active buff/debuff modifier events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``event``, ``modifier_id``, ``ability_id``,
    /// ``duration``, ``caster_hero_id``, ``stacks``.
    ///
    /// Events: ``"applied"`` when a modifier is first seen on a player,
    /// ``"removed"`` when it disappears.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn active_modifiers(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_active_modifiers.is_none() {
            self.load(vec!["active_modifiers".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_active_modifiers.clone().unwrap()))
    }

    /// Urn (idol) lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``event``, ``hero_id``, ``team_num``, ``x``, ``y``, ``z``.
    ///
    /// Events: ``"picked_up"`` when a player grabs it, ``"dropped"`` when
    /// the carrier loses it, ``"returned"`` when the urn is delivered,
    /// ``"delivery_active"`` when a delivery point activates,
    /// ``"delivery_inactive"`` when a delivery point deactivates.
    ///
    /// For modifier events (``picked_up``, ``dropped``, ``returned``),
    /// ``team_num``/``x``/``y``/``z`` are 0. For delivery events, ``hero_id`` is 0.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn urn(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_urn.is_none() {
            self.load(vec!["urn".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_urn.clone().unwrap()))
    }

    /// Per-tick street brawl state as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``round``, ``state``, ``amber_score``,
    /// ``sapphire_score``, ``buy_countdown``, ``next_state_time``,
    /// ``state_start_time``, ``non_combat_time``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Auto-loads on first access if not already loaded via ``load()``.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    fn street_brawl_ticks(&mut self) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_ticks.is_none() {
            self.load(vec!["street_brawl_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_street_brawl_ticks.clone().unwrap()))
    }

    /// Street brawl round scoring events as a Polars DataFrame.
    ///
    /// Columns: ``round``, ``tick``, ``scoring_team``, ``amber_score``,
    /// ``sapphire_score``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Auto-loads on first access if not already loaded via ``load()``.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    fn street_brawl_rounds(&mut self) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_rounds.is_none() {
            self.load(vec!["street_brawl_rounds".to_string()])?;
        }
        Ok(PyDataFrame(
            self.cached_street_brawl_rounds.clone().unwrap(),
        ))
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

    /// Scan for the GameOver event if not already done.
    /// Uses the lightweight events-only parser pass.
    fn ensure_always_events_scanned(&mut self) -> PyResult<()> {
        if self.game_over_scanned {
            return Ok(());
        }
        let events = self.parser.events(None).map_err(to_py_err)?;
        for event in &events {
            if event.msg_type == Msg::KEUserMsgGameOver as u32
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMessageGameOver::decode(event.payload.as_slice())
            {
                self.game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
            }
        }
        self.game_over_scanned = true;
        Ok(())
    }
}

/// Return a mapping of hero ID to hero name.
///
/// Returns:
///     A dict mapping hero IDs (int) to hero names (str).
#[pyfunction]
fn hero_names() -> HashMap<i64, &'static str> {
    boon_parser::all_heroes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of team number to team name.
///
/// Returns:
///     A dict mapping team numbers (int) to team names (str).
#[pyfunction]
fn team_names() -> HashMap<i64, &'static str> {
    boon_parser::all_teams()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of ability hash ID to ability name.
///
/// Returns:
///     A dict mapping MurmurHash2 ability IDs (int) to ability names (str).
#[pyfunction]
fn ability_names() -> HashMap<u32, &'static str> {
    boon_parser::all_abilities()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of game mode ID to game mode name.
///
/// Returns:
///     A dict mapping game mode IDs (int) to game mode names (str).
#[pyfunction]
fn game_mode_names() -> HashMap<i64, &'static str> {
    boon_parser::all_game_modes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of modifier hash ID to modifier name.
///
/// Returns:
///     A dict mapping MurmurHash2 modifier IDs (int) to modifier names (str).
#[pyfunction]
fn modifier_names() -> HashMap<u32, &'static str> {
    boon_parser::all_modifiers()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Python bindings for the boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add_function(wrap_pyfunction!(hero_names, m)?)?;
    m.add_function(wrap_pyfunction!(team_names, m)?)?;
    m.add_function(wrap_pyfunction!(ability_names, m)?)?;
    m.add_function(wrap_pyfunction!(modifier_names, m)?)?;
    m.add_function(wrap_pyfunction!(game_mode_names, m)?)?;
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
