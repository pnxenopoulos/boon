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

def modifier_names() -> dict[int, str]:
    """Return a mapping of MurmurHash2 modifier ID to modifier name."""
    ...

def game_mode_names() -> dict[int, str]:
    """Return a mapping of game mode ID to game mode name."""
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
        DemoMessageError: If the match ID could not be resolved from game entities.

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
            ['abilities', 'ability_upgrades', 'boss_kills', ...]
        """
        ...

    def load(self, *datasets: str) -> None:
        """Load one or more datasets from the demo file in a single pass.

        Valid dataset names: see :meth:`available_datasets`.

        Already-loaded datasets are skipped. Multiple datasets requested together
        share a single parse pass over the file for efficiency.

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
        """The total duration of the demo as a formatted string (e.g., ``"12:34"``)."""
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
    def match_id(self) -> int:
        """The match ID for this demo."""
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
        """Convert a tick number to a clock time string (e.g., ``"3:14"``), excluding paused time.

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
    def players(self) -> pl.DataFrame:
        """Player information as a Polars DataFrame.

        Columns:
            - **player_name** (*str*) -- The player's display name.
            - **steam_id** (*int*) -- The player's Steam ID.
            - **hero_id** (*int*) -- The player's hero ID.
            - **team_num** (*int*) -- The player's raw team number.
            - **start_lane** (*int*) -- The player's original lane (1=left, 4=center, 6=right).
        """
        ...

    @property
    def player_ticks(self) -> pl.DataFrame:
        """Per-tick, per-player state as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.
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
            - **death_time** (*float*) -- Time of death.
            - **last_spawn_time** (*float*) -- Time of last spawn.
            - **respawn_time** (*float*) -- Time until respawn.
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **lifestate** (*int*) -- Life state value.
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

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **is_paused** (*bool*) -- Whether the game is paused.
            - **next_midboss** (*float*) -- Time until next midboss spawn.
        """
        ...

    @property
    def kills(self) -> pl.DataFrame:
        """Hero kill events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

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

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the damage occurred.
            - **damage** (*int*) -- The damage dealt.
            - **pre_damage** (*float*) -- The damage before mitigation.
            - **victim_hero_id** (*int*) -- The hero ID of the victim (0 if not a hero).
            - **attacker_hero_id** (*int*) -- The hero ID of the attacker (0 if not a hero).
            - **victim_health_new** (*int*) -- The victim's health after damage.
            - **hitgroup_id** (*int*) -- The hitgroup that was hit.
            - **crit_damage** (*float*) -- Critical damage amount.
            - **attacker_class** (*int*) -- The attacker's entity class ID.
            - **victim_class** (*int*) -- The victim's entity class ID.
        """
        ...

    @property
    def flex_slots(self) -> pl.DataFrame:
        """Flex slot unlock events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the flex slot was unlocked.
            - **team_num** (*int*) -- The team number that unlocked the flex slot.
        """
        ...

    @property
    def abilities(self) -> pl.DataFrame:
        """Important ability usage events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the ability was used.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability** (*str*) -- The ability name.
        """
        ...

    @property
    def ability_upgrades(self) -> pl.DataFrame:
        """Hero ability point spending events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

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

        Auto-loads on first access if not already loaded via :meth:`load`.

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

        Auto-loads on first access if not already loaded via :meth:`load`.

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
        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the change occurred.
            - **objective_type** (*str*) -- ``"walker"``, ``"barracks"``, ``"shrine"``, ``"patron"``, or ``"mid_boss"``.
            - **team_num** (*int*) -- The team that owns the objective.
            - **lane** (*int*) -- Lane assignment (1, 4, or 6; 0 for patron/shrine/mid_boss).
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **phase** (*int*) -- Patron phase (0=normal, 2=shields down, 1=final phase; 0 for non-patron).
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
        """
        ...

    @property
    def boss_kills(self) -> pl.DataFrame:
        """Objective destruction events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the objective was destroyed.
            - **objective_team** (*int*) -- The team that owned the destroyed objective.
            - **objective_id** (*int*) -- Objective mask change ID.
            - **entity_class** (*str*) -- ``"walker"``, ``"barracks"``, ``"shrine"``, ``"mid_boss"``, ``"patron_shields_down"``, ``"patron"``.
            - **gametime** (*float*) -- The game time when the objective was destroyed.
        """
        ...

    @property
    def mid_boss(self) -> pl.DataFrame:
        """Mid boss lifecycle events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick.
            - **hero_id** (*int*) -- The hero involved (0 for spawn/kill events).
            - **team_num** (*int*) -- The team involved.
            - **event** (*str*) -- ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
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
        """
        ...

    @property
    def neutrals(self) -> pl.DataFrame:
        """Neutral creep state changes as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("neutrals")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the state changed.
            - **neutral_type** (*str*) -- ``"neutral"`` or ``"neutral_node_mover"``.
            - **team_num** (*int*) -- The neutral's team.
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
        """
        ...

    @property
    def stat_modifier_events(self) -> pl.DataFrame:
        """Permanent stat bonus change events as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("stat_modifier_events")`` explicitly.

        Emits a row whenever a stat total changes (idol/breakable pickups).

        Columns:
            - **tick** (*int*) -- The game tick when the stat changed.
            - **hero_id** (*int*) -- The player's hero ID.
            - **stat_type** (*str*) -- ``"health"``, ``"spirit_power"``, ``"fire_rate"``, ``"weapon_damage"``, ``"cooldown_reduction"``, or ``"ammo"``.
            - **amount** (*float*) -- The increase from this event.
        """
        ...

    @property
    def active_modifiers(self) -> pl.DataFrame:
        """Active buff/debuff modifier events as a Polars DataFrame.

        Not loaded by default. Access this property or call ``load("active_modifiers")`` explicitly.

        Columns:
            - **tick** (*int*) -- The game tick when the modifier event occurred.
            - **hero_id** (*int*) -- The affected player's hero ID.
            - **event** (*str*) -- ``"applied"`` or ``"removed"``.
            - **modifier_id** (*int*) -- Raw modifier subclass hash ID.
            - **ability_id** (*int*) -- Raw ability subclass hash ID.
            - **duration** (*float*) -- Modifier duration.
            - **caster_hero_id** (*int*) -- Hero ID of the caster.
            - **stacks** (*int*) -- Number of stacks.
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
        Auto-loads on first access if not already loaded via :meth:`load`.

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
        Auto-loads on first access if not already loaded via :meth:`load`.

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
