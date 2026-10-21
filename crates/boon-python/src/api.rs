use crate::*;

#[pymethods]
impl Demo {
    /// Open one Valve Deadlock Source 2 replay with Boon.
    ///
    /// Args:
    ///     path: String path to a local .dem file.
    ///     preload: Load kills, damage, and abilities during construction when True. False keeps dataset loading deferred.
    ///
    /// Returns:
    ///     None.
    ///
    /// Raises:
    ///     FileNotFoundError: The file does not exist.
    ///     InvalidDemoError: The file is not a supported replay.
    ///     DemoHeaderError: Required header fields are absent.
    ///     DemoInfoError: Required file-info fields are absent.
    #[new]
    #[pyo3(signature = (path, *, preload = true))]
    pub(crate) fn new(py: Python<'_>, path: &str, preload: bool) -> PyResult<Self> {
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

        // Parse the first tick. Get match_id and game_mode from
        // CCitadelGameRulesProxy when they are available. A partial capture or
        // custom demo can omit the match ID. Store `None` in that case. Use 0
        // for game_mode when it is not available.
        let ctx = parser.parse_to_tick(1).map_err(to_py_err)?;

        let game_rules = ctx
            .entities()
            .iter()
            .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy");

        let match_id = game_rules.and_then(|(_, e)| {
            let serializer = ctx.serializers().get(&e.class_name)?;
            let mid_key = serializer.resolve_field_key("m_pGameRules.m_unMatchID")?;
            match e.fields.get(&mid_key)? {
                boon_parser::FieldValue::U64(id) => Some(*id),
                boon_parser::FieldValue::I64(id) => Some(*id as u64),
                _ => None,
            }
        });

        let game_mode = game_rules
            .and_then(|(_, e)| {
                let serializer = ctx.serializers().get(&e.class_name)?;
                Some(e.get_i64(serializer.resolve_field_key("m_pGameRules.m_eGameMode")))
            })
            .unwrap_or(0);

        let tick_rate = if playback_time > 0.0 {
            (total_ticks as f32 / playback_time).round() as i32
        } else {
            0
        };

        let mut demo = Demo {
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
            cached_datasets: DatasetCache::default(),
            cached_barriers: std::sync::OnceLock::new(),
            cached_summary: None,
            cached_ziplines: std::sync::OnceLock::new(),
            game_over: None,
            game_over_match_clock: None,
            game_over_match_clock_scanned: false,
            banned_hero_ids: None,
            always_events_scanned: false,
            cached_players: None,
        };
        if preload {
            demo.load_datasets(py, &[Dataset::Kills, Dataset::Damage, Dataset::Abilities])?;
        }
        Ok(demo)
    }

    /// Verify that the file is a valid demo file.
    ///
    /// Returns:
    ///     True if the file is valid.
    ///
    /// Notes:
    ///     This is already called during construction, so it will always
    ///     return True for an existing Demo instance.
    pub(crate) fn verify(&self) -> PyResult<bool> {
        self.parser.verify().map_err(to_py_err)?;
        Ok(true)
    }

    /// The path to the demo file.
    ///
    /// Returns:
    ///     Path: The path to the demo file.
    #[getter]
    pub(crate) fn path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pathlib = py.import("pathlib")?;
        let path = pathlib
            .getattr("Path")?
            .call1((self.path.to_string_lossy().to_string(),))?;
        Ok(path.unbind())
    }

    /// The total number of ticks in the demo.
    ///
    /// Returns:
    ///     int: The total number of ticks in the demo.
    #[getter]
    pub(crate) fn total_ticks(&self) -> i32 {
        self.total_ticks
    }

    /// The total duration of the demo in seconds.
    ///
    /// Returns:
    ///     float: The total duration of the demo in seconds.
    #[getter]
    pub(crate) fn total_seconds(&self) -> f32 {
        self.playback_time
    }

    /// The total duration of the demo as a formatted string (for example, ``"12:34"``).
    ///
    /// Returns:
    ///     str: The total duration of the demo as a formatted string (for example, ``"12:34"``).
    #[getter]
    pub(crate) fn total_clock_time(&self) -> String {
        let total_seconds = self.playback_time as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{minutes}:{seconds:02}")
    }

    /// The build number of the game that recorded the demo.
    ///
    /// Returns:
    ///     int: The build number of the game that recorded the demo.
    #[getter]
    pub(crate) fn build(&self) -> i32 {
        self.build
    }

    /// The name of the map the demo was recorded on.
    ///
    /// Returns:
    ///     str: The name of the map the demo was recorded on.
    #[getter]
    pub(crate) fn map_name(&self) -> String {
        self.map_name.clone()
    }

    /// Read the match ID recorded in the replay.
    ///
    /// Returns:
    ///     Match ID, or None when the replay omits it.
    ///
    /// Notes:
    ///     Partial captures and custom demos can omit the match ID.
    #[getter]
    pub(crate) fn match_id(&self) -> Option<u64> {
        self.match_id
    }

    /// The game mode ID for this demo.
    ///
    /// Returns:
    ///     int: The game mode ID for this demo.
    ///
    /// Notes:
    ///     Use ``game_mode_names()`` to resolve IDs to names.
    #[getter]
    pub(crate) fn game_mode(&self) -> i64 {
        self.game_mode
    }

    /// The tick rate of the demo (ticks per second).
    ///
    /// Returns:
    ///     int: The tick rate of the demo (ticks per second).
    #[getter]
    pub(crate) fn tick_rate(&self) -> i32 {
        self.tick_rate
    }

    /// Read the recorded Deadlock PostMatchDetails message.
    ///
    /// Returns:
    ///     Dictionary of six cached Polars DataFrames:
    ///
    ///     - ``snapshots``: cumulative player counters and state at ``snapshot_time_s``.
    ///       Includes ``steam_id``, ``hero_id``, damage by target type, damage taken,
    ///       ``player_healing``, ``teammate_healing``, and ``self_healing``.
    ///       ``barrier_absorption`` is damage stopped by barriers this player provided
    ///       (recorded ``player_barriering``). ``damage_absorbed`` is damage stopped
    ///       by barriers on this player. Healing and barrier absorption are separate.
    ///       Added counters are null when absent.
    ///     - ``gold_sources``: cumulative ``gold``, ``gold_orbs``, ``kills``, and ``damage``
    ///       for each player, snapshot, and source. Includes ``source_id`` and the protobuf
    ///       ``source_name``. Unknown IDs and absent counters remain available.
    ///     - ``last_hits``: final ``hero_id`` and ``last_hits`` totals.
    ///     - ``objectives``: recorded objective times and damage.
    ///     - ``damage``: the source-to-target matrix at each ``sample_time_s``.
    ///       ``damage`` is the interval amount; ``total`` is the recorded cumulative value.
    ///       The interval starts at ``interval_start_s``. Select ``stat_type`` for damage,
    ///       healing, regeneration, or another recorded statistic. Category rows
    ///       (``is_category=True``) duplicate specific sources; do not add them together.
    ///     - ``healing``: healing and regeneration rows without category duplicates.
    ///       Columns: ``interval_start_s``, ``interval_end_s``, ``healer_steam_id``, ``healer_hero_id``,
    ///       ``target_steam_id``, ``target_hero_id``, ``source_name``,
    ///       ``stat_type``, ``amount``, and ``total``. ``amount`` is the interval amount.
    ///       ``total`` is the recorded cumulative amount. Zero changes remain in the table.
    ///
    ///     Times use match-clock seconds. Snapshot and matrix reporting periods can differ.
    ///     Do not sum cumulative totals across periods. Use Steam IDs across hero changes;
    ///     hero IDs come from the match roster. These tables do not contain individual heals.
    ///
    /// Raises:
    ///     DemoMessageError: The post-match message is absent or invalid.
    pub(crate) fn summary(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.ensure_summary(py)?;

        let frames = self
            .cached_summary
            .as_ref()
            .expect("summary cache populated");
        let dict = PyDict::new(py);
        dict.set_item("snapshots", PyDataFrame(frames.snapshots.clone()))?;
        dict.set_item("last_hits", PyDataFrame(frames.last_hits.clone()))?;
        dict.set_item("objectives", PyDataFrame(frames.objectives.clone()))?;
        dict.set_item("damage", PyDataFrame(frames.damage.clone()))?;
        dict.set_item("healing", PyDataFrame(frames.healing.clone()))?;
        dict.set_item("gold_sources", PyDataFrame(frames.gold_sources.clone()))?;
        Ok(dict.into_any().unbind())
    }

    /// Read native sampled state before optional Python ammo calculation.
    ///
    /// Args:
    ///     datasets: Snapshot dataset name or names.
    ///     ticks: Exact demo ticks to read.
    ///     every: Positive tick stride.
    ///     seconds: Sampling interval in seconds.
    ///     events: Dataset name or names whose event ticks should be sampled.
    ///     start_tick: Inclusive lower tick bound.
    ///     end_tick: Inclusive upper tick bound.
    ///
    /// Returns:
    ///     pl.DataFrame | dict[str, pl.DataFrame]: Native results used by the public Python wrapper.
    #[pyo3(signature = (datasets=None, *, ticks=None, every=None, seconds=None, events=None, start_tick=None, end_tick=None))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn _snapshots(
        &mut self,
        py: Python<'_>,
        datasets: Option<StrOrList>,
        ticks: Option<IntOrList>,
        every: Option<i32>,
        seconds: Option<f32>,
        events: Option<StrOrList>,
        start_tick: Option<i32>,
        end_tick: Option<i32>,
    ) -> PyResult<Py<PyAny>> {
        // Requested datasets -> SnapWants (default player_ticks).
        let names = datasets
            .map(StrOrList::into_vec)
            .unwrap_or_else(|| vec!["player_ticks".to_string()]);
        if names.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "snapshots(): `datasets` must not be empty",
            ));
        }
        let mut wants = SnapWants::default();
        let mut wants_ziplines = false;
        for n in &names {
            match n.as_str() {
                "player_ticks" => wants.player_ticks = true,
                "world_ticks" => wants.world_ticks = true,
                "troopers" => wants.troopers = true,
                "ziplines" => wants_ziplines = true,
                other => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "snapshots(): datasets must be player_ticks / world_ticks / troopers / ziplines, got '{other}'"
                    )));
                }
            }
        }

        // Stride: `every` (ticks) or `seconds` (converted), mutually exclusive.
        if every.is_some() && seconds.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "snapshots(): pass either `every` or `seconds`, not both",
            ));
        }
        let stride: Option<i32> = match (every, seconds) {
            (Some(step), _) if step < 1 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "snapshots(): `every` must be >= 1 tick",
                ));
            }
            (Some(step), _) => Some(step),
            (_, Some(secs)) if !secs.is_finite() || secs <= 0.0 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "snapshots(): `seconds` must be a positive number",
                ));
            }
            (_, Some(secs)) => Some(((secs * self.tick_rate as f32).round() as i32).max(1)),
            (None, None) => None,
        };

        let explicit = ticks.map(IntOrList::into_vec).unwrap_or_default();
        let event_names = events.map(StrOrList::into_vec);
        let explicit_only =
            !explicit.is_empty() && event_names.as_ref().is_none_or(std::vec::Vec::is_empty);

        // Event-dataset loading, tick indexing, seeking, and the snapshot decode
        // are all pure Rust work. Keep only the final Python object conversion
        // under the interpreter lock.
        let ((pt, wt, tr), zl) = py.detach(|| {
            let mut tick_set: std::collections::HashSet<i32> = std::collections::HashSet::new();
            if let Some(names) = event_names.as_deref() {
                tick_set = self.event_ticks(names)?;
            }
            tick_set.extend(explicit);

            let has_window = start_tick.is_some() || end_tick.is_some();
            if stride.is_none() && tick_set.is_empty() && !has_window {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "snapshots(): specify at least one of `ticks`, `every`, `seconds`, \
                     `events`, or `start_tick` / `end_tick`",
                ));
            }

            let direct = stride.is_none()
                && !has_window
                && (tick_set.len() == 1 || (explicit_only && tick_set.len() <= 4));
            let start = start_tick.unwrap_or(i32::MIN);
            let end = end_tick.unwrap_or(i32::MAX);
            let predicate = if stride.is_none() && tick_set.is_empty() {
                TickPredicate::Window { start, end }
            } else {
                if let Some(step) = stride {
                    let mut last: Option<i32> = None;
                    for tick in self.parser.distinct_ticks().map_err(to_py_err)? {
                        if last.is_none_or(|previous| tick - previous >= step) {
                            tick_set.insert(tick);
                            last = Some(tick);
                        }
                    }
                }
                TickPredicate::Set {
                    ticks: tick_set,
                    start,
                    end,
                }
            };
            // Static map nodes need the serial history, even for exact seeks.
            // Other snapshot datasets keep their existing seek/parallel paths.
            let frames = if !wants.any() {
                (None, None, None)
            } else if direct {
                let TickPredicate::Set { ticks, .. } = &predicate else {
                    unreachable!()
                };
                self.snapshots_at_ticks(ticks.iter().copied().collect(), wants)?
            } else {
                self.build_snapshots_parallel(wants, &predicate)?
            };
            let ziplines = wants_ziplines
                .then(|| self.zipline_snapshot_frame(&predicate))
                .transpose()?;
            Ok::<_, PyErr>((frames, ziplines))
        })?;
        let frame_for = |name: &str| -> PyResult<DataFrame> {
            let frame = match name {
                "player_ticks" => pt.as_ref(),
                "world_ticks" => wt.as_ref(),
                "troopers" => tr.as_ref(),
                "ziplines" => zl.as_ref(),
                _ => None,
            };
            frame.cloned().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "snapshot dataset '{name}' was not built"
                ))
            })
        };

        if names.len() == 1 {
            let df = frame_for(&names[0])?;
            PyDataFrame(df).into_py_any(py)
        } else {
            let dict = PyDict::new(py);
            for n in &names {
                dict.set_item(n, PyDataFrame(frame_for(n)?))?;
            }
            Ok(dict.into_any().unbind())
        }
    }

    /// Return only player positions at the selected ticks for analysis helpers.
    #[pyo3(name = "_player_positions")]
    pub(crate) fn player_positions(&self, py: Python<'_>, ticks: Vec<i32>) -> PyResult<Py<PyAny>> {
        if ticks.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "_player_positions(): ticks must not be empty",
            ));
        }
        let frame = py.detach(|| self.build_player_positions(ticks))?;
        PyDataFrame(frame).into_py_any(py)
    }

    /// Convert a tick number to seconds elapsed, excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    ///
    /// Args:
    ///     tick: The game tick to convert.
    ///
    /// Returns:
    ///     The elapsed time in seconds, excluding pauses.
    pub(crate) fn tick_to_seconds(&mut self, tick: i32) -> PyResult<f64> {
        if self.tick_rate == 0 {
            return Ok(0.0);
        }
        self.ensure_paused_ticks_built()?;
        let active_ticks = self.count_active_ticks(tick);
        Ok(active_ticks as f64 / self.tick_rate as f64)
    }

    /// Convert a tick number to a clock time string (for example, ``"3:14"``), excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    ///
    /// Args:
    ///     tick: The game tick to convert.
    ///
    /// Returns:
    ///     A formatted string like ``"3:14"`` or ``"12:34"``.
    pub(crate) fn tick_to_clock_time(&mut self, tick: i32) -> PyResult<String> {
        let secs = self.tick_to_seconds(tick)?;
        let total_seconds = secs as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(format!("{minutes}:{seconds:02}"))
    }

    /// The pre-game lobby duration in seconds.
    ///
    /// Returns:
    ///     float | None: The pre-game lobby duration in seconds.
    ///
    /// Notes:
    ///     The demo starts recording during the pre-game, so tick 0 leads the on-screen
    ///     match clock (which reaches ``0:00`` only when the barrier drops and the game
    ///     begins) by this amount: ``match clock = tick_to_seconds - pregame_seconds``.
    ///     Derived from the replicated match clock at game over, not a heuristic. Returns
    ///     ``None`` if the demo has no game-over event, does not replicate the match clock
    ///     (older builds), or does not start in the pre-game (a recording that begins after
    ///     the barrier drop, where the offset would be negative).
    #[getter]
    pub(crate) fn pregame_seconds(&mut self) -> PyResult<Option<f64>> {
        if self.tick_rate == 0 {
            return Ok(None);
        }
        self.ensure_game_over_match_clock_scanned()?;
        let (Some((_, game_over_tick)), Some(match_clock)) =
            (self.game_over, self.game_over_match_clock)
        else {
            return Ok(None);
        };
        let elapsed = self.tick_to_seconds(game_over_tick)?;
        let pregame = elapsed - match_clock as f64;
        // A negative offset means the recording starts after the barrier drop, so it holds
        // no pre-game. The pre-game duration is then undeterminable.
        if pregame < 0.0 {
            return Ok(None);
        }
        Ok(Some(pregame))
    }

    /// Read the demo tick at match start.
    ///
    /// Returns:
    ///     Tick at match clock 0:00, or None when pregame_seconds is unavailable.
    ///
    /// Notes:
    ///     This is the end of the pregame lobby, when the starting barriers drop.
    #[getter]
    pub(crate) fn game_start_tick(&mut self) -> PyResult<Option<i32>> {
        let Some(pregame) = self.pregame_seconds()? else {
            return Ok(None);
        };
        Ok(Some((pregame * self.tick_rate as f64).round() as i32))
    }

    /// Convert a tick to on-screen match-clock seconds, excluding paused time.
    ///
    /// ``0.0`` at ``game_start_tick``; negative during the pre-game lobby (matching
    /// the spectator clock's pre-game countdown). Equal to
    /// ``tick_to_seconds(tick) - pregame_seconds``.
    ///
    /// Args:
    ///     tick: The game tick to convert.
    ///
    /// Returns:
    ///     Match-clock seconds, or ``None`` if ``pregame_seconds`` is unavailable.
    pub(crate) fn tick_to_match_seconds(&mut self, tick: i32) -> PyResult<Option<f64>> {
        let Some(pregame) = self.pregame_seconds()? else {
            return Ok(None);
        };
        Ok(Some(self.tick_to_seconds(tick)? - pregame))
    }

    /// Convert a tick to an on-screen match-clock string (for example, ``"3:14"``).
    ///
    /// Args:
    ///     tick: Nonnegative demo tick to convert.
    ///
    /// Returns:
    ///     str | None: Convert a tick to an on-screen match-clock string (for example, ``"3:14"``).
    ///
    /// Notes:
    ///     The formatted counterpart to ``tick_to_match_seconds``; pre-game ticks read as
    ///     a negative clock (for example, ``"-0:12"``). Returns ``None`` if
    ///     ``pregame_seconds`` is unavailable.
    pub(crate) fn tick_to_match_clock(&mut self, tick: i32) -> PyResult<Option<String>> {
        let Some(secs) = self.tick_to_match_seconds(tick)? else {
            return Ok(None);
        };
        let sign = if secs < 0.0 { "-" } else { "" };
        let total_seconds = secs.abs() as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(Some(format!("{sign}{minutes}:{seconds:02}")))
    }

    /// Player information as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **player_name** (*str*) -- The player's display name.
    ///     - **steam_id** (*int*) -- The player's Steam ID.
    ///     - **hero_id** (*int*) -- The player's hero ID.
    ///     - **team_num** (*int*) -- The player's raw team number.
    ///     - **start_lane** (*int*) -- The player's original lane color (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the ``CMsgLaneColor`` proto enum).
    ///     - **rank** (*int*) -- The player's packed competitive display rank; 0 means unranked, calibrating, or unavailable.
    #[getter]
    pub(crate) fn players(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if let Some(ref df) = self.cached_players {
            return Ok(PyDataFrame(df.clone()));
        }

        // The roster does not change after the match starts.
        // Read it from one tick.
        // Prefer the game-over tick because all roster fields are set.
        // This tick occurs before the game removes player controllers.
        // The final recorded tick can contain pre-game placeholder values.
        // It can also occur after some controllers are removed.
        // Use the final tick only when the demo has no game-over event.
        // An incomplete recording is one example.
        py.detach(|| self.ensure_always_events_scanned())?;
        let snapshot_tick = self.game_over.map_or(self.total_ticks, |(_, tick)| tick);
        let mut df = py.detach(|| self.collect_players_at(snapshot_tick))?;

        // Defensive: if that tick somehow had no controllers, try the other.
        if df.height() == 0 && snapshot_tick != self.total_ticks {
            df = py.detach(|| self.collect_players_at(self.total_ticks))?;
        }

        self.cached_players = Some(df.clone());
        Ok(PyDataFrame(df))
    }

    /// Heroes banned from this match as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **hero_id** (*int*) -- The banned hero's ID (joins to ``players.hero_id``).
    ///     - **hero_name** (*str*) -- The resolved hero name, or ``"HERO_NOT_FOUND"`` for an ID absent from the selected boon-data catalog.
    ///
    /// Notes:
    ///     Read from the single ``BannedHeroes`` user message, which the server
    ///     sends early in the demo (before the match starts) only when the match
    ///     has bans. The message carries nothing but the hero IDs -- no team, no
    ///     banning player, and no pick/ban ordering -- so this cannot be used to
    ///     build a draft.
    ///
    ///     An empty DataFrame means no bans were recorded for this match. Demos
    ///     from builds that never emit the message are indistinguishable from
    ///     ban-free matches, so treat empty as "nothing recorded" rather than as
    ///     positive proof that nothing was banned.
    #[getter]
    pub(crate) fn banned_heroes(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        py.detach(|| self.ensure_always_events_scanned())?;
        let ids: Vec<i64> = self
            .banned_hero_ids
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|&id| id as i64)
            .collect();
        let lookup: HashMap<i64, String> = if ids.is_empty() {
            HashMap::new()
        } else {
            py.import("boon")?
                .getattr("hero_names")?
                .call0()?
                .extract()?
        };
        let names: Vec<&str> = ids
            .iter()
            .map(|id| lookup.get(id).map_or("HERO_NOT_FOUND", String::as_str))
            .collect();
        let df = df_from_columns(vec![
            Column::new("hero_id".into(), ids),
            Column::new("hero_name".into(), names),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
        Ok(PyDataFrame(df))
    }

    /// Return the list of dataset names that can be passed to ``load()`` or accessed as properties.
    ///
    /// Returns:
    ///     A list of valid dataset name strings.
    ///
    /// Examples:
    ///     >>> Demo.available_datasets()
    ///     ['abilities', 'ability_upgrades', 'chat', ...]
    #[staticmethod]
    pub(crate) fn available_datasets() -> Vec<&'static str> {
        Dataset::ALL.into_iter().map(Dataset::as_str).collect()
    }
}
