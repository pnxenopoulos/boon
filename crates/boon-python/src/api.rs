use crate::*;

#[pymethods]
impl Demo {
    #[new]
    pub(crate) fn new(path: &str) -> PyResult<Self> {
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
            cached_summary: None,
            game_over: None,
            game_over_match_clock: None,
            game_over_match_clock_scanned: false,
            banned_hero_ids: None,
            always_events_scanned: false,
            cached_abilities: None,
            cached_flex_slots: None,
            cached_ability_upgrades: None,
            cached_item_purchases: None,
            cached_chat: None,
            cached_objectives: None,
            cached_mid_boss: None,
            cached_troopers: None,
            cached_neutrals: None,
            cached_breakables: None,
            cached_sinners_sacrifice: None,
            cached_stat_modifier_events: None,
            cached_active_modifiers: None,
            cached_ability_ticks: None,
            cached_players: None,
            cached_street_brawl_ticks: None,
            cached_street_brawl_rounds: None,
            cached_urn: None,
            cached_rift: None,
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
    pub(crate) fn verify(&self) -> PyResult<bool> {
        self.parser.verify().map_err(to_py_err)?;
        Ok(true)
    }

    /// The path to the demo file.
    #[getter]
    pub(crate) fn path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pathlib = py.import("pathlib")?;
        let path = pathlib
            .getattr("Path")?
            .call1((self.path.to_string_lossy().to_string(),))?;
        Ok(path.unbind())
    }

    /// The total number of ticks in the demo.
    #[getter]
    pub(crate) fn total_ticks(&self) -> i32 {
        self.total_ticks
    }

    /// The total duration of the demo in seconds.
    #[getter]
    pub(crate) fn total_seconds(&self) -> f32 {
        self.playback_time
    }

    /// The total duration of the demo as a formatted string (for example, "12:34").
    #[getter]
    pub(crate) fn total_clock_time(&self) -> String {
        let total_seconds = self.playback_time as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{minutes}:{seconds:02}")
    }

    /// The build number of the game that recorded the demo.
    #[getter]
    pub(crate) fn build(&self) -> i32 {
        self.build
    }

    /// The name of the map the demo was recorded on.
    #[getter]
    pub(crate) fn map_name(&self) -> String {
        self.map_name.clone()
    }

    /// The match ID for this demo, or ``None`` if the demo does not carry one
    /// (for example a partial capture or sandbox / custom content).
    #[getter]
    pub(crate) fn match_id(&self) -> Option<u64> {
        self.match_id
    }

    /// The game mode ID for this demo.
    ///
    /// Use ``game_mode_names()`` to resolve IDs to names.
    #[getter]
    pub(crate) fn game_mode(&self) -> i64 {
        self.game_mode
    }

    /// The tick rate of the demo (ticks per second).
    #[getter]
    pub(crate) fn tick_rate(&self) -> i32 {
        self.tick_rate
    }

    /// Parse the post-match summary from the demo's ``PostMatchDetails`` event.
    ///
    /// Returns a dictionary with four top-level keys:
    ///
    /// - ``snapshots``: a Polars DataFrame with one row per (snapshot, player).
    ///   Snapshots are taken at intervals through the match (not every minute);
    ///   ``snapshot_time_s`` marks each one. Columns hold that player's running
    ///   totals at that time: ``hero_id``, ``kills``, ``deaths``, ``assists``,
    ///   ``net_worth``, ``denies``, ``level``, ``lane``, ``creep_kills``,
    ///   ``neutral_kills``, ``player_damage``, and the per-source gold/orbs
    ///   breakdown.
    /// - ``last_hits``: a Polars DataFrame of ``hero_id`` and ``last_hits`` (the
    ///   final scoreboard last-hit / souls-secured total, which is only recorded
    ///   per match, not per snapshot).
    /// - ``objectives``: a Polars DataFrame of post-match objective records
    ///   (lane/team objectives, destruction time, and damage taken).
    /// - ``damage``: a Polars DataFrame of the damage matrix — one row per
    ///   (dealer, target, source, sample). Dealer/target are given as both
    ///   ``*_player_slot`` and resolved ``*_hero_id`` (null for non-player slots
    ///   like 0), so it joins to the other frames on ``hero_id``. ``damage`` is
    ///   the per-interval (additive) amount for that ``stat_type`` (a string:
    ///   ``damage``, ``healing``, ``mitigated``, …) dealt during the interval
    ///   ending at ``sample_time_s``. Each hit is recorded under both a coarse
    ///   category (``is_category`` true) and a specific source, so filter to
    ///   ``is_category == False`` to avoid double-counting, then ``sum``.
    ///
    /// The decoded message and all four frames are cached after the first call;
    /// repeated calls do not parse the demo or rebuild the frames.
    ///
    /// Raises ``DemoMessageError`` if the demo contains no post-match details
    /// (for example, an incomplete recording).
    pub(crate) fn summary(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        use boon_proto::proto::{CCitadelUserMsgPostMatchDetails, CMsgMatchMetaDataContents};

        if self.cached_summary.is_none() {
            let frames = py.detach(|| {
                let event_types = HashSet::from([Msg::KEUserMsgPostMatchDetails as u32]);
                let events = self
                    .parser
                    .events_filtered(None, &event_types)
                    .map_err(to_py_err)?;
                let event = events
                    .iter()
                    .find(|e| e.msg_type == Msg::KEUserMsgPostMatchDetails as u32)
                    .ok_or_else(|| {
                        DemoMessageError::new_err("no PostMatchDetails event found in demo")
                    })?;

                let outer = CCitadelUserMsgPostMatchDetails::decode(event.payload.as_slice())
                    .map_err(|e| {
                        DemoMessageError::new_err(format!("failed to decode PostMatchDetails: {e}"))
                    })?;
                let details_bytes = outer.match_details.as_ref().ok_or_else(|| {
                    DemoMessageError::new_err("PostMatchDetails has no match_details bytes")
                })?;
                let contents = CMsgMatchMetaDataContents::decode(details_bytes.as_slice())
                    .map_err(|e| {
                        DemoMessageError::new_err(format!("failed to decode match metadata: {e}"))
                    })?;
                let match_info = contents
                    .match_info
                    .ok_or_else(|| DemoMessageError::new_err("match metadata has no match_info"))?;

                let to_df_err = |e: PolarsError| {
                    DemoMessageError::new_err(format!("failed to build summary: {e}"))
                };
                Ok::<SummaryFrames, PyErr>(SummaryFrames {
                    snapshots: build_snapshots_frame(&match_info).map_err(to_df_err)?,
                    last_hits: build_last_hits_frame(&match_info).map_err(to_df_err)?,
                    objectives: build_objectives_frame(&match_info).map_err(to_df_err)?,
                    damage: build_damage_frame(&match_info).map_err(to_df_err)?,
                })
            })?;
            self.cached_summary = Some(frames);
        }

        let frames = self
            .cached_summary
            .as_ref()
            .expect("summary cache populated");
        let dict = PyDict::new(py);
        dict.set_item("snapshots", PyDataFrame(frames.snapshots.clone()))?;
        dict.set_item("last_hits", PyDataFrame(frames.last_hits.clone()))?;
        dict.set_item("objectives", PyDataFrame(frames.objectives.clone()))?;
        dict.set_item("damage", PyDataFrame(frames.damage.clone()))?;
        Ok(dict.into_any().unbind())
    }

    /// Snapshot per-tick state at selected ticks in a single parallel pass.
    ///
    /// Decode the demo once. Process keyframe segments in parallel.
    /// Collect rows only at selected ticks. This uses less memory and time than
    /// a full per-tick frame that you filter in Python.
    ///
    /// Args:
    ///     datasets: Which snapshot dataset(s) to return — ``"player_ticks"``
    ///         (default), ``"world_ticks"``, ``"troopers"``, or a list of them.
    ///     ticks: A specific tick or list of ticks.
    ///     every: Sample every ``N`` ticks (gap-robust stride).
    ///     seconds: Sample about once per ``seconds`` (converted with the tick rate).
    ///         Mutually exclusive with ``every``.
    ///     events: Sample at the ticks of these event datasets (for example ``"kills"``
    ///         or ``["kills", "damage"]``).
    ///     start_tick, end_tick: Restrict to a contiguous ``[start, end]`` window.
    ///
    /// A window without another selector returns each tick in the window.
    /// A request without a selector is an error. Return one DataFrame for one
    /// dataset. Return a dictionary for multiple datasets.
    ///
    /// Example:
    ///     >>> demo.snapshots(every=64)                     # ~1 row/sec of ticks
    ///     >>> demo.snapshots(ticks=[29000, 30000])         # specific ticks
    ///     >>> demo.snapshots("troopers", events="kills")   # troopers at kill ticks
    ///     >>> demo.snapshots(["player_ticks", "world_ticks"], seconds=1.0)
    #[pyo3(signature = (datasets=None, *, ticks=None, every=None, seconds=None, events=None, start_tick=None, end_tick=None))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn snapshots(
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
        for n in &names {
            match n.as_str() {
                "player_ticks" => wants.player_ticks = true,
                "world_ticks" => wants.world_ticks = true,
                "troopers" => wants.troopers = true,
                other => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "snapshots(): datasets must be player_ticks / world_ticks / troopers, got '{other}'"
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

        // Event-dataset loading, tick indexing, seeking, and the snapshot decode
        // are all pure Rust work. Keep only the final Python object conversion
        // under the interpreter lock.
        let (pt, wt, tr) = py.detach(|| {
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

            // Fast path: a single tick with no stride or window seeks directly
            // (`parse_to_tick`) instead of decoding the whole demo.
            if stride.is_none() && !has_window && tick_set.len() == 1 {
                let t = *tick_set.iter().next().expect("len == 1");
                self.snapshot_at_tick(t, wants)
            } else {
                // The tick predicate is the union of stride-sampled and explicit
                // event ticks, restricted to the requested window.
                let start = start_tick.unwrap_or(i32::MIN);
                let end = end_tick.unwrap_or(i32::MAX);
                let pred = if stride.is_none() && tick_set.is_empty() {
                    TickPredicate::Window { start, end }
                } else {
                    let mut sampled = tick_set;
                    if let Some(step) = stride {
                        let mut last: Option<i32> = None;
                        for t in self.parser.distinct_ticks().map_err(to_py_err)? {
                            if last.is_none_or(|l| t - l >= step) {
                                sampled.insert(t);
                                last = Some(t);
                            }
                        }
                    }
                    TickPredicate::Set {
                        ticks: sampled,
                        start,
                        end,
                    }
                };
                self.build_snapshots_parallel(wants, &pred)
            }
        })?;
        let frame_for = |name: &str| -> Option<DataFrame> {
            match name {
                "player_ticks" => pt.clone(),
                "world_ticks" => wt.clone(),
                "troopers" => tr.clone(),
                _ => None,
            }
        };

        if names.len() == 1 {
            let df = frame_for(&names[0]).unwrap();
            PyDataFrame(df).into_py_any(py)
        } else {
            let dict = PyDict::new(py);
            for n in &names {
                dict.set_item(n, PyDataFrame(frame_for(n).unwrap()))?;
            }
            Ok(dict.into_any().unbind())
        }
    }

    /// Sample native, persistent baseline, and effective player stats.
    ///
    /// Create only the requested stats and ticks. Express percentage stats in
    /// percentage points. Express spirit power in points. A complete column is
    /// false when the catalog cannot calculate a known active formula.
    #[pyo3(signature = (stats, *, ticks=None, every=None, seconds=None, events=None, start_tick=None, end_tick=None))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stat_ticks(
        &mut self,
        py: Python<'_>,
        stats: StrOrList,
        ticks: Option<IntOrList>,
        every: Option<i32>,
        seconds: Option<f32>,
        events: Option<StrOrList>,
        start_tick: Option<i32>,
        end_tick: Option<i32>,
    ) -> PyResult<PyDataFrame> {
        let names = stats.into_vec();
        if names.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "stat_ticks(): stats must not be empty",
            ));
        }
        let mut selected = boon_parser::StatMask::default();
        for name in names {
            let Some(stat) = boon_parser::StatId::from_name(&name) else {
                let valid: Vec<_> = boon_parser::StatId::ALL
                    .into_iter()
                    .map(boon_parser::StatId::name)
                    .collect();
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "stat_ticks(): unknown stat {name:?}; valid stats: {valid:?}"
                )));
            };
            selected.insert(stat);
        }

        if every.is_some() && seconds.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "stat_ticks(): pass either every or seconds, not both",
            ));
        }
        let stride = match (every, seconds) {
            (Some(step), _) if step < 1 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "stat_ticks(): every must be at least 1",
                ));
            }
            (Some(step), _) => Some(step),
            (_, Some(value)) if !value.is_finite() || value <= 0.0 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "stat_ticks(): seconds must be positive",
                ));
            }
            (_, Some(value)) => Some(((value * self.tick_rate as f32).round() as i32).max(1)),
            _ => None,
        };
        let explicit = ticks.map(IntOrList::into_vec).unwrap_or_default();
        let event_names = events.map(StrOrList::into_vec);

        let frame = py.detach(|| {
            let mut tick_set = HashSet::new();
            if let Some(names) = event_names.as_deref() {
                tick_set = self.event_ticks(names)?;
            }
            tick_set.extend(explicit);

            let has_window = start_tick.is_some() || end_tick.is_some();
            if stride.is_none() && tick_set.is_empty() && !has_window {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "stat_ticks(): select ticks, a stride, events, or a tick window",
                ));
            }
            if stride.is_none() && !has_window && tick_set.len() == 1 {
                return self.stat_ticks_at(
                    *tick_set.iter().next().expect("one selected tick"),
                    selected,
                );
            }

            let start = start_tick.unwrap_or(i32::MIN);
            let end = end_tick.unwrap_or(i32::MAX);
            let predicate = if stride.is_none() && tick_set.is_empty() {
                TickPredicate::Window { start, end }
            } else {
                if let Some(step) = stride {
                    let mut last = None;
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
            self.build_stat_ticks_parallel(selected, &predicate)
        })?;
        Ok(PyDataFrame(frame))
    }

    /// Explain generated item and modifier contributions to tracked stats.
    ///
    /// The result is change-oriented: item purchases/removals and relevant
    /// modifier apply/change/remove transitions each produce one row per
    /// affected stat. Passing no stats collects the complete supported set.
    #[pyo3(signature = (stats=None))]
    pub(crate) fn stat_effects(
        &self,
        py: Python<'_>,
        stats: Option<StrOrList>,
    ) -> PyResult<PyDataFrame> {
        let selected = if let Some(stats) = stats {
            let names = stats.into_vec();
            if names.is_empty() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "stat_effects(): stats must not be empty",
                ));
            }
            let mut selected = boon_parser::StatMask::default();
            for name in names {
                let Some(stat) = boon_parser::StatId::from_name(&name) else {
                    let valid: Vec<_> = boon_parser::StatId::ALL
                        .into_iter()
                        .map(boon_parser::StatId::name)
                        .collect();
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "stat_effects(): unknown stat {name:?}; valid stats: {valid:?}"
                    )));
                };
                selected.insert(stat);
            }
            selected
        } else {
            boon_parser::StatMask::ALL
        };

        py.detach(|| self.build_stat_effects(selected))
            .map(PyDataFrame)
    }

    /// Convert a tick number to seconds elapsed, excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    pub(crate) fn tick_to_seconds(&mut self, tick: i32) -> PyResult<f64> {
        if self.tick_rate == 0 {
            return Ok(0.0);
        }
        self.ensure_paused_ticks_built()?;
        let active_ticks = self.count_active_ticks(tick);
        Ok(active_ticks as f64 / self.tick_rate as f64)
    }

    /// Convert a tick number to a clock time string (for example, ``"03:14"``),
    /// excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    pub(crate) fn tick_to_clock_time(&mut self, tick: i32) -> PyResult<String> {
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
    /// - start_lane: The player's original lane color
    ///   (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the `CMsgLaneColor` proto enum)
    /// - rank: The player's packed competitive display rank (0 means unranked,
    ///   calibrating, or unavailable)
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
    /// Returns a DataFrame with columns:
    /// - hero_id: The banned hero's ID (joins to ``players.hero_id``)
    /// - hero_name: The resolved hero name, or ``"HERO_NOT_FOUND"`` for an ID
    ///   that predates the bundled hero table
    ///
    /// Read the ``BannedHeroes`` user message. The server can send this
    /// message once before the match starts. The message contains only hero
    /// IDs. It does not contain the team, banning player, or draft order.
    /// Therefore, Boon cannot build a draft.
    ///
    /// An empty DataFrame means that the demo contains no ban data. It does not
    /// prove that the match had no bans. The demo cannot distinguish a match
    /// without bans from a server build that did not send the message.
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
        let names: Vec<&'static str> = ids.iter().map(|&id| boon_parser::hero_name(id)).collect();
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
    #[staticmethod]
    pub(crate) fn available_datasets() -> Vec<&'static str> {
        let mut all = VALID_DATASETS.to_vec();
        all.extend_from_slice(VALID_STREET_BRAWL_DATASETS);
        all
    }
}
