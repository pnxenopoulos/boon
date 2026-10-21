use crate::*;

#[pymethods]
impl Demo {
    /// Initial zipline geometry and later revisions, with full node handles.
    ///
    /// Returns:
    ///     pl.DataFrame: Initial zipline geometry and later revisions, with full node handles.
    ///
    /// Notes:
    ///     Columns: tick, node_handle, lane, x, y, z, connections,
    ///     cumulative_distance, capturable. No catalog is required.
    ///     Connections contain full handles; null differs from an empty vector.
    #[getter]
    pub(crate) fn zipline_nodes(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ZiplineNodes)
    }

    /// Recorded zipline ownership and visibility transitions.
    ///
    /// Returns:
    ///     pl.DataFrame: Recorded zipline ownership and visibility transitions.
    ///
    /// Notes:
    ///     Columns: tick, node_handle, change, team_num. Change values:
    ///     observed, owner_changed, left_pvs, removed. Missing teams remain null.
    ///     A PVS exit is not a deletion. No timeout or lane pressure is inferred.
    #[getter]
    pub(crate) fn zipline_changes(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ZiplineChanges)
    }

    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick.
    ///     - **steam_id** (*int, nullable*) -- Steam account ID (UInt64).
    ///     - **hero_id** (*int*) -- The player's hero ID.
    ///     - **boon_count** (*int, nullable*) -- Directly recorded boon count, when present in the replay schema.
    ///     - **upgrade_ids** (*list[int], nullable*) -- Complete current controller upgrade-token vector; classify item tokens with matching game data. Null means unavailable or undecodable.
    ///     - **ammo_fraction** (*float, nullable*) -- Recorded primary-gun ammo fraction.
    ///           Use snapshots(data_version=...) to calculate ammo and max_ammo.
    ///     - **x** (*float*) -- Player X position.
    ///     - **y** (*float*) -- Player Y position.
    ///     - **z** (*float*) -- Player Z position.
    ///     - **pitch** (*float*) -- Camera pitch angle.
    ///     - **yaw** (*float*) -- Camera yaw angle.
    ///     - **roll** (*float*) -- Camera roll angle.
    ///     - **in_regen_zone** (*bool*) -- Whether the player is in a regeneration zone.
    ///     - **in_item_shop** (*bool*) -- Whether the player is in an item shop zone.
    ///     - **death_time** (*float*) -- Time of death.
    ///     - **last_spawn_time** (*float*) -- Time of last spawn.
    ///     - **respawn_time** (*float*) -- Time until respawn.
    ///     - **health** (*int*) -- Current health.
    ///     - **max_health** (*int*) -- Effective maximum health (level + items +
    ///           buffs), from the controller's ``m_iHealthMax``.
    ///     - **barrier** (*float*) -- Current barrier remaining. Returns ``0.0`` when
    ///           the demo does not contain a barrier tracker for this player.
    ///     - **lifestate** (*int*) -- Life state value (use ``lifestate_names()`` to resolve).
    ///     - **souls** (*int*) -- Current souls (currency).
    ///     - **spent_souls** (*int*) -- Total spent souls.
    ///     - **in_combat_end_time** (*float*) -- In-combat timer end.
    ///     - **in_combat_last_damage_time** (*float*) -- In-combat last damage time.
    ///     - **in_combat_start_time** (*float*) -- In-combat timer start.
    ///     - **player_damage_dealt_end_time** (*float*) -- Player damage dealt timer end.
    ///     - **player_damage_dealt_last_damage_time** (*float*) -- Player damage dealt last damage time.
    ///     - **player_damage_dealt_start_time** (*float*) -- Player damage dealt timer start.
    ///     - **player_damage_taken_end_time** (*float*) -- Player damage taken timer end.
    ///     - **player_damage_taken_last_damage_time** (*float*) -- Player damage taken last damage time.
    ///     - **player_damage_taken_start_time** (*float*) -- Player damage taken timer start.
    ///     - **time_revealed_by_npc** (*float*) -- Time revealed on minimap by NPC.
    ///     - **build_id** (*int*) -- Hero build ID.
    ///     - **is_alive** (*bool*) -- Whether the player is alive.
    ///     - **has_rebirth** (*bool*) -- Whether the player has rebirth.
    ///     - **has_rejuvenator** (*bool*) -- Whether the player has rejuvenator.
    ///     - **has_ultimate_trained** (*bool*) -- Whether the player's ultimate is trained.
    ///     - **health_regen** (*float*) -- Health regeneration rate.
    ///     - **ultimate_cooldown_start** (*float*) -- Ultimate cooldown start time.
    ///     - **ultimate_cooldown_end** (*float*) -- Ultimate cooldown end time.
    ///     - **ap_net_worth** (*int*) -- Ability power net worth.
    ///     - **gold_net_worth** (*int*) -- Gold net worth.
    ///     - **denies** (*int*) -- Total denies.
    ///     - **hero_damage** (*int*) -- Total hero damage dealt.
    ///     - **hero_healing** (*int*) -- Total hero healing done.
    ///     - **objective_damage** (*int*) -- Total objective damage dealt.
    ///     - **self_healing** (*int*) -- Total self healing done.
    ///     - **kill_streak** (*int*) -- Current kill streak.
    ///     - **last_hits** (*int*) -- Total last hits.
    ///     - **level** (*int*) -- Current player level.
    ///     - **kills** (*int*) -- Total kills.
    ///     - **deaths** (*int*) -- Total deaths.
    ///     - **assists** (*int*) -- Total assists.
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    ///     Returns a DataFrame with one row per player per tick, containing
    ///     position, health, combat timers, kills, deaths, net worth, and more.
    ///     Rows where the pawn is not found or ``hero_id == 0`` are skipped.
    #[getter]
    pub(crate) fn player_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::PlayerTicks)
    }

    /// World state at every tick as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick.
    ///     - **is_paused** (*bool*) -- Whether the game is paused.
    ///     - **next_midboss** (*float*) -- Time until next midboss spawn.
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn world_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::WorldTicks)
    }

    /// Hero kill events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the kill occurred.
    ///     - **victim_hero_id** (*int*) -- The hero ID of the killed player.
    ///     - **attacker_hero_id** (*int*) -- The hero ID of the attacker.
    ///     - **assister_hero_ids** (*list[int]*) -- List of hero IDs of players who assisted.
    ///
    /// Notes:
    ///     Boon preloads this dataset unless ``preload=False``.
    ///     With preloading disabled, the first access loads and caches the data.
    #[getter]
    pub(crate) fn kills(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Kills)
    }

    /// Damage events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The enclosing demo command tick.
    ///     - **damage** (*int*) -- The damage dealt.
    ///     - **pre_damage** (*float*) -- The damage before mitigation.
    ///     - **damage_absorbed** (*float | None*) -- Recorded absorption; null when absent.
    ///     - **victim_shield_new** (*int | None*) -- Remaining shield after the hit.
    ///     - **victim_shield_max** (*int | None*) -- Shield capacity.
    ///     - **server_tick** (*int | None*) -- Server tick recorded in the damage message.
    ///     - **victim_hero_id** (*int*) -- The hero ID of the victim (0 if not a hero).
    ///     - **attacker_hero_id** (*int*) -- The hero ID of the attacker (0 if not a hero).
    ///     - **victim_health_new** (*int*) -- The victim's health after damage.
    ///     - **hitgroup_id** (*int*) -- The hitgroup that was hit (use ``hitgroup_names()`` to resolve).
    ///     - **crit_damage** (*float*) -- Critical damage amount.
    ///     - **attacker_class** (*int*) -- The attacker's entity class ID.
    ///     - **victim_class** (*int*) -- The victim's entity class ID.
    ///     - **victim_entity_id** (*int*) -- Victim entity index; -1 when absent.
    ///     - **ability_id** (*int*) -- The ability/weapon responsible, or 0 when absent.
    ///     - **damage_type** (*int*) -- Raw Source ``type`` damage bitfield.
    ///     - **citadel_type** (*int*) -- Deadlock damage category; 3 is melee-typed damage.
    ///     - **damage_flags** (*int*) -- Raw Valve damage flags.
    ///     - **is_melee** (*bool*) -- Whether this is melee-typed damage (``citadel_type == 3``).
    ///     - **melee_type** (*str | None*) -- ``"light"``, ``"heavy"``, or ``"other"`` for melee-typed damage; null otherwise.
    ///
    /// Notes:
    ///     Boon preloads this dataset unless ``preload=False``.
    ///     With preloading disabled, the first access loads and caches the data.
    #[getter]
    pub(crate) fn damage(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Damage)
    }

    /// Flex slot unlock events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the flex slot was unlocked.
    ///     - **team_num** (*int*) -- The team number that unlocked the flex slot.
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn flex_slots(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::FlexSlots)
    }

    /// Important ability usage events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the ability was used.
    ///     - **hero_id** (*int*) -- The hero ID of the player.
    ///     - **ability** (*str*) -- The ability name.
    ///
    /// Notes:
    ///     Boon preloads this dataset unless ``preload=False``.
    ///     With preloading disabled, the first access loads and caches the data.
    #[getter]
    pub(crate) fn abilities(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Abilities)
    }

    /// Hero ability point spending events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the upgrade occurred.
    ///     - **hero_id** (*int*) -- The hero ID of the player.
    ///     - **ability_id** (*int*) -- The raw MurmurHash2 ability ID.
    ///     - **tier** (*int*) -- Upgrade tier (1, 2, or 3).
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn ability_upgrades(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::AbilityUpgrades)
    }

    /// Read raw item events without catalog component links.
    ///
    /// Returns:
    ///     pl.DataFrame: Native results used by the public Python wrapper.
    #[getter]
    pub(crate) fn _item_purchases(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ItemPurchases)
    }

    /// In-game chat messages as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the message was sent.
    ///     - **hero_id** (*int*) -- The hero ID of the sender.
    ///     - **text** (*str*) -- The message text.
    ///     - **chat_type** (*str*) -- ``"all"`` or ``"team"``.
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn chat(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Chat)
    }

    /// Objective health state changes as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the change occurred.
    ///     - **objective_type** (*str*) -- ``"walker"``, ``"barracks"``, ``"shrine"``, ``"patron"``, or ``"mid_boss"``.
    ///     - **team_num** (*int*) -- The team that owns the objective.
    ///     - **lane** (*int*) -- Lane assignment (1, 4, or 6; 0 for patron/shrine/mid_boss).
    ///     - **health** (*int*) -- Current health.
    ///     - **max_health** (*int*) -- Maximum health.
    ///     - **phase** (*int*) -- Patron phase (use ``patron_phase_names()`` to resolve; 0=normal, 1=final, 2=transforming; 0 for non-patron).
    ///     - **x** (*float*) -- X position.
    ///     - **y** (*float*) -- Y position.
    ///     - **z** (*float*) -- Z position.
    ///     - **entity_id** (*int*) -- Entity index (stable per structure across ticks).
    ///     - **entity_serial** (*int*) -- Entity lifetime serial; pair with entity_id to distinguish slot reuse.
    ///
    /// Notes:
    ///     Emits a row when an objective's health, max_health, or phase changes.
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn objectives(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Objectives)
    }

    /// Mid boss lifecycle events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick.
    ///     - **team_num** (*int*) -- The team involved.
    ///     - **event** (*str*) -- ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
    ///
    /// Notes:
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn mid_boss(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::MidBoss)
    }

    /// Rift lifecycle as a Polars DataFrame -- one row per Rift.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **rift_num** (*int*) -- 1-based Rift index in the match. Entity
    ///           indices are recycled between Rifts, so this is the stable
    ///           identifier.
    ///     - **announce_tick** (*int | None*) -- Tick the spawner appeared,
    ///           ahead of the Rift becoming contestable. ``None`` if not observed.
    ///     - **active_tick** (*int*) -- Tick the Rift became contestable.
    ///     - **capture_tick** (*int | None*) -- Tick a team captured it, or
    ///           ``None`` if it expired.
    ///     - **expire_tick** (*int | None*) -- Tick it expired uncaptured, or
    ///           ``None`` if it was captured.
    ///     - **winning_team** (*int | None*) -- Team that captured it, from the
    ///           game rules' scoring team. ``None`` if it expired.
    ///     - **lane** (*int*) -- Lane the Rift spawned in (``1``/``6``
    ///           observed), or ``0`` when the location is not a known Rift site.
    ///     - **x** (*float*) -- X position of the cash-in in world (Hammer) units.
    ///     - **y** (*float*) -- Y position of the cash-in in world (Hammer) units.
    ///     - **z** (*float*) -- Z position of the cash-in in world (Hammer) units.
    ///
    /// Notes:
    ///     The Rift is a periodic king-of-the-hill objective (``Koth`` in the game
    ///     files). It is announced, becomes contestable, and then either is captured
    ///     by a team -- which grants that team buffed troopers in the Rift's lane --
    ///     or expires uncaptured.
    ///
    ///     Exactly one of ``capture_tick`` / ``expire_tick`` is set per row. Only
    ///     completed Rifts appear: one still live when the demo ends is omitted.
    ///
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn rift(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Rift)
    }

    /// Per-tick alive lane trooper state as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick.
    ///     - **trooper_type** (*str*) -- ``"trooper"`` or ``"trooper_boss"``.
    ///     - **team_num** (*int*) -- The trooper's team.
    ///     - **lane** (*int*) -- Lane assignment (1, 4, or 6).
    ///     - **health** (*int*) -- Current health.
    ///     - **max_health** (*int*) -- Maximum health.
    ///     - **x** (*float*) -- X position.
    ///     - **y** (*float*) -- Y position.
    ///     - **z** (*float*) -- Z position.
    ///     - **entity_id** (*int*) -- Entity index (stable per trooper across ticks).
    ///
    /// Notes:
    ///     **Warning:** This is a large dataset (~5M+ rows). Not loaded by default.
    ///     Access this property or call ``load("troopers")`` explicitly.
    #[getter]
    pub(crate) fn troopers(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Troopers)
    }

    /// Neutral creep state changes as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the state changed.
    ///     - **team_num** (*int*) -- The neutral's team.
    ///     - **health** (*int*) -- Current health.
    ///     - **max_health** (*int*) -- Maximum health.
    ///     - **x** (*float*) -- X position.
    ///     - **y** (*float*) -- Y position.
    ///     - **z** (*float*) -- Z position.
    ///     - **entity_id** (*int*) -- Entity index (stable per neutral across ticks).
    ///
    /// Notes:
    ///     Not loaded by default. Access this property or call ``load("neutrals")`` explicitly.
    #[getter]
    pub(crate) fn neutrals(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Neutrals)
    }

    /// Breakable map-prop destruction events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the prop was broken.
    ///     - **event** (*str*) -- Always "broken".
    ///     - **entity_id** (*int*) -- Entity slot index.
    ///     - **entity_serial** (*int*) -- Serial distinguishing reuse of the slot.
    ///     - **subclass_id** (*int*) -- Raw unsigned breakable subclass hash ID.
    ///     - **subclass_name** (*str*) -- Resolved internal subclass name, or "BREAKABLE_NOT_FOUND".
    ///     - **team_num** (*int*) -- The prop's last-known team.
    ///     - **x** (*float*) -- Last-known X position.
    ///     - **y** (*float*) -- Last-known Y position.
    ///     - **z** (*float*) -- Last-known Z position.
    ///
    /// Notes:
    ///     Deadlock represents a broken CCitadel_BreakableProp as an entity
    ///     leaving the PVS. Boon emits the candidate only if the same index and
    ///     serial never reactivate later in the demo, and ignores full-packet
    ///     delete/create replacements.
    ///
    ///     Health and lifestate are intentionally omitted because the server
    ///     does not report a final health-zero or dead state.
    ///
    ///     Not loaded by default. Access this property or call load("breakables") explicitly.
    ///     Subclass names use boon-data misc.json, downloading latest if no verified
    ///     local data exists. Recorded IDs are preserved even when names are unknown.
    #[getter]
    pub(crate) fn breakables(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Breakables)
    }

    /// Sinner's Sacrifice machine lifecycle and hit events.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the event occurred.
    ///     - **event** (*str*) -- ``"spawned"``, ``"hit"``, or ``"reset"``.
    ///     - **entity_id** (*int*) -- Entity slot index.
    ///     - **entity_serial** (*int*) -- Serial distinguishing reuse of the slot.
    ///     - **attacker_hero_id** (*int*) -- Attacker hero ID, or 0 when unresolved/not applicable.
    ///     - **damage** (*int*) -- Incoming damage, or 0 for spawned/reset events.
    ///     - **health** (*int*) -- Machine health at the end of this tick.
    ///     - **max_health** (*int*) -- Maximum machine health.
    ///     - **team_num** (*int*) -- The machine's team.
    ///     - **x** (*float*) -- X position.
    ///     - **y** (*float*) -- Y position.
    ///     - **z** (*float*) -- Z position.
    ///
    /// Notes:
    ///     Tracks both the standard and Hideout machine entity classes. Damage
    ///     messages provide the exact victim entity, incoming damage, and attacker
    ///     hero. An unmatched entity health decrease is retained as a hit with an
    ///     unresolved attacker. ``health`` is the machine state at the end of the
    ///     tick, so multiple hits in one tick can share the same value.
    ///
    ///     Not loaded by default. Access this property or call
    ///     ``load("sinners_sacrifice")`` explicitly.
    #[getter]
    pub(crate) fn sinners_sacrifice(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::SinnersSacrifice)
    }

    /// Permanent stat bonus change events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the stat changed.
    ///     - **hero_id** (*int*) -- The player's hero ID.
    ///     - **stat_type** (*str*) -- ``"health"``, ``"spirit_power"``, ``"fire_rate"``, ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``, ``"bullet_resist"``, or ``"spirit_resist"``.
    ///     - **amount** (*float*) -- The signed change from this event.
    ///
    /// Notes:
    ///     Not loaded by default. Access this property or call ``load("stat_modifier_events")`` explicitly.
    ///
    ///     Types come from the newest installed boon-data catalog. If no catalog is
    ///     installed, Boon downloads the latest version. Use data for the replay's client.
    ///     Emits a row whenever a recorded stat total changes.
    #[getter]
    pub(crate) fn stat_modifier_events(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StatModifierEvents)
    }

    /// Active buff/debuff modifier events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the modifier event occurred.
    ///     - **hero_id** (*int*) -- The affected player's hero ID.
    ///     - **event** (*str*) -- ``"applied"``, ``"changed"`` (stack count
    ///           changed while active), or ``"removed"``.
    ///     - **modifier_id** (*int*) -- Raw modifier subclass hash ID.
    ///     - **ability_id** (*int*) -- Raw ability subclass hash ID.
    ///     - **duration** (*float*) -- Modifier duration.
    ///     - **caster_hero_id** (*int*) -- Hero ID of the caster.
    ///     - **stacks** (*int*) -- Number of stacks. On ``removed``, the final count.
    ///
    /// Notes:
    ///     Not loaded by default. Access this property or call ``load("active_modifiers")`` explicitly.
    #[getter]
    pub(crate) fn active_modifiers(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::ActiveModifiers)
    }

    /// Ability cooldown / charge state changes as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick of the state change.
    ///     - **hero_id** (*int*) -- The owning player's hero ID.
    ///     - **ability_id** (*int*) -- Ability subclass hash (``CUtlStringToken``;
    ///           resolve with ``ability_names()``).
    ///     - **slot** (*int*) -- Ability slot (``EAbilitySlots_t``); signature
    ///           abilities use small values, innate movement abilities larger ones.
    ///     - **cooldown_start** (*float*) -- Game time the cooldown started.
    ///     - **cooldown_end** (*float*) -- Game time the ability is available again.
    ///     - **remaining_charges** (*int*) -- Charges currently available.
    ///     - **charge_recharge_start** (*float*) -- Game time the regenerating charge started.
    ///     - **charge_recharge_end** (*float*) -- Game time the regenerating charge completes.
    ///
    /// Notes:
    ///     Change-only: a row is emitted for an ability only on the tick its
    ///     cooldown or charge state changes, keeping the frame compact. One ability
    ///     entity exists per ability a player owns, including innate movement
    ///     abilities (jump, dash, slide, ...) which can be filtered out with ``slot``.
    ///
    ///     Not loaded by default. Access this property or call ``load("ability_ticks")`` explicitly.
    #[getter]
    pub(crate) fn ability_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::AbilityTicks)
    }

    /// Urn (idol) lifecycle events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick when the event occurred.
    ///     - **event** (*str*) -- ``"picked_up"``, ``"dropped"``, ``"returned"``, ``"delivery_active"``, or ``"delivery_inactive"``.
    ///     - **hero_id** (*int*) -- The hero involved (0 for delivery events).
    ///     - **team_num** (*int*) -- Team of the delivery point (0 for modifier events).
    ///     - **x** (*float*) -- Delivery point X position (0.0 for modifier events).
    ///     - **y** (*float*) -- Delivery point Y position (0.0 for modifier events).
    ///     - **z** (*float*) -- Delivery point Z position (0.0 for modifier events).
    ///
    /// Notes:
    ///     Not loaded by default. Access this property or call ``load("urn")`` explicitly.
    #[getter]
    pub(crate) fn urn(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::Urn)
    }

    /// Per-tick street brawl state as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **tick** (*int*) -- The game tick.
    ///     - **round** (*int*) -- Current round number.
    ///     - **state** (*int*) -- Street brawl state enum value.
    ///     - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) score.
    ///     - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) score.
    ///     - **buy_countdown** (*int*) -- Last buy phase countdown value.
    ///     - **next_state_time** (*float*) -- Time of next state transition.
    ///     - **state_start_time** (*float*) -- Time the current state started.
    ///     - **non_combat_time** (*float*) -- Total non-combat time elapsed.
    ///
    /// Raises:
    ///     NotStreetBrawlError: The demo is not a Street Brawl match.
    ///
    /// Notes:
    ///     Only available for street brawl demos (game_mode=4).
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn street_brawl_ticks(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StreetBrawlTicks)
    }

    /// Street brawl round scoring events as a Polars DataFrame.
    ///
    /// Returns:
    ///     Polars DataFrame with these columns:
    ///
    ///     - **round** (*int*) -- Sequential round number (1-indexed).
    ///     - **tick** (*int*) -- The game tick when the round ended.
    ///     - **scoring_team** (*int*) -- The team that scored.
    ///     - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) cumulative score.
    ///     - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) cumulative score.
    ///
    /// Raises:
    ///     NotStreetBrawlError: The demo is not a Street Brawl match.
    ///
    /// Notes:
    ///     Only available for street brawl demos (game_mode=4).
    ///     Boon loads this dataset on first access.
    #[getter]
    pub(crate) fn street_brawl_rounds(&mut self, py: Python<'_>) -> PyResult<PyDataFrame> {
        self.dataset_frame(py, Dataset::StreetBrawlRounds)
    }

    /// The team number of the winning team, or ``None`` if no game-over event was found.
    ///
    /// Returns:
    ///     int | None: The team number of the winning team, or ``None`` if no game-over event was found.
    #[getter]
    pub(crate) fn winning_team_num(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(team, _)| team))
    }

    /// The tick when the game ended, or ``None`` if no game-over event was found.
    ///
    /// Returns:
    ///     int | None: The tick when the game ended, or ``None`` if no game-over event was found.
    #[getter]
    pub(crate) fn game_over_tick(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(_, tick)| tick))
    }

    /// Match-clock ticks at game over.
    ///
    /// Returns:
    ///     int | None: Match-clock ticks at game over.
    ///
    /// Notes:
    ///     Uses the replicated HUD match clock. This excludes pregame, pauses, and
    ///     post-game time. Old demos that omit this field or set it to zero use
    ///     active demo ticks as a fallback. The fallback excludes pauses and
    ///     post-game time, but it can include pregame recording time. Therefore,
    ///     it might not match the HUD clock exactly. Returns ``None`` if no
    ///     game-over event was found.
    #[getter]
    pub(crate) fn regulation_ticks(&mut self) -> PyResult<Option<i32>> {
        let Some(seconds) = self.regulation_seconds()? else {
            return Ok(None);
        };
        Ok(Some((seconds * self.tick_rate as f32).round() as i32))
    }

    /// HUD match-clock value at game over, in seconds.
    ///
    /// Returns:
    ///     float | None: HUD match-clock value at game over, in seconds.
    ///
    /// Notes:
    ///     This excludes pregame, pauses, and post-game time. Old demos that omit
    ///     this field or set it to zero use active demo ticks as a fallback. The
    ///     fallback excludes pauses and post-game time, but it can include pregame
    ///     recording time. Therefore, it might not match the HUD clock exactly.
    ///     Returns ``None`` if no game-over event was found.
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

    /// Match-clock value at game over (for example, ``"32:45"``).
    ///
    /// Returns:
    ///     str | None: Match-clock value at game over (for example, ``"32:45"``).
    ///
    /// Notes:
    ///     This is the formatted counterpart to ``regulation_seconds``. Returns
    ///     ``None`` if no game-over event was found.
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
