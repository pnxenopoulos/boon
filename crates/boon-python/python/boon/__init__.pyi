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
        shape: (12, 7)
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

    def load(self, *datasets: str) -> None:
        """Load one or more datasets from the demo file in a single pass.

        Valid dataset names: ``"player_ticks"``, ``"world_ticks"``, ``"kills"``, ``"damage"``, ``"flex_slots"``, ``"respawns"``, ``"purchases"``.
        Already-loaded datasets are skipped. Multiple datasets requested together
        share a single parse pass over the file for efficiency.

        Args:
            *datasets: One or more dataset names to load.

        Raises:
            ValueError: If an unknown dataset name is provided.

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
    def tick_rate(self) -> int:
        """The tick rate of the demo (ticks per second)."""
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
    def winning_team(self) -> str | None:
        """The name of the winning team (e.g., ``"Archmother"``), or ``None`` if no game-over event was found."""
        ...

    @property
    def banned_hero_ids(self) -> list[int]:
        """List of banned hero IDs. Returns an empty list if no banned heroes event was found."""
        ...

    @property
    def banned_heroes(self) -> list[str]:
        """List of banned hero names. Returns an empty list if no banned heroes event was found."""
        ...

    @property
    def teams(self) -> pl.DataFrame:
        """Team number to team name mapping as a Polars DataFrame.

        Columns:
            - **team_num** (*int*) -- The raw team number (1=Spectator, 2=Hidden King, 3=Archmother).
            - **team_name** (*str*) -- The team name.
        """
        ...

    @property
    def players(self) -> pl.DataFrame:
        """Player information as a Polars DataFrame.

        Columns:
            - **player_name** (*str*) -- The player's display name.
            - **steam_id** (*int*) -- The player's Steam ID.
            - **hero** (*str*) -- The player's hero name.
            - **hero_id** (*int*) -- The player's hero ID.
            - **team** (*str*) -- The player's team (``"Archmother"``, ``"Hidden King"``, or ``"Spectator"``).
            - **team_num** (*int*) -- The player's raw team number.
            - **start_lane** (*int*) -- The player's original lane (1=left, 4=center, 6=right).
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
    def purchases(self) -> pl.DataFrame:
        """Item purchase events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the purchase occurred.
            - **hero_id** (*int*) -- The hero ID of the purchasing player.
            - **ability_id** (*int*) -- The raw ability/item hash ID.
            - **ability** (*str*) -- The ability/item name purchased.
            - **sell** (*bool*) -- Whether this was a sell event.
            - **quickbuy** (*bool*) -- Whether this was a quickbuy purchase.
        """
        ...

    @property
    def respawns(self) -> pl.DataFrame:
        """Player respawn events as a Polars DataFrame.

        Auto-loads on first access if not already loaded via :meth:`load`.

        Columns:
            - **tick** (*int*) -- The game tick when the player respawned.
            - **hero_id** (*int*) -- The hero ID of the respawned player.
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
