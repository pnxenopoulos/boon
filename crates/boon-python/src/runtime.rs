use crate::*;

impl Demo {
    pub(super) fn ensure_snapshots_detached(
        &mut self,
        py: Python<'_>,
        wants: SnapWants,
    ) -> PyResult<()> {
        py.detach(|| self.ensure_snapshots(wants))
    }

    /// Build the paused_ticks cache from world_ticks if not already done.
    pub(super) fn ensure_paused_ticks_built(&mut self) -> PyResult<()> {
        if self.paused_ticks.is_some() {
            return Ok(());
        }
        // Ensure world_ticks is loaded
        if self.cached_world_ticks.is_none() {
            Python::attach(|py| self.load(py, vec!["world_ticks".to_string()]))?;
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
    pub(super) fn count_active_ticks(&self, tick: i32) -> i32 {
        let paused = self
            .paused_ticks
            .as_ref()
            .map(|pts| pts.partition_point(|&t| t < tick) as i32)
            .unwrap_or(0);
        (tick - paused).max(0)
    }

    /// Scan once for `GameOver` and `BannedHeroes` messages.
    ///
    /// Use the event-only parser pass.
    pub(super) fn ensure_always_events_scanned(&mut self) -> PyResult<()> {
        if self.always_events_scanned {
            return Ok(());
        }
        let event_types = HashSet::from([
            Msg::KEUserMsgGameOver as u32,
            Msg::KEUserMsgBannedHeroes as u32,
        ]);
        let events = self
            .parser
            .events_filtered(None, &event_types)
            .map_err(to_py_err)?;
        let mut banned: Vec<u32> = Vec::new();
        for event in &events {
            if event.msg_type == Msg::KEUserMsgGameOver as u32
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMessageGameOver::decode(event.payload.as_slice())
            {
                self.game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
            }
            if event.msg_type == Msg::KEUserMsgBannedHeroes as u32
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(event.payload.as_slice())
            {
                banned.extend(msg.banned_hero_ids);
            }
        }
        // A completed scan always sets Some.
        // An empty list means "no ban data" and not "not scanned."
        self.banned_hero_ids = Some(banned);
        self.always_events_scanned = true;
        Ok(())
    }

    /// Read the replicated match clock at the game-over tick once.
    pub(super) fn ensure_game_over_match_clock_scanned(&mut self) -> PyResult<()> {
        if self.game_over_match_clock_scanned {
            return Ok(());
        }

        self.ensure_always_events_scanned()?;
        self.game_over_match_clock = match self.game_over {
            Some((_, tick)) => self.match_clock_at(tick)?,
            None => None,
        };
        self.game_over_match_clock_scanned = true;
        Ok(())
    }

    /// Read the authoritative HUD match clock from the game-rules entity.
    fn match_clock_at(&self, tick: i32) -> PyResult<Option<f32>> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;
        let Some(serializer) = ctx.serializers().get("CCitadelGameRulesProxy") else {
            return Ok(None);
        };
        let Some(key) = serializer.resolve_field_key("m_pGameRules.m_flMatchClockAtLastUpdate")
        else {
            return Ok(None);
        };
        let Some((_, game_rules)) = ctx
            .entities()
            .iter()
            .find(|(_, entity)| entity.class_name.as_ref() == "CCitadelGameRulesProxy")
        else {
            return Ok(None);
        };
        if !game_rules.fields.contains_key(&key) {
            return Ok(None);
        }

        let seconds = game_rules.get_f32(Some(key));
        // Old builds can replicate the field with its default value. A match
        // that has ended must have a positive clock, so use the active-tick
        // fallback when the value is zero.
        // This fallback can include pregame recording time, so it might not
        // match the HUD clock exactly.
        if seconds.is_finite() && seconds > 0.0 {
            Ok(Some(seconds))
        } else {
            Ok(None)
        }
    }

    /// Collect the player roster from controllers at `tick`.
    /// Skip bots and empty slots that have no Steam ID.
    /// Return an empty frame when the tick has no controllers.
    pub(super) fn collect_players_at(&self, tick: i32) -> PyResult<DataFrame> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;

        let mut player_names: Vec<String> = Vec::new();
        let mut steam_ids: Vec<u64> = Vec::new();
        let mut hero_ids: Vec<i64> = Vec::new();
        let mut team_nums: Vec<i64> = Vec::new();
        let mut start_lanes: Vec<i64> = Vec::new();
        let mut ranks: Vec<i64> = Vec::new();

        // Resolve field keys once for CCitadelPlayerController
        let player_serializer = ctx.serializers().get("CCitadelPlayerController");
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
        let key_rank = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_PlayerDataGlobal.m_unPackedRank"));

        // Find all CCitadelPlayerController entities
        for (_idx, entity) in ctx.entities().iter() {
            if entity.class_name.as_ref() == "CCitadelPlayerController" {
                let player_name = key_player_name
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::String(bytes) => {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

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

                let hero_id = entity.get_i64(key_hero_id);
                let team_num = entity.get_i64(key_team_num);
                // Original lane assignment (CMsgLaneColor IDs: 1=yellow, 3=green,
                // 4=blue, 6=purple, 0=none).
                let start_lane = entity.get_i64(key_start_lane);
                // Packed display-rank value used by the server's post-match
                // `initial_display_rank`; 0 also covers calibration / no rank.
                let rank = entity.get_i64(key_rank);

                player_names.push(player_name);
                steam_ids.push(steam_id);
                hero_ids.push(hero_id);
                team_nums.push(team_num);
                start_lanes.push(start_lane);
                ranks.push(rank);
            }
        }

        df_from_columns(vec![
            Column::new("player_name".into(), player_names),
            Column::new("steam_id".into(), steam_ids),
            Column::new("hero_id".into(), hero_ids),
            Column::new("team_num".into(), team_nums),
            Column::new("start_lane".into(), start_lanes),
            Column::new("rank".into(), ranks),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))
    }

    /// Decode only player positions at selected ticks.
    pub(super) fn build_player_positions(&self, mut ticks: Vec<i32>) -> PyResult<DataFrame> {
        ticks.sort_unstable();
        ticks.dedup();

        let filter: HashSet<&str> = ["CCitadelPlayerPawn", "CCitadelPlayerController"]
            .into_iter()
            .collect();
        let init = self.parser.parse_init().map_err(to_py_err)?;
        let keys = PtKeys::resolve(&init);
        drop(init);

        let mut merged = PlayerPositionCols::default();
        if ticks.len() <= 4 {
            for tick in ticks {
                let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;
                if ctx.tick() == tick {
                    merged.collect_tick(&ctx, &keys);
                }
            }
            return merged.into_dataframe();
        }

        let selected: HashSet<i32> = ticks.into_iter().collect();
        let offsets = self.parser.full_packet_offsets().map_err(to_py_err)?;
        let count = parallel_segments().min(offsets.len().max(1));
        if count <= 1 {
            self.parser
                .decode_segment(None, i32::MAX, &filter, |ctx| {
                    if selected.contains(&ctx.tick()) {
                        merged.collect_tick(ctx, &keys);
                    }
                })
                .map_err(to_py_err)?;
            return merged.into_dataframe();
        }

        let ranges = segment_ranges(&offsets, count);
        let parser = &self.parser;
        let parts: std::result::Result<Vec<PlayerPositionCols>, String> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = ranges
                    .iter()
                    .map(|&(start, end_tick)| {
                        let filter = &filter;
                        let keys = &keys;
                        let selected = &selected;
                        scope.spawn(move || -> std::result::Result<PlayerPositionCols, String> {
                            let mut columns = PlayerPositionCols::default();
                            parser
                                .decode_segment(start, end_tick, filter, |ctx| {
                                    if selected.contains(&ctx.tick()) {
                                        columns.collect_tick(ctx, keys);
                                    }
                                })
                                .map_err(|error| error.to_string())?;
                            Ok(columns)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| handle.join().expect("position segment thread panicked"))
                    .collect()
            });
        for part in parts.map_err(InvalidDemoError::new_err)? {
            merged.append(part);
        }
        merged.into_dataframe()
    }

    /// Decode requested snapshot datasets in one parallel pass.
    ///
    /// Each full packet contains a new keyframe for the required entity state.
    /// Therefore, the segment results are identical to one serial pass.
    /// Return the requested `player_ticks`, `world_ticks`, and `troopers`
    /// frames. Use one serial decode when `BOON_TICK_SEGMENTS=1` or when the
    /// demo has one keyframe.
    pub(super) fn build_snapshots_parallel(
        &self,
        wants: SnapWants,
        pred: &TickPredicate,
    ) -> PyResult<(Option<DataFrame>, Option<DataFrame>, Option<DataFrame>)> {
        let mut classes: Vec<&str> = Vec::new();
        if wants.player_ticks {
            classes.push("CCitadelPlayerPawn");
            classes.push("CCitadelPlayerController");
        }
        if wants.world_ticks {
            classes.push("CCitadelGameRulesProxy");
        }
        if wants.troopers {
            classes.push("CNPC_Trooper");
            classes.push("CNPC_TrooperBoss");
        }
        let filter: std::collections::HashSet<&str> = classes.into_iter().collect();

        // Resolve all field keys once from the send-table serializers.
        let init = self.parser.parse_init().map_err(to_py_err)?;
        let keys = SnapKeys {
            pt: PtKeys::resolve(&init),
            wk: WkKeys::resolve(&init),
            tk: TkKeys::resolve(&init),
        };
        drop(init);

        let offsets = self.parser.full_packet_offsets().map_err(to_py_err)?;
        let n = parallel_segments().min(offsets.len().max(1));
        let merged = if n <= 1 {
            let mut cols = SegSnap::default();
            self.parser
                .decode_segment(None, i32::MAX, &filter, |ctx| {
                    cols.update(ctx, wants);
                    if pred.matches(ctx.tick()) {
                        cols.collect_tick(ctx, &keys, wants);
                    }
                })
                .map_err(to_py_err)?;
            cols
        } else {
            let segments = segment_ranges(&offsets, n);
            let parser = &self.parser;
            let filter = &filter;
            let keys = &keys;
            let parts: std::result::Result<Vec<SegSnap>, String> = std::thread::scope(|s| {
                let handles: Vec<_> = segments
                    .iter()
                    .map(|&(start, end_tick)| {
                        s.spawn(move || -> std::result::Result<SegSnap, String> {
                            let mut cols = SegSnap::default();
                            parser
                                .decode_segment(start, end_tick, filter, |ctx| {
                                    cols.update(ctx, wants);
                                    if pred.matches(ctx.tick()) {
                                        cols.collect_tick(ctx, keys, wants);
                                    }
                                })
                                .map_err(|error| error.to_string())?;
                            Ok(cols)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("snapshot segment thread panicked"))
                    .collect()
            });
            let mut merged = SegSnap::default();
            for part in parts.map_err(InvalidDemoError::new_err)? {
                merged.append(part);
            }
            merged
        };

        let SegSnap { pt, wt, tr, .. } = merged;
        Ok((
            if wants.player_ticks {
                Some(pt.into_dataframe()?)
            } else {
                None
            },
            if wants.world_ticks {
                Some(wt.into_dataframe()?)
            } else {
                None
            },
            if wants.troopers {
                Some(tr.into_dataframe()?)
            } else {
                None
            },
        ))
    }

    /// Get requested datasets at one tick with `parse_to_tick`.
    ///
    /// Return empty frames when the demo does not emit `tick`. Each full packet
    /// contains a new keyframe for these entities. Therefore, a direct seek
    /// produces the same state as a full decode at `tick`.
    pub(super) fn snapshot_at_tick(
        &self,
        tick: i32,
        wants: SnapWants,
    ) -> PyResult<(Option<DataFrame>, Option<DataFrame>, Option<DataFrame>)> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;
        let mut cols = SegSnap::default();
        if ctx.tick() == tick {
            let keys = SnapKeys {
                pt: PtKeys::resolve(&ctx),
                wk: WkKeys::resolve(&ctx),
                tk: TkKeys::resolve(&ctx),
            };
            if wants.player_ticks {
                cols.barriers.rebuild(&ctx);
            }
            cols.collect_tick(&ctx, &keys, wants);
        }
        let SegSnap { pt, wt, tr, .. } = cols;
        Ok((
            if wants.player_ticks {
                Some(pt.into_dataframe()?)
            } else {
                None
            },
            if wants.world_ticks {
                Some(wt.into_dataframe()?)
            } else {
                None
            },
            if wants.troopers {
                Some(tr.into_dataframe()?)
            } else {
                None
            },
        ))
    }

    /// Get requested datasets at a small set of ticks with direct seeks.
    ///
    /// Sort and deduplicate the ticks so row order matches a normal decode.
    /// The caller limits this path to small explicit requests. A full pass is
    /// faster when the request contains many ticks.
    pub(super) fn snapshots_at_ticks(
        &self,
        mut ticks: Vec<i32>,
        wants: SnapWants,
    ) -> PyResult<(Option<DataFrame>, Option<DataFrame>, Option<DataFrame>)> {
        ticks.sort_unstable();
        ticks.dedup();

        let mut merged = (None, None, None);
        for tick in ticks {
            let (pt, wt, tr) = self.snapshot_at_tick(tick, wants)?;
            append_snapshot_frame(&mut merged.0, pt)?;
            append_snapshot_frame(&mut merged.1, wt)?;
            append_snapshot_frame(&mut merged.2, tr)?;
        }
        Ok(merged)
    }

    /// Populate the caches for the requested snapshot datasets that aren't
    /// already loaded, using a single parallel decode pass over the demo.
    pub(super) fn ensure_snapshots(&mut self, mut wants: SnapWants) -> PyResult<()> {
        if self.cached_player_ticks.is_some() {
            wants.player_ticks = false;
        }
        if self.cached_world_ticks.is_some() {
            wants.world_ticks = false;
        }
        if self.cached_troopers.is_some() {
            wants.troopers = false;
        }
        if !wants.any() {
            return Ok(());
        }
        let (pt, wt, tr) = self.build_snapshots_parallel(wants, &TickPredicate::All)?;
        if let Some(df) = pt {
            self.cached_player_ticks = Some(df);
        }
        if let Some(df) = wt {
            self.cached_world_ticks = Some(df);
        }
        if let Some(df) = tr {
            self.cached_troopers = Some(df);
        }
        Ok(())
    }

    /// The cached DataFrame for a loaded dataset, by name (for `snapshots(events=)`).
    pub(super) fn cached_frame(&self, name: &str) -> Option<&DataFrame> {
        match name {
            "abilities" => self.cached_abilities.as_ref(),
            "ability_upgrades" => self.cached_ability_upgrades.as_ref(),
            "ability_ticks" => self.cached_ability_ticks.as_ref(),
            "chat" => self.cached_chat.as_ref(),
            "mid_boss" => self.cached_mid_boss.as_ref(),
            "objectives" => self.cached_objectives.as_ref(),
            "player_ticks" => self.cached_player_ticks.as_ref(),
            "world_ticks" => self.cached_world_ticks.as_ref(),
            "kills" => self.cached_kills.as_ref(),
            "damage" => self.cached_damage.as_ref(),
            "healing" => self.cached_healing.as_ref(),
            "flex_slots" => self.cached_flex_slots.as_ref(),
            "item_purchases" => self.cached_item_purchases.as_ref(),
            "troopers" => self.cached_troopers.as_ref(),
            "neutrals" => self.cached_neutrals.as_ref(),
            "breakables" => self.cached_breakables.as_ref(),
            "sinners_sacrifice" => self.cached_sinners_sacrifice.as_ref(),
            "stat_modifier_events" => self.cached_stat_modifier_events.as_ref(),
            "active_modifiers" => self.cached_active_modifiers.as_ref(),
            "urn" => self.cached_urn.as_ref(),
            "street_brawl_ticks" => self.cached_street_brawl_ticks.as_ref(),
            "street_brawl_rounds" => self.cached_street_brawl_rounds.as_ref(),
            "rift" => self.cached_rift.as_ref(),
            _ => None,
        }
    }

    /// Union of the `tick` columns of the given event datasets (loading each if
    /// needed), for `snapshots(events=)`.
    pub(super) fn event_ticks(
        &mut self,
        names: &[String],
    ) -> PyResult<std::collections::HashSet<i32>> {
        let missing: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| self.cached_frame(name).is_none())
            .collect();
        if missing
            .iter()
            .all(|name| direct_event_message_type(name).is_some())
        {
            let mut set = std::collections::HashSet::new();
            for name in names {
                let Some(df) = self.cached_frame(name) else {
                    continue;
                };
                let tick = df
                    .column("tick")
                    .and_then(|column| column.i32())
                    .map_err(|_| {
                        pyo3::exceptions::PyValueError::new_err(format!(
                            "snapshots(events=): '{name}' has no i32 tick column"
                        ))
                    })?;
                set.extend(tick.into_iter().flatten());
            }
            if missing.is_empty() {
                return Ok(set);
            }

            // These datasets have one row for each valid message. Read only the
            // selected messages. Do not decode entity state or build a frame.
            let event_types: HashSet<u32> = missing
                .iter()
                .filter_map(|name| direct_event_message_type(name))
                .collect();
            let events = self
                .parser
                .events_filtered(None, &event_types)
                .map_err(to_py_err)?;
            for event in events {
                let has_row = match event.msg_type {
                    value if value == Msg::KEUserMsgHeroKilled as u32 => {
                        boon_proto::proto::CCitadelUserMsgHeroKilled::decode(
                            event.payload.as_slice(),
                        )
                        .map(|_| true)
                        .map_err(|error| {
                            DemoMessageError::new_err(format!(
                                "Failed to decode HeroKilled event: {error}"
                            ))
                        })?
                    }
                    value if value == Msg::KEUserMsgDamage as u32 => {
                        boon_proto::proto::CCitadelUserMessageDamage::decode(
                            event.payload.as_slice(),
                        )
                        .map(|_| true)
                        .map_err(|error| {
                            DemoMessageError::new_err(format!(
                                "Failed to decode Damage event: {error}"
                            ))
                        })?
                    }
                    value if value == Msg::KEUserMsgFlexSlotUnlocked as u32 => {
                        boon_proto::proto::CCitadelUserMsgFlexSlotUnlocked::decode(
                            event.payload.as_slice(),
                        )
                        .is_ok()
                    }
                    value if value == Msg::KEUserMsgImportantAbilityUsed as u32 => {
                        boon_proto::proto::CCitadelUserMessageImportantAbilityUsed::decode(
                            event.payload.as_slice(),
                        )
                        .is_ok()
                    }
                    value if value == Msg::KEUserMsgAbilitiesChanged as u32 => {
                        boon_proto::proto::CCitadelUserMsgAbilitiesChanged::decode(
                            event.payload.as_slice(),
                        )
                        .is_ok()
                    }
                    value if value == Msg::KEUserMsgChatMsg as u32 => {
                        boon_proto::proto::CCitadelUserMsgChatMsg::decode(event.payload.as_slice())
                            .is_ok()
                    }
                    _ => false,
                };
                if has_row {
                    set.insert(event.tick);
                }
            }
            return Ok(set);
        }

        // Load all missing event datasets in one parser pass. Loading each name
        // inside the loop makes compatible datasets scan the full demo once per
        // name, although `load` can collect them together.
        Python::attach(|py| self.load(py, names.to_vec()))?;

        let mut set = std::collections::HashSet::new();
        for name in names {
            let df = self.cached_frame(name).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "snapshots(events=): '{name}' is not an event dataset with a tick column"
                ))
            })?;
            let tick = df.column("tick").and_then(|c| c.i32()).map_err(|_| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "snapshots(events=): '{name}' has no i32 `tick` column"
                ))
            })?;
            for t in tick.into_iter().flatten() {
                set.insert(t);
            }
        }
        Ok(set)
    }
}

fn append_snapshot_frame(output: &mut Option<DataFrame>, next: Option<DataFrame>) -> PyResult<()> {
    let Some(next) = next else {
        return Ok(());
    };
    if let Some(output) = output {
        output.vstack_mut(&next).map_err(|error| {
            InvalidDemoError::new_err(format!("Failed to combine snapshot frames: {error}"))
        })?;
    } else {
        *output = Some(next);
    }
    Ok(())
}

fn direct_event_message_type(name: &str) -> Option<u32> {
    match name {
        "kills" => Some(Msg::KEUserMsgHeroKilled as u32),
        "damage" => Some(Msg::KEUserMsgDamage as u32),
        "flex_slots" => Some(Msg::KEUserMsgFlexSlotUnlocked as u32),
        "abilities" => Some(Msg::KEUserMsgImportantAbilityUsed as u32),
        "item_purchases" => Some(Msg::KEUserMsgAbilitiesChanged as u32),
        "chat" => Some(Msg::KEUserMsgChatMsg as u32),
        _ => None,
    }
}
