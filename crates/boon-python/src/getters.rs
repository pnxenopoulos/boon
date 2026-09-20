use crate::*;

#[pymethods]
impl Demo {
    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns a DataFrame with 60 columns covering position, health, barrier, observed stat
    /// modifiers, combat timers, kills, deaths, net worth, and more for every player at every tick.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn player_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::PlayerTicks)
    }

    /// World state at every tick as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``is_paused``, ``next_midboss``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn world_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::WorldTicks)
    }

    /// Hero kill events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the kill occurred
    /// - victim_hero_id: The hero ID of the killed player
    /// - attacker_hero_id: The hero ID of the attacker
    /// - assister_hero_ids: List of hero IDs of players who assisted
    ///
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn kills(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Kills)
    }

    /// Damage events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the damage occurred
    /// - damage: The damage dealt
    /// - pre_damage: The damage before mitigation
    /// - damage_absorbed: Recorded barrier absorption, null when absent. Uses the
    ///   legacy integer field when the float field is absent.
    /// - victim_shield_new: Remaining shield after this hit, null when absent
    /// - victim_shield_max: Shield capacity, null when absent
    /// - is_secondary_stat: Recorded secondary-stat flag, null when absent
    /// - server_tick: Server tick, distinct from demo tick; null when absent
    /// - victim_hero_id: The hero ID of the victim (0 if not a hero)
    /// - attacker_hero_id: The hero ID of the attacker (0 if not a hero)
    /// - victim_health_new: The victim's health after damage
    /// - hitgroup_id: The hitgroup that was hit (use ``hitgroup_names()`` to resolve)
    /// - crit_damage: Critical damage amount
    /// - attacker_class: The attacker's entity class ID
    /// - victim_class: The victim's entity class ID
    /// - victim_entity_id: The victim's entity index (-1 if absent). Join it to an entity-keyed
    ///   dataset such as ``neutrals`` or ``sinners_sacrifice`` to identify the exact unit killed,
    ///   which ``victim_class`` (a coarse enum) cannot distinguish.
    /// - ability_id: The ability/weapon that dealt the hit (0 if absent; use
    ///   ``ability_names()`` to resolve it)
    /// - damage_type: Raw Source ``type`` damage bitfield
    /// - citadel_type: Deadlock damage category (3 is melee-typed damage)
    /// - damage_flags: Raw Valve damage flags used for detailed classification
    /// - is_melee: True for any melee-typed damage (``citadel_type == 3``)
    /// - melee_type: ``"light"`` or ``"heavy"`` for basic melee, ``"other"``
    ///   for another melee-typed source, otherwise null
    ///
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn damage(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Damage)
    }

    /// Recorded healing and regeneration per sample interval.
    ///
    /// Shares the cached frame returned by ``summary()["healing"]``.
    /// Columns: ``interval_start_s``, ``interval_end_s``, ``healer_player_slot``,
    /// ``healer_hero_id``, ``target_player_slot``, ``target_hero_id``, ``source_name``,
    /// ``stat_type`` (``healing`` or ``regen``), and ``amount``.
    /// Bounds use match-clock seconds, usually 180 seconds apart. These are
    /// interval totals, not individual heals. Hero IDs come from the match roster.
    /// Category duplicates are excluded. Raises ``DemoMessageError`` when the
    /// recording has no post-match details. Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn healing(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Healing)
    }

    /// Flex slot unlock events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn flex_slots(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::FlexSlots)
    }

    /// Ability usage events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn abilities(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Abilities)
    }

    /// Hero ability upgrade events (skill point spending) as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``tier``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn ability_upgrades(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::AbilityUpgrades)
    }

    /// Item purchase/sell/upgrade events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``change``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn item_purchases(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ItemPurchases)
    }

    /// Chat messages as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``text``, ``chat_type``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn chat(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Chat)
    }

    /// Objective health state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``objective_type``, ``team_num``, ``lane``, ``health``, ``max_health``, ``phase``, ``x``, ``y``, ``z``, ``entity_id``.
    /// Emits a row when an objective's health or max_health changes.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn objectives(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Objectives)
    }

    /// Mid boss lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``, ``event``.
    /// Events: ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn mid_boss(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::MidBoss)
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
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn rift(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Rift)
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
        self.dataset_frame(py, Dataset::Troopers)
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
        self.dataset_frame(py, Dataset::Neutrals)
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
        self.dataset_frame(py, Dataset::Breakables)
    }

    /// Sinner's Sacrifice machine lifecycle and hit events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``event``, ``entity_id``, ``entity_serial``,
    /// ``attacker_hero_id``, ``damage``, ``health``, ``max_health``,
    /// ``team_num``, ``x``, ``y``, ``z``.
    ///
    /// ``event`` is ``"spawned"``, ``"hit"``, or ``"reset"``. A hit row uses
    /// the victim and attacker from the Damage message. Boon keeps a health
    /// decrease that has no matching message. For this event,
    /// ``attacker_hero_id`` is ``0``. ``health`` is the machine state at the
    /// end of the tick. Multiple hits can have the same health value.
    ///
    /// Track ``CNPC_Neutral_SinnersSacrifice`` and its Hideout variant.
    /// An inactive machine can omit health fields. A completed machine stays
    /// alive at one health. Therefore, the dataset does not add health-zero or
    /// lifestate fields.
    ///
    /// Boon does not load this dataset by default. Access the property or call
    /// ``load("sinners_sacrifice")``.
    #[getter]
    pub(crate) fn sinners_sacrifice(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::SinnersSacrifice)
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
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn stat_modifier_events(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StatModifierEvents)
    }

    /// Active buff/debuff modifier events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``event``, ``modifier_id``, ``ability_id``,
    /// ``duration``, ``caster_hero_id``, ``stacks``.
    ///
    /// ``"applied"`` means that Boon first saw the modifier on a player.
    /// ``"changed"`` means that its effective state changed. ``"removed"``
    /// means that its effective lifetime ended. A removal can come from the
    /// replicated table, slot reuse, aura exit, or a finite duration.
    ///
    /// Finite durations use the replicated Source 2 simulation clock. Old
    /// demos without that clock keep explicit removal behavior. Boon does not
    /// clear every modifier on death because some modifiers survive death.
    /// The removed row contains the final stack count.
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn active_modifiers(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ActiveModifiers)
    }

    /// Ability cooldown / charge state changes as a Polars DataFrame.
    ///
    /// The dataset is change-only. Boon emits a row only when the cooldown or
    /// charge state changes. One entity exists for each ability that a player
    /// owns. These entities include movement abilities such as jump, dash, and
    /// slide. Use ``slot`` to remove these abilities from the result.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id`` (a ``CUtlStringToken``;
    /// resolve with ``ability_names()``), ``slot`` (``EAbilitySlots_t``),
    /// ``cooldown_start`` / ``cooldown_end`` (game time; available again at
    /// ``cooldown_end``), ``remaining_charges``, and ``charge_recharge_start`` /
    /// ``charge_recharge_end`` (recharge window of the charge currently
    /// regenerating).
    ///
    /// Boon does not load this dataset by default. Access the property or call
    /// ``load("ability_ticks")``.
    #[getter]
    pub(crate) fn ability_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::AbilityTicks)
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
    /// Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn urn(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Urn)
    }

    /// Per-tick street brawl state as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``round``, ``state``, ``amber_score``,
    /// ``sapphire_score``, ``buy_countdown``, ``next_state_time``,
    /// ``state_start_time``, ``non_combat_time``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Boon loads this dataset on first access.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    pub(crate) fn street_brawl_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StreetBrawlTicks)
    }

    /// Street brawl round scoring events as a Polars DataFrame.
    ///
    /// Columns: ``round``, ``tick``, ``scoring_team``, ``amber_score``,
    /// ``sapphire_score``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Boon loads this dataset on first access.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    pub(crate) fn street_brawl_rounds(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StreetBrawlRounds)
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

    /// The number of match-clock ticks at game over.
    ///
    /// Uses the replicated HUD match clock. This excludes pregame, pauses, and
    /// post-game time. Old demos that omit the clock or set it to zero use
    /// active demo ticks as a fallback. The fallback excludes pauses and
    /// post-game time, but it can include pregame recording time. Therefore,
    /// it might not match the HUD clock exactly.
    ///
    /// Returns ``None`` if no game-over event was found.
    #[getter]
    pub(crate) fn regulation_ticks(&mut self) -> PyResult<Option<i32>> {
        let Some(seconds) = self.regulation_seconds()? else {
            return Ok(None);
        };
        Ok(Some((seconds * self.tick_rate as f32).round() as i32))
    }

    /// The HUD match-clock value at game over, in seconds.
    ///
    /// This excludes pregame, pauses, and post-game time. Old demos that omit
    /// the clock or set it to zero use active demo ticks as a fallback. The
    /// fallback excludes pauses and post-game time, but it can include pregame
    /// recording time. Therefore, it might not match the HUD clock exactly.
    ///
    /// Returns ``None`` if no game-over event was found.
    #[getter]
    pub(crate) fn regulation_seconds(&mut self) -> PyResult<Option<f32>> {
        if self.tick_rate == 0 {
            return Ok(None);
        }
        self.ensure_game_over_match_clock_scanned()?;
        if let Some(seconds) = self.game_over_match_clock {
            return Ok(Some(seconds));
        }

        let Some((_, tick)) = self.game_over else {
            return Ok(None);
        };
        self.ensure_paused_ticks_built()?;
        Ok(Some(
            self.count_active_ticks(tick) as f32 / self.tick_rate as f32,
        ))
    }

    /// The match-clock value at game over (for example, ``"32:45"``).
    ///
    /// This is the formatted counterpart to ``regulation_seconds``.
    ///
    /// Returns ``None`` if no game-over event was found.
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
