use crate::*;

#[pymethods]
impl Demo {
    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns a DataFrame with 51 columns covering position, health, barrier, resistance, combat
    /// timers, kills, deaths, net worth, and more for every player at every tick.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn player_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.ensure_snapshots_detached(
            py,
            SnapWants {
                player_ticks: true,
                ..Default::default()
            },
        )?;
        Ok(PyDataFrame(self.cached_player_ticks.clone().unwrap()))
    }

    /// World state at every tick as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``is_paused``, ``next_midboss``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn world_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.ensure_snapshots_detached(
            py,
            SnapWants {
                world_ticks: true,
                ..Default::default()
            },
        )?;
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
    pub(crate) fn kills(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_kills.is_none() {
            self.load(py, vec!["kills".to_string()])?;
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
    /// - hitgroup_id: The hitgroup that was hit (use ``hitgroup_names()`` to resolve)
    /// - crit_damage: Critical damage amount
    /// - attacker_class: The attacker's entity class ID
    /// - victim_class: The victim's entity class ID
    /// - ability_id: The ability/weapon that dealt the hit (0 if absent; use
    ///   ``ability_names()`` to resolve it)
    /// - damage_type: Raw Source ``type`` damage bitfield
    /// - citadel_type: Deadlock damage category (3 is melee-typed damage)
    /// - damage_flags: Raw Valve damage flags used for detailed classification
    /// - is_melee: True for any melee-typed damage (``citadel_type == 3``)
    /// - melee_type: ``"light"`` or ``"heavy"`` for basic melee, ``"other"``
    ///   for another melee-typed source, otherwise null
    ///
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn damage(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_damage.is_none() {
            self.load(py, vec!["damage".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_damage.clone().unwrap()))
    }

    /// Flex slot unlock events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn flex_slots(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_flex_slots.is_none() {
            self.load(py, vec!["flex_slots".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_flex_slots.clone().unwrap()))
    }

    /// Ability usage events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn abilities(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_abilities.is_none() {
            self.load(py, vec!["abilities".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_abilities.clone().unwrap()))
    }

    /// Hero ability upgrade events (skill point spending) as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``tier``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn ability_upgrades(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_ability_upgrades.is_none() {
            self.load(py, vec!["ability_upgrades".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_ability_upgrades.clone().unwrap()))
    }

    /// Item purchase/sell/upgrade events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``change``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn item_purchases(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_item_purchases.is_none() {
            self.load(py, vec!["item_purchases".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_item_purchases.clone().unwrap()))
    }

    /// Chat messages as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``text``, ``chat_type``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn chat(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_chat.is_none() {
            self.load(py, vec!["chat".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_chat.clone().unwrap()))
    }

    /// Objective health state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``objective_type``, ``team_num``, ``lane``, ``health``, ``max_health``, ``phase``, ``x``, ``y``, ``z``, ``entity_id``.
    /// Emits a row when an objective's health or max_health changes.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn objectives(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_objectives.is_none() {
            self.load(py, vec!["objectives".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_objectives.clone().unwrap()))
    }

    /// Mid boss lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``, ``event``.
    /// Events: ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn mid_boss(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_mid_boss.is_none() {
            self.load(py, vec!["mid_boss".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_mid_boss.clone().unwrap()))
    }

    /// Rift lifecycle as a Polars DataFrame — one row per Rift.
    ///
    /// The Rift is a periodic king-of-the-hill objective (``Koth`` in the game
    /// files); the team that wins one gets buffed troopers in that lane.
    ///
    /// Columns: ``rift_num``, ``announce_tick``, ``active_tick``,
    /// ``capture_tick``, ``expire_tick``, ``winning_team``, ``lane``, ``x``,
    /// ``y``, ``z``. Exactly one of ``capture_tick`` / ``expire_tick`` is set
    /// per row; ``winning_team`` is null when the Rift expired uncaptured.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn rift(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_rift.is_none() {
            self.load(py, vec!["rift".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_rift.clone().unwrap()))
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
    pub(crate) fn troopers(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.ensure_snapshots_detached(
            py,
            SnapWants {
                troopers: true,
                ..Default::default()
            },
        )?;
        Ok(PyDataFrame(self.cached_troopers.clone().unwrap()))
    }

    /// Neutral creep state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``,
    /// ``health``, ``max_health``, ``x``, ``y``, ``z``.
    ///
    /// Tracks ``CNPC_TrooperNeutral``.
    /// Only emits a row when an alive neutral's state changes (health,
    /// position), significantly reducing data volume.
    ///
    /// **Note:** Not loaded by default. Access this property or call
    /// ``load("neutrals")`` explicitly.
    #[getter]
    pub(crate) fn neutrals(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_neutrals.is_none() {
            self.load(py, vec!["neutrals".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_neutrals.clone().unwrap()))
    }

    /// Breakable map-prop destruction events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``event``, ``entity_id``, ``entity_serial``,
    /// ``subclass_id``, ``subclass_name``, ``team_num``, ``x``, ``y``, ``z``.
    ///
    /// Deadlock represents a broken ``CCitadel_BreakableProp`` as an entity
    /// leaving the PVS without a health-zero update or permanent delete. Boon
    /// keeps that leave as a candidate through the end of the demo and emits it
    /// only if the same entity identity never reactivates. Full-packet
    /// delete/create replacements are ignored.
    ///
    /// Each row has ``event="broken"`` and the prop's last-known position.
    /// Health and lifestate are intentionally omitted because the server never
    /// reports a final health-zero or dead state.
    ///
    /// **Note:** Not loaded by default. Access this property or call
    /// ``load("breakables")`` explicitly.
    #[getter]
    pub(crate) fn breakables(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_breakables.is_none() {
            self.load(py, vec!["breakables".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_breakables.clone().unwrap()))
    }

    /// Sinner's Sacrifice machine lifecycle and hit events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``event``, ``entity_id``, ``entity_serial``,
    /// ``attacker_hero_id``, ``damage``, ``health``, ``max_health``,
    /// ``team_num``, ``x``, ``y``, ``z``.
    ///
    /// ``event`` is ``"spawned"``, ``"hit"``, or ``"reset"``. Hit rows use
    /// the Damage message's exact victim entity and attacker when available;
    /// an entity health decrease without a matching message is retained with
    /// ``attacker_hero_id=0``. ``health`` is the machine's state at the end of
    /// the tick, which may be shared by multiple hits in the same tick.
    ///
    /// Tracks both ``CNPC_Neutral_SinnersSacrifice`` and the Hideout variant.
    /// Health zero and lifestate are intentionally omitted because inactive
    /// machines can omit health fields and completed machines remain alive at
    /// one health.
    ///
    /// **Note:** Not loaded by default. Access this property or call
    /// ``load("sinners_sacrifice")`` explicitly.
    #[getter]
    pub(crate) fn sinners_sacrifice(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_sinners_sacrifice.is_none() {
            self.load(py, vec!["sinners_sacrifice".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_sinners_sacrifice.clone().unwrap()))
    }

    /// Permanent stat bonus change events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``stat_type``, ``amount``.
    ///
    /// ``stat_type`` is one of: ``"health"``, ``"spirit_power"``, ``"fire_rate"``,
    /// ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``,
    /// ``"bullet_resist"``, or ``"spirit_resist"``.
    /// ``amount`` is the signed change from this event.
    ///
    /// Emits a row whenever a stat total changes (idol/breakable pickups).
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn stat_modifier_events(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_stat_modifier_events.is_none() {
            self.load(py, vec!["stat_modifier_events".to_string()])?;
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
    /// ``"changed"`` when its ``stacks`` count changes while active, and
    /// ``"removed"`` when it disappears. The ``removed`` row reports the final
    /// stack count.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    pub(crate) fn active_modifiers(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_active_modifiers.is_none() {
            self.load(py, vec!["active_modifiers".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_active_modifiers.clone().unwrap()))
    }

    /// Ability cooldown / charge state changes as a Polars DataFrame.
    ///
    /// Change-only: a row is emitted for an ability only on the tick its
    /// cooldown or charge state changes (not every tick), keeping the frame
    /// compact. One ability entity exists per ability the player owns, including
    /// innate movement abilities (jump, dash, slide, ...) which can be filtered
    /// out via ``slot``.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id`` (a ``CUtlStringToken``;
    /// resolve with ``ability_names()``), ``slot`` (``EAbilitySlots_t``),
    /// ``cooldown_start`` / ``cooldown_end`` (game time; available again at
    /// ``cooldown_end``), ``remaining_charges``, and ``charge_recharge_start`` /
    /// ``charge_recharge_end`` (recharge window of the charge currently
    /// regenerating).
    ///
    /// Not loaded by default. Access this property or call
    /// ``load("ability_ticks")`` explicitly.
    #[getter]
    pub(crate) fn ability_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_ability_ticks.is_none() {
            self.load(py, vec!["ability_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_ability_ticks.clone().unwrap()))
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
    pub(crate) fn urn(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.cached_urn.is_none() {
            self.load(py, vec!["urn".to_string()])?;
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
    pub(crate) fn street_brawl_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_ticks.is_none() {
            self.load(py, vec!["street_brawl_ticks".to_string()])?;
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
    pub(crate) fn street_brawl_rounds(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_rounds.is_none() {
            self.load(py, vec!["street_brawl_rounds".to_string()])?;
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
    pub(crate) fn winning_team_num(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(team, _)| team))
    }

    /// The tick when the game ended.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    pub(crate) fn game_over_tick(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(_, tick)| tick))
    }

    /// The number of ticks of regulation play, excluding paused time.
    ///
    /// Counts active (non-paused) ticks from the start of the recording up to
    /// the ``k_EUserMsg_GameOver`` event, reflecting how much of the game was
    /// actually played rather than the full recording length (``total_ticks``,
    /// which also includes pre-game and post-match time). Scans for the
    /// game-over event and loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found (e.g. an incomplete
    /// recording).
    #[getter]
    pub(crate) fn regulation_ticks(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        let Some((_, tick)) = self.game_over else {
            return Ok(None);
        };
        self.ensure_paused_ticks_built()?;
        Ok(Some(self.count_active_ticks(tick)))
    }

    /// The duration of regulation play in seconds, excluding paused time.
    ///
    /// Unlike ``total_seconds`` (the full recording length), this measures the
    /// active gameplay duration up to the ``k_EUserMsg_GameOver`` event. Equal
    /// to ``regulation_ticks / tick_rate``. Scans for the game-over event and
    /// loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found.
    #[getter]
    pub(crate) fn regulation_seconds(&mut self) -> PyResult<Option<f32>> {
        if self.tick_rate == 0 {
            return Ok(None);
        }
        let Some(ticks) = self.regulation_ticks()? else {
            return Ok(None);
        };
        Ok(Some(ticks as f32 / self.tick_rate as f32))
    }

    /// The duration of regulation play as a formatted string (e.g. ``"32:45"``),
    /// excluding paused time.
    ///
    /// The regulation counterpart to ``total_clock_time``. Scans for the
    /// game-over event and loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found.
    #[getter]
    pub(crate) fn regulation_clock_time(&mut self) -> PyResult<Option<String>> {
        let Some(secs) = self.regulation_seconds()? else {
            return Ok(None);
        };
        let total_seconds = secs as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(Some(format!("{minutes}:{seconds:02}")))
    }

    pub(crate) fn __repr__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }

    pub(crate) fn __str__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }
}
