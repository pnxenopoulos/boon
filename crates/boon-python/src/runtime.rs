use crate::*;

impl Demo {
    pub(super) fn build_stat_ticks_parallel(
        &self,
        selected: boon_parser::StatMask,
        predicate: &TickPredicate,
    ) -> PyResult<DataFrame> {
        let filter: HashSet<&str> = ["CCitadelPlayerPawn", "CCitadelPlayerController"]
            .into_iter()
            .collect();
        let init = self.parser.parse_init().map_err(to_py_err)?;
        let keys = PtKeys::resolve(&init);
        drop(init);

        let offsets = self.parser.full_packet_offsets().map_err(to_py_err)?;
        let segment_count = parallel_segments().min(offsets.len().max(1));
        let merged = if segment_count <= 1 {
            let mut segment = StatSegment::default();
            self.parser
                .decode_segment(None, i32::MAX, &filter, |ctx| {
                    segment.update(ctx);
                    if predicate.matches(ctx.tick()) {
                        segment
                            .columns
                            .collect_tick(ctx, &keys, &segment.modifiers, selected);
                    }
                })
                .map_err(to_py_err)?;
            segment
        } else {
            let ranges = segment_ranges(&offsets, segment_count);
            let parser = &self.parser;
            let parts: std::result::Result<Vec<StatSegment>, String> =
                std::thread::scope(|scope| {
                    let handles: Vec<_> = ranges
                        .iter()
                        .map(|&(start, end_tick)| {
                            let filter = &filter;
                            let keys = &keys;
                            scope.spawn(move || -> std::result::Result<StatSegment, String> {
                                let mut segment = StatSegment::default();
                                parser
                                    .decode_segment(start, end_tick, filter, |ctx| {
                                        segment.update(ctx);
                                        if predicate.matches(ctx.tick()) {
                                            segment.columns.collect_tick(
                                                ctx,
                                                keys,
                                                &segment.modifiers,
                                                selected,
                                            );
                                        }
                                    })
                                    .map_err(|error| error.to_string())?;
                                Ok(segment)
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|handle| handle.join().expect("stat segment thread panicked"))
                        .collect()
                });
            let mut merged = StatSegment::default();
            for part in parts.map_err(InvalidDemoError::new_err)? {
                merged.columns.append(part.columns, selected);
            }
            merged
        };
        merged.columns.into_dataframe(selected)
    }

    pub(super) fn stat_ticks_at(
        &self,
        tick: i32,
        selected: boon_parser::StatMask,
    ) -> PyResult<DataFrame> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;
        let mut segment = StatSegment::default();
        if ctx.tick() == tick {
            let keys = PtKeys::resolve(&ctx);
            segment.modifiers.rebuild(&ctx);
            segment.initialized = true;
            segment
                .columns
                .collect_tick(&ctx, &keys, &segment.modifiers, selected);
        }
        segment.columns.into_dataframe(selected)
    }

    pub(super) fn build_stat_effects(
        &self,
        selected: boon_parser::StatMask,
    ) -> PyResult<DataFrame> {
        let filter: HashSet<&str> = ["CCitadelPlayerPawn", "CCitadelPlayerController"]
            .into_iter()
            .collect();
        let init = self.parser.parse_init().map_err(to_py_err)?;
        let keys = PtKeys::resolve(&init);
        drop(init);

        let mut rows = StatEffectCols::default();
        let mut modifiers = boon_parser::ModifierState::default();
        let mut signatures: HashMap<u32, ModifierEffectSignature> = HashMap::new();
        let mut serial_owners: HashMap<u32, i64> = HashMap::new();
        let mut previous_upgrades: HashMap<i64, HashSet<u32>> = HashMap::new();
        let mut last_inputs: HashMap<i64, PlayerStatInputs> = HashMap::new();
        let mut known_entity_to_hero: HashMap<i32, i64> = HashMap::new();

        self.parser
            .decode_segment(None, i32::MAX, &filter, |ctx| {
                let mut by_pawn: HashMap<i32, PlayerStatInputs> = HashMap::new();
                let mut current_entity_to_hero: HashMap<i32, i64> = HashMap::new();

                for (controller_index, controller) in ctx
                    .entities()
                    .iter()
                    .filter(|(_, entity)| entity.class_name.as_ref() == "CCitadelPlayerController")
                {
                    let Some(pawn_handle) = controller.get_handle(keys.pawn_handle) else {
                        continue;
                    };
                    let Some(pawn) = ctx.entities().get_by_handle(pawn_handle) else {
                        continue;
                    };
                    if pawn.class_name.as_ref() != "CCitadelPlayerPawn" {
                        continue;
                    }
                    let hero_id = pawn.get_i64(keys.hero_id);
                    let Some(pawn_index) = boon_parser::protobuf_handle_index(Some(pawn_handle))
                    else {
                        continue;
                    };
                    if hero_id == 0 {
                        continue;
                    }
                    let (upgrades, ability_tiers) = stat_inputs(controller, &keys);
                    let inputs = PlayerStatInputs {
                        hero_id,
                        level: controller.get_i64(keys.level),
                        upgrades,
                        ability_tiers,
                    };
                    by_pawn.insert(pawn_index, inputs.clone());
                    current_entity_to_hero.insert(controller_index, hero_id);
                    current_entity_to_hero.insert(pawn_index, hero_id);
                }
                known_entity_to_hero.extend(
                    current_entity_to_hero
                        .iter()
                        .map(|(&entity, &hero)| (entity, hero)),
                );

                // Purchased item changes explain baseline contributions. Compare
                // sets rather than vector positions because upgrades may reorder.
                let mut players: Vec<_> = by_pawn.values().cloned().collect();
                players.sort_by_key(|inputs| inputs.hero_id);
                for inputs in players {
                    let current: HashSet<u32> = inputs.upgrades.iter().copied().collect();
                    let previous = previous_upgrades
                        .get(&inputs.hero_id)
                        .cloned()
                        .unwrap_or_default();
                    let layers = boon_parser::evaluate_player_stats(
                        inputs.hero_id,
                        inputs.level,
                        &inputs.upgrades,
                        &inputs.ability_tiers,
                        std::iter::empty(),
                    );
                    let spirit_power = layers.baseline[boon_parser::StatId::SpiritPower];

                    let mut added: Vec<_> = current.difference(&previous).copied().collect();
                    let mut removed: Vec<_> = previous.difference(&current).copied().collect();
                    added.sort_unstable();
                    removed.sort_unstable();
                    for (event, active, upgrades) in
                        [("applied", true, added), ("removed", false, removed)]
                    {
                        for ability_id in upgrades {
                            for &effect in boon_parser::stat_catalog::item_stat_effects(ability_id)
                            {
                                if !selected.contains(effect.stat) {
                                    continue;
                                }
                                rows.push(ModifierEffectRow {
                                    tick: ctx.tick(),
                                    hero_id: inputs.hero_id,
                                    event,
                                    effect,
                                    source_type: "item",
                                    layer: "baseline",
                                    ability_id,
                                    modifier_id: 0,
                                    serial: 0,
                                    caster_hero_id: inputs.hero_id,
                                    provider_hero_id: 0,
                                    stacks: 1,
                                    duration: -1.0,
                                    active,
                                    spirit_power,
                                    ability_tier: 0,
                                });
                            }
                        }
                    }
                    previous_upgrades.insert(inputs.hero_id, current);
                    last_inputs.insert(inputs.hero_id, inputs);
                }

                for change in modifiers.update(ctx) {
                    let entry = &change.entry;
                    let ability_id = entry.ability_subclass.unwrap_or(0);
                    let modifier_id = entry.modifier_subclass.unwrap_or(0);
                    let stacks = entry.stack_count.unwrap_or(0);
                    let signature = ModifierEffectSignature {
                        ability_id,
                        modifier_id,
                        stacks,
                        in_aura_range: entry.in_aura_range,
                    };

                    if change.kind == boon_parser::ModifierChangeKind::Changed
                        && signatures.get(&change.serial) == Some(&signature)
                    {
                        continue;
                    }

                    let parent_index = boon_parser::protobuf_handle_index(entry.parent);
                    let hero_id = parent_index
                        .and_then(|index| by_pawn.get(&index))
                        .map(|inputs| inputs.hero_id)
                        .or_else(|| serial_owners.get(&change.serial).copied())
                        .unwrap_or(0);
                    if hero_id == 0 {
                        continue;
                    }
                    if change.kind != boon_parser::ModifierChangeKind::Removed {
                        serial_owners.insert(change.serial, hero_id);
                        signatures.insert(change.serial, signature);
                    }

                    let Some(inputs) = by_pawn
                        .values()
                        .find(|inputs| inputs.hero_id == hero_id)
                        .or_else(|| last_inputs.get(&hero_id))
                    else {
                        continue;
                    };

                    // An item's permanent auto-registered modifier duplicates
                    // the baseline item row emitted above.
                    let duration = entry.duration.unwrap_or(-1.0);
                    if duration < 0.0 && inputs.upgrades.contains(&ability_id) {
                        if change.kind == boon_parser::ModifierChangeKind::Removed {
                            signatures.remove(&change.serial);
                            serial_owners.remove(&change.serial);
                        }
                        continue;
                    }

                    let layers = boon_parser::evaluate_player_stats(
                        inputs.hero_id,
                        inputs.level,
                        &inputs.upgrades,
                        &inputs.ability_tiers,
                        std::iter::empty(),
                    );
                    let spirit_power = layers.baseline[boon_parser::StatId::SpiritPower];
                    let ability_tier = inputs.ability_tiers.get(&ability_id).copied().unwrap_or(0);
                    let caster_hero_id = boon_parser::protobuf_handle_index(entry.caster)
                        .and_then(|index| {
                            current_entity_to_hero
                                .get(&index)
                                .or_else(|| known_entity_to_hero.get(&index))
                        })
                        .copied()
                        .unwrap_or(0);
                    let provider_hero_id =
                        boon_parser::protobuf_handle_index(entry.aura_provider_ehandle)
                            .and_then(|index| {
                                current_entity_to_hero
                                    .get(&index)
                                    .or_else(|| known_entity_to_hero.get(&index))
                            })
                            .copied()
                            .unwrap_or(0);
                    let active = change.kind != boon_parser::ModifierChangeKind::Removed
                        && entry.in_aura_range != Some(false);
                    let layer = if duration < 0.0 {
                        "baseline"
                    } else {
                        "effective"
                    };

                    for &effect in
                        boon_parser::stat_catalog::modifier_stat_effects(ability_id, modifier_id)
                    {
                        if !selected.contains(effect.stat) {
                            continue;
                        }
                        rows.push(ModifierEffectRow {
                            tick: ctx.tick(),
                            hero_id,
                            event: change.kind.as_str(),
                            effect,
                            source_type: "modifier",
                            layer,
                            ability_id,
                            modifier_id,
                            serial: change.serial,
                            caster_hero_id,
                            provider_hero_id,
                            stacks,
                            duration,
                            active,
                            spirit_power,
                            ability_tier,
                        });
                    }

                    if change.kind == boon_parser::ModifierChangeKind::Removed {
                        signatures.remove(&change.serial);
                        serial_owners.remove(&change.serial);
                    }
                }
            })
            .map_err(to_py_err)?;

        rows.into_dataframe()
    }

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
                                .map_err(|e| e.to_string())?;
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
        let mut set = std::collections::HashSet::new();
        for name in names {
            Python::attach(|py| self.load(py, vec![name.clone()]))?;
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
