from pathlib import Path

import polars as pl

class InvalidDemoError(Exception):
    """Raised when a demo file is invalid or cannot be parsed."""

    ...

class DemoHeaderError(Exception):
    """Raised when required fields are missing from the demo file header."""

    ...

class DemoInfoError(Exception):
    """Raised when required fields are missing from the demo file info."""

    ...

class DemoMessageError(Exception):
    """Raised when required data could not be resolved from demo messages."""

    ...

class NotStreetBrawlError(Exception):
    """Raised when accessing street brawl datasets on a non-street-brawl demo."""

    ...

def hero_names() -> dict[int, str]:
    """Return a mapping of hero ID to hero name."""
    ...

def team_names() -> dict[int, str]:
    """Return a mapping of team number to team name."""
    ...

def ability_names() -> dict[int, str]:
    """Return a mapping of MurmurHash2 ability ID to ability name."""
    ...

def ability_display_names() -> dict[str, str]:
    """Return exact internal ability/item names mapped to English display names.

    Includes exact localized top-level entries such as ability_*, upgrade_*,
    and citadel_ability_*. Unlocalized internal names are omitted rather than
    assigned a synthesized name.
    """
    ...

def modifier_names() -> dict[int, str]:
    """Return a mapping of MurmurHash2 modifier ID to modifier name."""
    ...

def game_mode_names() -> dict[int, str]:
    """Return a mapping of game mode ID to game mode name."""
    ...

def patron_phase_names() -> dict[int, str]:
    """Return a mapping of patron phase ID to phase name.

    Phases of ``CNPC_Boss_Tier3.m_ePhase``: ``0=normal`` (shielded),
    ``1=final`` (killable), ``2=transforming`` (vulnerable). Non-patron
    objectives report ``0`` by default.
    """
    ...

def hitgroup_names() -> dict[int, str]:
    """Return a mapping of hit group ID to hit group name.

    Hit group IDs are Source 2's ``HitGroup_t`` enum values, used by the
    ``hitgroup_id`` column on the ``damage`` frame (``0=generic``, ``1=head``,
    ``2=chest`` ... ``19=head_no_resist``; ``-1=invalid``).
    """
    ...

def lifestate_names() -> dict[int, str]:
    """Return a mapping of life state ID to life state name.

    Life state IDs are Source 2's ``LifeState_t`` enum values, used by the
    ``lifestate`` column on ``player_ticks`` (``0=alive``, ``1=dying``,
    ``2=dead``, ``3=respawnable``, ``4=respawning``).
    """
    ...

class Demo:
    """A Deadlock demo file.

    Args:
        path: Path to the demo file.

    Raises:
        FileNotFoundError: If the file does not exist.
        InvalidDemoError: If the file is not a valid demo file.
        DemoHeaderError: If required fields are missing from the file header.
        DemoInfoError: If required fields are missing from the file info.

    Example:
        >>> demo = Demo("match.dem")
        >>> demo.total_ticks
        54000
        >>> demo.players
        shape: (12, 5)
        ...
    """

    def __init__(self, path: str) -> None: ...
    def verify(self) -> bool:
        """Verify that the file is a valid demo file.

        Returns:
            True if the file is valid.

        Note:
            This is already called during construction, so it will always
            return True for an existing Demo instance.
        """
        ...

    @staticmethod
    def available_datasets() -> list[str]:
        """Return the list of dataset names that can be passed to ``load()`` or accessed as properties.

        Returns:
            A list of valid dataset name strings.

        Example:
            >>> Demo.available_datasets()
            ['abilities', 'ability_upgrades', 'chat', ...]
        """
        ...

    def load(self, *datasets: str) -> None:
        """Load and cache one or more datasets using compatible parser passes.

        Valid dataset names: see :meth:`available_datasets`.

        Already-loaded datasets are skipped. Event/entity datasets requested
        together share one filtered pass; snapshot datasets share one parallel
        keyframe-segmented pass, including in mixed requests.

        Args:
            *datasets: One or more dataset names to load.

        Raises:
            ValueError: If an unknown dataset name is provided.
            NotStreetBrawlError: If a street brawl dataset is requested on a
                non-street-brawl demo.

        Example:
            >>> demo = Demo("match.dem")
            >>> demo.load("kills", "player_ticks", "world_ticks")
            >>> demo.kills.shape
            (76, 4)
        """
        ...

    @property
    def path(self) -> Path:
        """The path to the demo file."""
        ...

    @property
    def total_ticks(self) -> int:
        """The total number of ticks in the demo."""
        ...

    @property
    def total_seconds(self) -> float:
        """The total duration of the demo in seconds."""
        ...

    @property
    def total_clock_time(self) -> str:
        """The total duration of the demo as a formatted string (for example, ``"12:34"``)."""
        ...

    @property
    def build(self) -> int:
        """The build number of the game that recorded the demo."""
        ...

    @property
    def map_name(self) -> str:
        """The name of the map the demo was recorded on."""
        ...

    @property
    def match_id(self) -> int | None:
        """The match ID for this demo, or ``None`` if the demo does not carry
        one (for example a partial capture or sandbox / custom content)."""
        ...

    @property
    def game_mode(self) -> int:
        """The game mode ID for this demo.

        Use ``game_mode_names()`` to resolve IDs to names.
        """
        ...

    @property
    def tick_rate(self) -> int:
        """The tick rate of the demo (ticks per second)."""
        ...

    def summary(self) -> dict[str, pl.DataFrame | None]:
        """Parse the post-match summary from the demo's ``PostMatchDetails`` event.

        Returns a dict with four top-level keys:

        - ``snapshots``: a Polars DataFrame with one row per (snapshot, player).
          Snapshots are taken at intervals through the match (not every minute);
          ``snapshot_time_s`` marks each one. Columns hold that player's running
          totals at that time: ``hero_id``, ``kills``, ``deaths``, ``assists``,
          ``net_worth``, ``denies``, ``level``, ``lane``, ``creep_kills``,
          ``neutral_kills``, ``player_damage``, and the per-source gold/orbs
          breakdown (``player_*``, ``lane_creep_*``, ``neutral_creep*``,
          ``boss_*``, ``treasure_*``, ``denies_*``, ``team_bonus_*``,
          ``breakable_*``, ``assassinate_*``, ``trophy_collector_*``,
          ``cultist_sacrifice_*``, ``assists_*``, and ``unknown_*``).
        - ``last_hits``: a Polars DataFrame of ``hero_id`` and ``last_hits`` (the
          final scoreboard last-hit / souls-secured total, only recorded per
          match, not per snapshot).
        - ``objectives``: a Polars DataFrame of post-match objective records
          (``team_objective_id``, ``team``, ``destroyed_time_s``,
          ``first_damage_time_s``, ``creep_damage``, ``player_damage``,
          ``player_spirit_damage``). ``destroyed_time_s``/``first_damage_time_s``
          are null when the objective was never destroyed/damaged.
        - ``damage``: a Polars DataFrame of the damage matrix — one row per
          (``dealer_player_slot``, ``target_player_slot``, ``source_name``,
          ``sample_time_s``). Dealer/target are also given as resolved
          ``dealer_hero_id``/``target_hero_id`` (null for non-player slots like
          0), so the frame joins to ``snapshots``/``last_hits`` on ``hero_id``.
          ``damage`` is the **per-interval** (additive) amount for that
          ``stat_type`` dealt during the interval ending at that sample; ``sum``
          for totals, ``cumsum`` over ``sample_time_s`` for the running total.
          ``stat_type`` is a string (``damage``, ``healing``, ``heal_prevented``,
          ``mitigated``, ``lethal``, ``regen``). ``is_category`` flags coarse
          damage-type buckets (``Bullet``/``Ability``/``Melee``/``Misc``/
          ``UnknownAbility``) which duplicate the specific-source rows, so filter
          to ``is_category == False`` for the complete, non-overlapping
          breakdown.

        The decoded message and all four frames are cached after the first call;
        repeated calls do not parse the demo or rebuild the frames.

        Raises ``DemoMessageError`` if the demo contains no post-match details
        (for example, an incomplete recording).
        """
        ...

    def snapshots(
        self,
        datasets: str | list[str] | None = ...,
        *,
        ticks: int | list[int] | None = ...,
        every: int | None = ...,
        seconds: float | None = ...,
        events: str | list[str] | None = ...,
        start_tick: int | None = ...,
        end_tick: int | None = ...,
    ) -> pl.DataFrame | dict[str, pl.DataFrame]:
        """Snapshot per-tick state at selected ticks in a single parallel pass.

        Decodes the demo once (across full-packet keyframe segments, in parallel)
        and collects rows only at the ticks you select — far cheaper than
        creating a full per-tick frame and filtering it in Python.

        Args:
            datasets: Which snapshot dataset(s) to return: ``"player_ticks"``
                (default), ``"world_ticks"``, ``"troopers"``, or a list of them.
            ticks: A specific tick or list of ticks.
            every: Sample every ``N`` ticks (gap-robust stride).
            seconds: Sample about once per ``seconds`` (converted with the tick
                rate). Mutually exclusive with ``every``.
            events: Sample at the ticks of these event datasets (for example ``"kills"``
                or ``["kills", "damage"]``).
            start_tick: Restrict to ticks at or after this tick.
            end_tick: Restrict to ticks at or before this tick.

        Returns:
            A single DataFrame when one dataset is requested, otherwise a dict
            keyed by dataset name. With only a window and no other selector, every
            tick in the window is returned; specifying no selector is a
            ``ValueError``.

        Example:
            >>> demo.snapshots(every=64)                    # ~1 row/sec of ticks
            >>> demo.snapshots(ticks=[29000, 30000])        # specific ticks
            >>> demo.snapshots("troopers", events="kills")  # troopers at kills
            >>> demo.snapshots(["player_ticks", "world_ticks"], seconds=1.0)
        """
        ...

    def stat_ticks(
        self,
        stats: str | list[str],
        *,
        ticks: int | list[int] | None = ...,
        every: int | None = ...,
        seconds: float | None = ...,
        events: str | list[str] | None = ...,
        start_tick: int | None = ...,
        end_tick: int | None = ...,
    ) -> pl.DataFrame:
        """Sample derived player stats at selected ticks.

        Each requested stat produces _native, _baseline, _effective, and
        _complete columns. Percentage stats use percentage points. Tick
        selectors match those supported by snapshots(). Values are analytical
        calculations; _complete reports known source/formula coverage, not
        independent verification of Valve's exact operation ordering, caps, or
        dynamic runtime values.
        """
        ...

    def stat_effects(
        self, stats: str | list[str] | None = ...
    ) -> pl.DataFrame:
        """Return change-only source details for generated stat contributions.

        Rows describe item and modifier apply/change/remove events, their layer,
        operation, resolved value, source IDs/names, caster/provider, stacks,
        duration, active state, and whether the value is complete.
        """
        ...

    def in_combat(self) -> pl.DataFrame:
        """Whether each player is in combat, per tick.

        Convenience method delegating to :func:`boon.stats.in_combat`. Returns
        one row per ``(tick, hero_id)`` -- so it joins directly onto
        ``player_ticks`` -- with a boolean ``in_combat`` column derived from the
        pawn's ``in_combat_end_time`` window.
        """
        ...

    def kill_participation(
        self, *, start_tick: int | None = ..., end_tick: int | None = ...
    ) -> pl.DataFrame:
        """Kill participation per player: ``(kills + assists) / team_kills``.

        Convenience method delegating to :func:`boon.stats.kill_participation`.
        Returns one row per player (``hero_id``, ``team_num``, ``kills``,
        ``assists``, ``team_kills``, ``kill_participation``), optionally
        restricted to a ``[start_tick, end_tick]`` window.
        """
        ...

    def time_dead(self) -> pl.DataFrame:
        """Time each player spent dead during regulation (non-paused ticks).

        Convenience method delegating to :func:`boon.stats.time_dead`. Returns
        one row per player (``hero_id``, ``team_num``, ``ticks_dead``,
        ``seconds_dead``, ``pct_regulation_dead``).

        Raises ``ValueError`` if the demo has no game-over event.
        """
        ...

    def teamfights(
        self, *, gap_seconds: float = ..., radius: float = ..., min_players: int = ...
    ) -> pl.DataFrame:
        """Detect teamfights from hero-vs-hero damage, clustered in space and time.

        Convenience method delegating to :func:`boon.stats.teamfights`. Clusters
        opposing hero-vs-hero damage events that are close in both time
        (``gap_seconds``) and location (``radius`` map units) into fights of at
        least ``min_players`` heroes, separating concurrent skirmishes in
        different lanes. Returns one row per fight (``fight_id``, ``start_tick``,
        ``end_tick``, ``start_seconds``, ``end_seconds``, ``duration_seconds``,
        ``center_x``, ``center_y``, ``participants``, ``num_participants``,
        ``hero_damage``, ``kills``).
        """
        ...

    def tick_to_seconds(self, tick: int) -> float:
        """Convert a tick number to seconds elapsed, excluding paused time.

        Automatically loads ``world_ticks`` on first call to determine pauses.

        Args:
            tick: The game tick to convert.

        Returns:
            The elapsed time in seconds, excluding pauses.
        """
        ...

    def tick_to_clock_time(self, tick: int) -> str:
        """Convert a tick number to a clock time string (for example, ``"3:14"``), excluding paused time.

        Automatically loads ``world_ticks`` on first call to determine pauses.

        Args:
            tick: The game tick to convert.

        Returns:
            A formatted string like ``"3:14"`` or ``"12:34"``.
        """
        ...

    @property
    def winning_team_num(self) -> int | None:
        """The team number of the winning team, or ``None`` if no game-over event was found."""
        ...

    @property
    def game_over_tick(self) -> int | None:
        """The tick when the game ended, or ``None`` if no game-over event was found."""
        ...

    @property
    def regulation_ticks(self) -> int | None:
        """Match-clock ticks at game over.

        Uses the replicated HUD match clock. This excludes pregame, pauses, and
        post-game time. Old demos that omit this field or set it to zero use
        active demo ticks as a fallback. The fallback excludes pauses and
        post-game time, but it can include pregame recording time. Therefore,
        it might not match the HUD clock exactly. Returns ``None`` if no
        game-over event was found.
        """
        ...

    @property
    def regulation_seconds(self) -> float | None:
        """HUD match-clock value at game over, in seconds.

        This excludes pregame, pauses, and post-game time. Old demos that omit
        this field or set it to zero use active demo ticks as a fallback. The
        fallback excludes pauses and post-game time, but it can include pregame
        recording time. Therefore, it might not match the HUD clock exactly.
        Returns ``None`` if no game-over event was found.
        """
        ...

    @property
    def regulation_clock_time(self) -> str | None:
        """Match-clock value at game over (for example, ``"32:45"``).

        This is the formatted counterpart to ``regulation_seconds``. Returns
        ``None`` if no game-over event was found.
        """
        ...


    @property
    def players(self) -> pl.DataFrame:
        """Player information as a Polars DataFrame.

        Columns:
            - **player_name** (*str*) -- The player's display name.
            - **steam_id** (*int*) -- The player's Steam ID.
            - **hero_id** (*int*) -- The player's hero ID.
            - **team_num** (*int*) -- The player's raw team number.
            - **start_lane** (*int*) -- The player's original lane color (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the ``CMsgLaneColor`` proto enum).
            - **rank** (*int*) -- The player's packed competitive display rank; 0 means unranked, calibrating, or unavailable.
        """
        ...

    @property
    def banned_heroes(self) -> pl.DataFrame:
        """Heroes banned from this match as a Polars DataFrame.

        Read from the single ``BannedHeroes`` user message, which the server
        sends early in the demo (before the match starts) only when the match
        has bans. The message carries nothing but the hero IDs -- no team, no
        banning player, and no pick/ban ordering -- so this cannot be used to
        build a draft.

        An empty DataFrame means no bans were recorded for this match. Demos
        from builds that never emit the message are indistinguishable from
        ban-free matches, so treat empty as "nothing recorded" rather than as
        positive proof that nothing was banned.

        Columns:
            - **hero_id** (*int*) -- The banned hero's ID (joins to ``players.hero_id``).
            - **hero_name** (*str*) -- The resolved hero name, or ``"HERO_NOT_FOUND"`` for an ID that predates the bundled hero table.
        """
        ...

    @property
    def player_ticks(self) -> pl.DataFrame:
        """Per-tick, per-player state as a Polars DataFrame.

        Boon loads this dataset on first access.
        Returns a DataFrame with one row per player per tick, containing
        position, health, combat timers, kills, deaths, net worth, and more.
        Rows where the pawn is not found or ``hero_id == 0`` are skipped.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **hero_id** (*int*) -- The player's hero ID.
            - **x** (*float*) -- Player X position.
            - **y** (*float*) -- Player Y position.
            - **z** (*float*) -- Player Z position.
            - **pitch** (*float*) -- Camera pitch angle.
            - **yaw** (*float*) -- Camera yaw angle.
            - **roll** (*float*) -- Camera roll angle.
            - **in_regen_zone** (*bool*) -- Whether the player is in a regeneration zone.
            - **in_item_shop** (*bool*) -- Whether the player is in an item shop zone.
            - **death_time** (*float*) -- Time of death.
            - **last_spawn_time** (*float*) -- Time of last spawn.
            - **respawn_time** (*float*) -- Time until respawn.
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Effective maximum health (level + items +
              buffs), from the controller's ``m_iHealthMax``.
            - **barrier** (*float*) -- Current barrier remaining. Returns ``0.0`` when
              the demo does not contain a barrier tracker for this player.
            - **bullet_resist_baseline** (*float*) -- Baseline bullet/gun damage
              resistance in percentage points from hero progression and
              unconditional equipped-item stats.
            - **spirit_resist_baseline** (*float*) -- Baseline spirit damage
              resistance in percentage points from hero progression and
              unconditional equipped-item stats.
            - **lifestate** (*int*) -- Life state value (use ``lifestate_names()`` to resolve).
            - **souls** (*int*) -- Current souls (currency).
            - **spent_souls** (*int*) -- Total spent souls.
            - **in_combat_end_time** (*float*) -- In-combat timer end.
            - **in_combat_last_damage_time** (*float*) -- In-combat last damage time.
            - **in_combat_start_time** (*float*) -- In-combat timer start.
            - **player_damage_dealt_end_time** (*float*) -- Player damage dealt timer end.
            - **player_damage_dealt_last_damage_time** (*float*) -- Player damage dealt last damage time.
            - **player_damage_dealt_start_time** (*float*) -- Player damage dealt timer start.
            - **player_damage_taken_end_time** (*float*) -- Player damage taken timer end.
            - **player_damage_taken_last_damage_time** (*float*) -- Player damage taken last damage time.
            - **player_damage_taken_start_time** (*float*) -- Player damage taken timer start.
            - **time_revealed_by_npc** (*float*) -- Time revealed on minimap by NPC.
            - **build_id** (*int*) -- Hero build ID.
            - **is_alive** (*bool*) -- Whether the player is alive.
            - **has_rebirth** (*bool*) -- Whether the player has rebirth.
            - **has_rejuvenator** (*bool*) -- Whether the player has rejuvenator.
            - **has_ultimate_trained** (*bool*) -- Whether the player's ultimate is trained.
            - **health_regen** (*float*) -- Health regeneration rate.
            - **ultimate_cooldown_start** (*float*) -- Ultimate cooldown start time.
            - **ultimate_cooldown_end** (*float*) -- Ultimate cooldown end time.
            - **ap_net_worth** (*int*) -- Ability power net worth.
            - **gold_net_worth** (*int*) -- Gold net worth.
            - **denies** (*int*) -- Total denies.
            - **hero_damage** (*int*) -- Total hero damage dealt.
            - **hero_healing** (*int*) -- Total hero healing done.
            - **objective_damage** (*int*) -- Total objective damage dealt.
            - **self_healing** (*int*) -- Total self healing done.
            - **kill_streak** (*int*) -- Current kill streak.
            - **last_hits** (*int*) -- Total last hits.
            - **level** (*int*) -- Current player level.
            - **kills** (*int*) -- Total kills.
            - **deaths** (*int*) -- Total deaths.
            - **assists** (*int*) -- Total assists.
        """
        ...

    @property
    def world_ticks(self) -> pl.DataFrame:
        """World state at every tick as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **is_paused** (*bool*) -- Whether the game is paused.
            - **next_midboss** (*float*) -- Time until next midboss spawn.
        """
        ...

    @property
    def kills(self) -> pl.DataFrame:
        """Hero kill events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the kill occurred.
            - **victim_hero_id** (*int*) -- The hero ID of the killed player.
            - **attacker_hero_id** (*int*) -- The hero ID of the attacker.
            - **assister_hero_ids** (*list[int]*) -- List of hero IDs of players who assisted.
        """
        ...

    @property
    def damage(self) -> pl.DataFrame:
        """Damage events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the damage occurred.
            - **damage** (*int*) -- The damage dealt.
            - **pre_damage** (*float*) -- The damage before mitigation.
            - **victim_hero_id** (*int*) -- The hero ID of the victim (0 if not a hero).
            - **attacker_hero_id** (*int*) -- The hero ID of the attacker (0 if not a hero).
            - **victim_health_new** (*int*) -- The victim's health after damage.
            - **hitgroup_id** (*int*) -- The hitgroup that was hit (use ``hitgroup_names()`` to resolve).
            - **crit_damage** (*float*) -- Critical damage amount.
            - **attacker_class** (*int*) -- The attacker's entity class ID.
            - **victim_class** (*int*) -- The victim's entity class ID.
            - **ability_id** (*int*) -- The ability/weapon responsible, or 0 when absent.
            - **damage_type** (*int*) -- Raw Source ``type`` damage bitfield.
            - **citadel_type** (*int*) -- Deadlock damage category; 3 is melee-typed damage.
            - **damage_flags** (*int*) -- Raw Valve damage flags.
            - **is_melee** (*bool*) -- Whether this is melee-typed damage (``citadel_type == 3``).
            - **melee_type** (*str | None*) -- ``"light"``, ``"heavy"``, or ``"other"`` for melee-typed damage; null otherwise.
        """
        ...

    @property
    def healing(self) -> pl.DataFrame:
        """Per-event healing as a Polars DataFrame.

        Heals are carried by ``CCitadelUserMessage_Damage`` as a negative
        ``health_lost`` (the ``damage`` field itself stays non-negative). This
        dataset keeps only those rows and reports ``amount`` as the positive
        health restored. Barrier / shield grants are not in this message, so
        this is health healing only; shielding needs another source.

        Not loaded by default. Access this property or call ``load("healing")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the heal occurred.
            - **target_hero_id** (*int*) -- The healed hero (0 if not a hero).
            - **source_hero_id** (*int*) -- The healer (0 if not a hero or self / none).
            - **amount** (*int*) -- Health restored (positive).
            - **ability_id** (*int*) -- The ability / item that healed (0 if absent).
            - **citadel_type** (*int*) -- The Deadlock damage category the heal came through.
        """
        ...

    @property
    def flex_slots(self) -> pl.DataFrame:
        """Flex slot unlock events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the flex slot was unlocked.
            - **team_num** (*int*) -- The team number that unlocked the flex slot.
        """
        ...

    @property
    def abilities(self) -> pl.DataFrame:
        """Important ability usage events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the ability was used.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability** (*str*) -- The ability name.
        """
        ...

    @property
    def ability_upgrades(self) -> pl.DataFrame:
        """Hero ability point spending events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the upgrade occurred.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability_id** (*int*) -- The raw MurmurHash2 ability ID.
            - **tier** (*int*) -- Upgrade tier (1, 2, or 3).
        """
        ...

    @property
    def item_purchases(self) -> pl.DataFrame:
        """Item shop transactions as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the transaction occurred.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability_id** (*int*) -- The raw MurmurHash2 item/ability ID.
            - **change** (*str*) -- Transaction type: ``"purchased"``, ``"upgraded"``, ``"sold"``, ``"swapped"``, ``"failure"``.
        """
        ...

    @property
    def chat(self) -> pl.DataFrame:
        """In-game chat messages as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the message was sent.
            - **hero_id** (*int*) -- The hero ID of the sender.
            - **text** (*str*) -- The message text.
            - **chat_type** (*str*) -- ``"all"`` or ``"team"``.
        """
        ...

    @property
    def objectives(self) -> pl.DataFrame:
        """Objective health state changes as a Polars DataFrame.

        Emits a row when an objective's health, max_health, or phase changes.
        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick when the change occurred.
            - **objective_type** (*str*) -- ``"walker"``, ``"barracks"``, ``"shrine"``, ``"patron"``, or ``"mid_boss"``.
            - **team_num** (*int*) -- The team that owns the objective.
            - **lane** (*int*) -- Lane assignment (1, 4, or 6; 0 for patron/shrine/mid_boss).
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **phase** (*int*) -- Patron phase (use ``patron_phase_names()`` to resolve; 0=normal, 1=final, 2=transforming; 0 for non-patron).
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
            - **entity_id** (*int*) -- Entity index (stable per structure across ticks).
        """
        ...

    @property
    def mid_boss(self) -> pl.DataFrame:
        """Mid boss lifecycle events as a Polars DataFrame.

        Boon loads this dataset on first access.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **team_num** (*int*) -- The team involved.
            - **event** (*str*) -- ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
        """
        ...

    @property
    def rift(self) -> pl.DataFrame:
        """Rift lifecycle as a Polars DataFrame -- one row per Rift.

        The Rift is a periodic king-of-the-hill objective (``Koth`` in the game
        files). It is announced, becomes contestable, and then either is captured
        by a team -- which grants that team buffed troopers in the Rift's lane --
        or expires uncaptured.

        Exactly one of ``capture_tick`` / ``expire_tick`` is set per row. Only
        completed Rifts appear: one still live when the demo ends is omitted.

        Boon loads this dataset on first access.

        Columns:
            - **rift_num** (*int*) -- 1-based Rift index in the match. Entity
              indices are recycled between Rifts, so this is the stable
              identifier.
            - **announce_tick** (*int | None*) -- Tick the spawner appeared,
              ahead of the Rift becoming contestable. ``None`` if not observed.
            - **active_tick** (*int*) -- Tick the Rift became contestable.
            - **capture_tick** (*int | None*) -- Tick a team captured it, or
              ``None`` if it expired.
            - **expire_tick** (*int | None*) -- Tick it expired uncaptured, or
              ``None`` if it was captured.
            - **winning_team** (*int | None*) -- Team that captured it, from the
              game rules' scoring team. ``None`` if it expired.
            - **lane** (*int*) -- Lane the Rift spawned in (``1``/``6``
              observed), or ``0`` when the location is not a known Rift site.
            - **x** (*float*) -- X position of the cash-in in world (Hammer) units.
            - **y** (*float*) -- Y position of the cash-in in world (Hammer) units.
            - **z** (*float*) -- Z position of the cash-in in world (Hammer) units.
        """
        ...

    @property
    def troopers(self) -> pl.DataFrame:
        """Per-tick alive lane trooper state as a Polars DataFrame.

        **Warning:** This is a large dataset (~5M+ rows). Not loaded by default.
        Access this property or call ``load("troopers")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **trooper_type** (*str*) -- ``"trooper"`` or ``"trooper_boss"``.
            - **team_num** (*int*) -- The trooper's team.
            - **lane** (*int*) -- Lane assignment (1, 4, or 6).
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
            - **entity_id** (*int*) -- Entity index (stable per trooper across ticks).
        """
        ...

    @property
    def neutrals(self) -> pl.DataFrame:
        """Neutral creep state changes as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("neutrals")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the state changed.
            - **team_num** (*int*) -- The neutral's team.
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
            - **entity_id** (*int*) -- Entity index (stable per neutral across ticks).
        """
        ...

    @property
    def breakables(self) -> pl.DataFrame:
        """Breakable map-prop destruction events as a Polars DataFrame.

        Deadlock represents a broken CCitadel_BreakableProp as an entity
        leaving the PVS. Boon emits the candidate only if the same index and
        serial never reactivate later in the demo, and ignores full-packet
        delete/create replacements.

        Health and lifestate are intentionally omitted because the server
        does not report a final health-zero or dead state.

        Not loaded by default. Access this property or call load("breakables") explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the prop was broken.
            - **event** (*str*) -- Always "broken".
            - **entity_id** (*int*) -- Entity slot index.
            - **entity_serial** (*int*) -- Serial distinguishing reuse of the slot.
            - **subclass_id** (*int*) -- Raw unsigned breakable subclass hash ID.
            - **subclass_name** (*str*) -- Resolved internal subclass name, or "BREAKABLE_NOT_FOUND".
            - **team_num** (*int*) -- The prop's last-known team.
            - **x** (*float*) -- Last-known X position.
            - **y** (*float*) -- Last-known Y position.
            - **z** (*float*) -- Last-known Z position.
        """
        ...

    @property
    def sinners_sacrifice(self) -> pl.DataFrame:
        """Sinner's Sacrifice machine lifecycle and hit events.

        Tracks both the standard and Hideout machine entity classes. Damage
        messages provide the exact victim entity, incoming damage, and attacker
        hero. An unmatched entity health decrease is retained as a hit with an
        unresolved attacker. ``health`` is the machine state at the end of the
        tick, so multiple hits in one tick can share the same value.

        Not loaded by default. Access this property or call
        ``load("sinners_sacrifice")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the event occurred.
            - **event** (*str*) -- ``"spawned"``, ``"hit"``, or ``"reset"``.
            - **entity_id** (*int*) -- Entity slot index.
            - **entity_serial** (*int*) -- Serial distinguishing reuse of the slot.
            - **attacker_hero_id** (*int*) -- Attacker hero ID, or 0 when unresolved/not applicable.
            - **damage** (*int*) -- Incoming damage, or 0 for spawned/reset events.
            - **health** (*int*) -- Machine health at the end of this tick.
            - **max_health** (*int*) -- Maximum machine health.
            - **team_num** (*int*) -- The machine's team.
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
        """
        ...

    @property
    def stat_modifier_events(self) -> pl.DataFrame:
        """Permanent stat bonus change events as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("stat_modifier_events")`` explicitly.

        Emits a row whenever a stat total changes (urn/breakable pickups).

        Columns:
            - **tick** (*int*) -- The game tick when the stat changed.
            - **hero_id** (*int*) -- The player's hero ID.
            - **stat_type** (*str*) -- ``"health"``, ``"spirit_power"``, ``"fire_rate"``, ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``, ``"bullet_resist"``, or ``"spirit_resist"``.
            - **amount** (*float*) -- The signed change from this event.
        """
        ...

    @property
    def active_modifiers(self) -> pl.DataFrame:
        """Active buff/debuff modifier events as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("active_modifiers")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the modifier event occurred.
            - **hero_id** (*int*) -- The affected player's hero ID.
            - **event** (*str*) -- ``"applied"``, ``"changed"`` (stack count
              changed while active), or ``"removed"``.
            - **modifier_id** (*int*) -- Raw modifier subclass hash ID.
            - **ability_id** (*int*) -- Raw ability subclass hash ID.
            - **duration** (*float*) -- Modifier duration.
            - **caster_hero_id** (*int*) -- Hero ID of the caster.
            - **stacks** (*int*) -- Number of stacks. On ``removed``, the final count.
        """
        ...

    @property
    def ability_ticks(self) -> pl.DataFrame:
        """Ability cooldown / charge state changes as a Polars DataFrame.

        Change-only: a row is emitted for an ability only on the tick its
        cooldown or charge state changes, keeping the frame compact. One ability
        entity exists per ability a player owns, including innate movement
        abilities (jump, dash, slide, ...) which can be filtered out with ``slot``.

        Not loaded by default. Access this property or call ``load("ability_ticks")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick of the state change.
            - **hero_id** (*int*) -- The owning player's hero ID.
            - **ability_id** (*int*) -- Ability subclass hash (``CUtlStringToken``;
              resolve with ``ability_names()``).
            - **slot** (*int*) -- Ability slot (``EAbilitySlots_t``); signature
              abilities use small values, innate movement abilities larger ones.
            - **cooldown_start** (*float*) -- Game time the cooldown started.
            - **cooldown_end** (*float*) -- Game time the ability is available again.
            - **remaining_charges** (*int*) -- Charges currently available.
            - **charge_recharge_start** (*float*) -- Game time the regenerating charge started.
            - **charge_recharge_end** (*float*) -- Game time the regenerating charge completes.
        """
        ...

    @property
    def urn(self) -> pl.DataFrame:
        """Urn (idol) lifecycle events as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("urn")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the event occurred.
            - **event** (*str*) -- ``"picked_up"``, ``"dropped"``, ``"returned"``, ``"delivery_active"``, or ``"delivery_inactive"``.
            - **hero_id** (*int*) -- The hero involved (0 for delivery events).
            - **team_num** (*int*) -- Team of the delivery point (0 for modifier events).
            - **x** (*float*) -- Delivery point X position (0.0 for modifier events).
            - **y** (*float*) -- Delivery point Y position (0.0 for modifier events).
            - **z** (*float*) -- Delivery point Z position (0.0 for modifier events).
        """
        ...

    @property
    def street_brawl_ticks(self) -> pl.DataFrame:
        """Per-tick street brawl state as a Polars DataFrame.

        Only available for street brawl demos (game_mode=4).
        Boon loads this dataset on first access.

        Raises:
            NotStreetBrawlError: If the demo is not a street brawl game.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **round** (*int*) -- Current round number.
            - **state** (*int*) -- Street brawl state enum value.
            - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) score.
            - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) score.
            - **buy_countdown** (*int*) -- Last buy phase countdown value.
            - **next_state_time** (*float*) -- Time of next state transition.
            - **state_start_time** (*float*) -- Time the current state started.
            - **non_combat_time** (*float*) -- Total non-combat time elapsed.
        """
        ...

    @property
    def street_brawl_rounds(self) -> pl.DataFrame:
        """Street brawl round scoring events as a Polars DataFrame.

        Only available for street brawl demos (game_mode=4).
        Boon loads this dataset on first access.

        Raises:
            NotStreetBrawlError: If the demo is not a street brawl game.

        Columns:
            - **round** (*int*) -- Sequential round number (1-indexed).
            - **tick** (*int*) -- The game tick when the round ended.
            - **scoring_team** (*int*) -- The team that scored.
            - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) cumulative score.
            - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) cumulative score.
        """
        ...
