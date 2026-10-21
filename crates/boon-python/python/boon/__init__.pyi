"""Typed Boon Python API for Valve Deadlock Source 2 replay data.

Native methods use PyO3. Dataset and analysis results use Polars DataFrames.
Public call contracts use Google-style docstrings.
"""

from collections.abc import Mapping, Sequence
from pathlib import Path

import polars as pl

from . import data as data
from .ability_stats import AbilityStat as AbilityStat
from .ability_stats import AbilityStatResult as AbilityStatResult
from .ability_stats import ImbueResult as ImbueResult
from .hero_stats import CalculationError as CalculationError
from .hero_stats import HeroStat as HeroStat
from .hero_stats import StatMode as StatMode
from .hero_stats import StatResult as StatResult
from .rulesets import Rule

__version__: str

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

def hero_names(version: str | None = None) -> dict[int, str]:
    """Map Deadlock hero IDs to localized names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict mapping hero IDs to display names. Internal names are used when localization is missing.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    ...

def team_names() -> dict[int, str]:
    """Return a mapping of team number to team name.

    Returns:
        dict[int, str]: A new dictionary of recorded enum IDs and readable names.
    """
    ...

def ability_names(version: str | None = None) -> dict[int, str]:
    """Map Deadlock ability and item IDs to internal names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict mapping MurmurHash2 IDs to exact internal ability or item names.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    ...

def ability_display_names(version: str | None = None) -> dict[str, str]:
    """Map internal Deadlock ability/item names to English display names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict keyed by exact internal names. Entries without localization are omitted.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    ...

def modifier_names(version: str | None = None) -> dict[int, str]:
    """Map unqualified and owner-qualified modifier IDs to names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict containing both supported modifier-ID forms.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.

    Notes:
        Duplicate definitions with the same ID and name share one entry.
        Conflicting names raise DataError.
    """
    ...

def breakable_names(version: str | None = None) -> dict[int, str]:
    """Map breakable subclass IDs to internal names in misc.json.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict of records whose definition._class is citadel_breakable_prop.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """

def game_mode_names() -> dict[int, str]:
    """Return a mapping of game mode ID to game mode name.

    Returns:
        dict[int, str]: A new dictionary of recorded enum IDs and readable names.
    """
    ...

def patron_phase_names() -> dict[int, str]:
    """Return a mapping of patron phase ID to phase name.

    Returns:
        dict[int, str]: A new dictionary of recorded enum IDs and readable names.

    Notes:
        Phases of ``CNPC_Boss_Tier3.m_ePhase``: ``0=normal`` (shielded),
        ``1=final`` (killable), ``2=transforming`` (vulnerable). Non-patron
        objectives report ``0`` by default.
    """
    ...

def hitgroup_names() -> dict[int, str]:
    """Return a mapping of hit group ID to hit group name.

    Returns:
        dict[int, str]: A new dictionary of recorded enum IDs and readable names.

    Notes:
        Hit group IDs are Source 2's ``HitGroup_t`` enum values, used by the
        ``hitgroup_id`` column on the ``damage`` frame (``0=generic``, ``1=head``,
        ``2=chest`` ... ``19=head_no_resist``; ``-1=invalid``).
    """
    ...

def lifestate_names() -> dict[int, str]:
    """Return a mapping of life state ID to life state name.

    Returns:
        dict[int, str]: A new dictionary of recorded enum IDs and readable names.

    Notes:
        Life state IDs are Source 2's ``LifeState_t`` enum values, used by the
        ``lifestate`` column on ``player_ticks`` (``0=alive``, ``1=dying``,
        ``2=dead``, ``3=respawnable``, ``4=respawning``).
    """
    ...

class Demo:
    """Read Valve Deadlock Source 2 replays with the Boon Rust parser.

    Args:
        path: String path to a local .dem replay.
        preload: Preload kills, damage, and abilities when True. Other datasets load on access.

    Notes:
        Python dataset results are Polars DataFrames. Use Steam IDs for player
        joins. Only name, game-data, and image queries can download boon-data.
        The Rust parser is exposed through PyO3.

    Examples:
        >>> from boon import Demo
        >>> demo = Demo("match.dem", preload=False)
        >>> print(demo.players)
        >>> print(demo.kills.head())
    """

    def __init__(self, path: str, *, preload: bool = True) -> None:
        """Open one Valve Deadlock Source 2 replay with Boon.

        Args:
            path: String path to a local .dem file.
            preload: Load kills, damage, and abilities during construction when True. False keeps dataset loading deferred.

        Returns:
            None.

        Raises:
            FileNotFoundError: The file does not exist.
            InvalidDemoError: The file is not a supported replay.
            DemoHeaderError: Required header fields are absent.
            DemoInfoError: Required file-info fields are absent.
        """
        ...
    def verify(self) -> bool:
        """Verify that the file is a valid demo file.

        Returns:
            True if the file is valid.

        Notes:
            This is already called during construction, so it will always
            return True for an existing Demo instance.
        """
        ...

    @staticmethod
    def available_datasets() -> list[str]:
        """Return the list of dataset names that can be passed to ``load()`` or accessed as properties.

        Returns:
            A list of valid dataset name strings.

        Examples:
            >>> Demo.available_datasets()
            ['abilities', 'ability_upgrades', 'chat', ...]
        """
        ...

    def load(self, *datasets: str) -> None:
        """Load and cache selected Boon datasets using compatible parser passes.

        Args:
            *datasets: One or more names returned by Demo.available_datasets().

        Returns:
            None. Read the cached frames through the matching Demo properties.

        Raises:
            ValueError: A dataset name is unknown.
            NotStreetBrawlError: A Street Brawl dataset is requested for another mode.
            InvalidDemoError: Requested replay state cannot be decoded.

        Notes:
            Compatible events share filtered passes. Player/world/trooper snapshots
            use parallel segments. Zipline tables share their own cached serial
            history. Previously loaded frames are reused.

        Examples:
            >>> from boon import Demo
            >>> demo = Demo("match.dem", preload=False)
            >>> demo.load("kills", "damage")
            >>> print(demo.kills)
        """
        ...

    @property
    def path(self) -> Path:
        """The path to the demo file.

        Returns:
            Path: The path to the demo file.
        """
        ...

    @property
    def total_ticks(self) -> int:
        """The total number of ticks in the demo.

        Returns:
            int: The total number of ticks in the demo.
        """
        ...

    @property
    def total_seconds(self) -> float:
        """The total duration of the demo in seconds.

        Returns:
            float: The total duration of the demo in seconds.
        """
        ...

    @property
    def total_clock_time(self) -> str:
        """The total duration of the demo as a formatted string (for example, ``"12:34"``).

        Returns:
            str: The total duration of the demo as a formatted string (for example, ``"12:34"``).
        """
        ...

    @property
    def build(self) -> int:
        """The build number of the game that recorded the demo.

        Returns:
            int: The build number of the game that recorded the demo.
        """
        ...

    @property
    def map_name(self) -> str:
        """The name of the map the demo was recorded on.

        Returns:
            str: The name of the map the demo was recorded on.
        """
        ...

    @property
    def match_id(self) -> int | None:
        """Read the match ID recorded in the replay.

        Returns:
            Match ID, or None when the replay omits it.

        Notes:
            Partial captures and custom demos can omit the match ID.
        """
        ...

    @property
    def game_mode(self) -> int:
        """The game mode ID for this demo.

        Returns:
            int: The game mode ID for this demo.

        Notes:
            Use ``game_mode_names()`` to resolve IDs to names.
        """
        ...

    @property
    def tick_rate(self) -> int:
        """The tick rate of the demo (ticks per second).

        Returns:
            int: The tick rate of the demo (ticks per second).
        """
        ...

    def summary(self) -> dict[str, pl.DataFrame]:
        """Read the recorded Deadlock PostMatchDetails message.

        Returns:
            Dictionary of six cached Polars DataFrames:

            - ``snapshots``: cumulative player counters and state at ``snapshot_time_s``.
              Includes ``steam_id``, ``hero_id``, damage by target type, damage taken,
              ``player_healing``, ``teammate_healing``, and ``self_healing``.
              ``barrier_absorption`` is damage stopped by barriers this player provided
              (recorded ``player_barriering``). ``damage_absorbed`` is damage stopped
              by barriers on this player. Healing and barrier absorption are separate.
              Added counters are null when absent.
            - ``gold_sources``: cumulative ``gold``, ``gold_orbs``, ``kills``, and ``damage``
              for each player, snapshot, and source. Includes ``source_id`` and the protobuf
              ``source_name``. Unknown IDs and absent counters remain available.
            - ``last_hits``: final ``hero_id`` and ``last_hits`` totals.
            - ``objectives``: recorded objective times and damage.
            - ``damage``: the source-to-target matrix at each ``sample_time_s``.
              ``damage`` is the interval amount; ``total`` is the recorded cumulative value.
              The interval starts at ``interval_start_s``. Select ``stat_type`` for damage,
              healing, regeneration, or another recorded statistic. Category rows
              (``is_category=True``) duplicate specific sources; do not add them together.
            - ``healing``: healing and regeneration rows without category duplicates.
              Columns: ``interval_start_s``, ``interval_end_s``, ``healer_steam_id``, ``healer_hero_id``,
              ``target_steam_id``, ``target_hero_id``, ``source_name``,
              ``stat_type``, ``amount``, and ``total``. ``amount`` is the interval amount.
              ``total`` is the recorded cumulative amount. Zero changes remain in the table.

            Times use match-clock seconds. Snapshot and matrix reporting periods can differ.
            Do not sum cumulative totals across periods. Use Steam IDs across hero changes;
            hero IDs come from the match roster. These tables do not contain individual heals.

        Raises:
            DemoMessageError: The post-match message is absent or invalid.
        """
        ...

    def snapshots(
        self,
        /,
        datasets: str | list[str] | None = ...,
        *,
        ticks: int | list[int] | None = ...,
        every: int | None = ...,
        seconds: float | None = ...,
        events: str | list[str] | None = ...,
        start_tick: int | None = ...,
        end_tick: int | None = ...,
        data_version: str | None = ...,
    ) -> pl.DataFrame | dict[str, pl.DataFrame]:
        """Read selected Deadlock ticks as Polars DataFrames.

        Args:
            datasets: One name or a list: player_ticks (default), world_ticks, troopers, or ziplines.
            ticks: One exact demo tick or a list of ticks.
            every: Tick stride measured from the recording start. Cannot be combined with seconds.
            seconds: Sample interval in seconds, converted to a tick stride. Paused samples can be included.
            events: Event dataset name or names. Select their recorded tick values.
            start_tick: Inclusive lower tick limit.
            end_tick: Inclusive upper tick limit.
            data_version: Optional boon-data version for calculated ammo in player_ticks. Requires player_ticks and downloads missing catalogs.

        Returns:
            One Polars DataFrame for a single dataset, or a dict keyed by dataset
            name for several datasets. Player rows include steam_id, hero_id, and
            recorded ammo_fraction. Unknown values stay null.

        Raises:
            ValueError: A dataset, selector, window, or data_version request is invalid.
            InvalidDemoError: The selected replay data cannot be decoded.
            data.DataError: Requested ammo catalogs cannot be downloaded or read.

        Notes:
            Supply a selector or tick window. A window alone returns every recorded
            tick inside it. Player, world, and trooper queries use direct seeks or
            parallel segments. Zipline queries reuse one cached serial history.
            With data_version, ammo is fraction * finite capacity, rounded to the
            nearest integer with halves rounded up. unlimited_ammo reads INFINITE_CLIP
            from the predicted-state mask and does not change capacity. Missing or
            duplicate Steam IDs leave calculated values null. Keep ammo_status and
            ammo_diagnostic with results. No catalog is required without data_version.

        Examples:
            >>> from boon import Demo
            >>> demo = Demo("match.dem", preload=False)
            >>> frames = demo.snapshots(["player_ticks", "ziplines"], ticks=10000)
            >>> print(frames["ziplines"])
        """
        ...

    def _snapshots(
        self,
        /,
        datasets: str | list[str] | None = ...,
        *,
        ticks: int | list[int] | None = ...,
        every: int | None = ...,
        seconds: float | None = ...,
        events: str | list[str] | None = ...,
        start_tick: int | None = ...,
        end_tick: int | None = ...,
    ) -> pl.DataFrame | dict[str, pl.DataFrame]:
        """Read native sampled state before optional Python ammo calculation.

        Args:
            datasets: Snapshot dataset name or names.
            ticks: Exact demo ticks to read.
            every: Positive tick stride.
            seconds: Sampling interval in seconds.
            events: Dataset name or names whose event ticks should be sampled.
            start_tick: Inclusive lower tick bound.
            end_tick: Inclusive upper tick bound.

        Returns:
            pl.DataFrame | dict[str, pl.DataFrame]: Native results used by the public Python wrapper.
        """
        ...
    def imbues(
        self,
        /,
        *,
        ticks: int | Sequence[int],
        data_version: str,
        steam_ids: Sequence[int] | None = None,
    ) -> ImbueResult:
        """Read recorded item imbues and their catalog effects at exact ticks.

        Args:
            ticks: One nonnegative demo tick or a sequence of exact ticks.
            data_version: Explicit Deadlock client version from `boon versions`. Download and verify it if missing.
            steam_ids: Steam account IDs from `demo.players`. None selects all players; an empty sequence selects none.

        Returns:
            ImbueResult with bindings, individual effects, and metadata.
            Missing catalog entries do not remove recorded bindings.

        Raises:
            ValueError: The version or tick/account selection is invalid.
            CalculationError: A requested tick or player cannot be resolved.
            data.DataError: The selected catalog cannot be downloaded or read.

        Notes:
            Effects keep their targeting filters. This method does not stack bonuses
            or return final cast values. Use calculate_ability_stats for temporary
            next-cast effects.

        Examples:
            >>> from boon import Demo
            >>> demo = Demo("match.dem", preload=False)
            >>> result = demo.imbues(ticks=10000, data_version="6712")
            >>> print(result.bindings)
        """
        ...
    def calculate_ability_stats(
        self,
        /,
        *,
        ticks: int | Sequence[int],
        data_version: str,
        stats: Sequence[AbilityStat | str] = ...,
        mode: StatMode | str = ...,
        steam_ids: Sequence[int] | None = None,
        abilities: Sequence[int] | None = None,
        include_items: bool = False,
        rulesets: Mapping[AbilityStat | str, Rule] | None = None,
        explain: bool = False,
        strict: bool = True,
    ) -> AbilityStatResult:
        """Calculate bonus percentages for each selected Deadlock ability.

        Args:
            ticks: One nonnegative demo tick or a sequence of exact ticks.
            data_version: Explicit Deadlock client version from `boon versions`. Download and verify it if missing.
            stats: AbilityStat members or matching strings. Defaults to cooldown_reduction, duration_bonus, range_bonus, and radius_bonus.
            mode: `current` or StatMode.CURRENT includes supported active effects. `baseline` or StatMode.BASELINE selects passive and permanent inputs.
            steam_ids: Steam account IDs from `demo.players`. None selects all players; an empty sequence selects none.
            abilities: Owned ability or item IDs. None selects signature abilities and, if requested, owned items.
            include_items: Include all owned items when True. Item cooldown reduction remains separate from hero ability cooldown reduction.
            rulesets: One supported V1 Rule for each selected stat. None uses the built-in V1 rules.
            explain: Include source scope, activation, and inclusion details in contributions.
            strict: Raise CalculationError for missing inputs when True. False returns unresolved rows with null values.

        Returns:
            AbilityStatResult with percentage values, source contributions, and metadata.
            Range and radius are separate bonuses. Results are not seconds or meters.

        Raises:
            ValueError: A selection, mode, stat name, or ruleset is invalid.
            CalculationError: A requested tick/player is absent, or required inputs are missing with strict=True.
            data.DataError: The selected catalog cannot be downloaded, verified, or read.

        Notes:
            V1 uses 100 * (1 - product(1 - bonus / 100)). Each source is counted once.
            Catalog filters restrict effects to their selected abilities. Current mode
            can include supported next-cast effects. Baseline excludes temporary
            effects. Unsupported activation produces partial values and diagnostics.

        Examples:
            >>> from boon import AbilityStat, Demo
            >>> demo = Demo("match.dem", preload=False)
            >>> result = demo.calculate_ability_stats(
            ...     ticks=10000, data_version="6712",
            ...     stats=[AbilityStat.COOLDOWN_REDUCTION], strict=False,
            ... )
            >>> print(result.values)
        """
        ...
    def _imbues(
        self,
        directory: Path,
        ticks: list[int],
        *,
        steam_ids: Sequence[int] | None = None,
    ) -> str:
        """Serialize recorded imbue rows using a local catalog.

        Args:
            directory: Verified local boon-data directory.
            ticks: Exact demo ticks to read.
            steam_ids: Optional selected Steam account IDs.

        Returns:
            str: Native results used by the public Python wrapper.
        """
        ...
    def _calculate_ability_stats(
        self,
        directory: Path,
        ticks: list[int],
        *,
        stats: list[str],
        mode: str = ...,
        steam_ids: Sequence[int] | None = None,
        abilities: Sequence[int] | None = None,
        include_items: bool = False,
        explain: bool = False,
        strict: bool = True,
    ) -> str:
        """Serialize native ability bonus results using a local catalog.

        Args:
            directory: Verified local boon-data directory.
            ticks: Exact demo ticks to read.
            stats: Exact selected stat strings.
            mode: current or baseline input mode.
            steam_ids: Optional selected Steam account IDs.
            abilities: Optional selected owned ability IDs.
            include_items: Include owned items when True.
            explain: Include source contributions when True.
            strict: Raise for missing calculation inputs when True.

        Returns:
            str: Native results used by the public Python wrapper.
        """
        ...
    def player_states(
        self,
        /,
        *,
        data_version: str,
        ticks: int | Sequence[int] | None = None,
        steam_ids: Sequence[int] | None = None,
    ) -> pl.DataFrame:
        """Read recorded Deadlock state names from a selected boon-data catalog.

        Args:
            data_version: Explicit Deadlock client version from `boon versions`. Download and verify it if missing.
            ticks: One exact tick, a sequence of ticks, or None for every recorded tick. An empty sequence returns no rows.
            steam_ids: Steam account IDs from `demo.players`. None selects all players; an empty sequence selects none.

        Returns:
            Polars DataFrame with tick, steam_id, hero_id, three state-name lists,
            and the corresponding unknown-bit lists. Missing masks and IDs stay null.

        Raises:
            ValueError: A version, tick, or account selection is invalid, or a requested tick is absent.
            data.DataError: The selected state catalog cannot be downloaded or read.

        Notes:
            Names are sorted and omit MODIFIER_STATE_. The states column uses the
            recorded predicted-state mask; Boon does not predict player states.
            Enabled and disabled masks remain separate. No mask priority is inferred.
            An empty name list can still have unknown bits. Read unknown_states,
            unknown_enabled_states, and unknown_disabled_states before assuming no state.

        Examples:
            >>> from boon import Demo
            >>> demo = Demo("match.dem", preload=False)
            >>> states = demo.player_states(ticks=10000, data_version="6712")
            >>> print(states.select("steam_id", "hero_id", "states"))
        """
        ...
    def _player_states(
        self,
        directory: Path,
        *,
        ticks: Sequence[int] | None = None,
        steam_ids: Sequence[int] | None = None,
    ) -> pl.DataFrame:
        """Read named state masks using a local catalog.

        Args:
            directory: Verified local boon-data directory.
            ticks: Exact demo ticks to read.
            steam_ids: Optional selected Steam account IDs.

        Returns:
            pl.DataFrame: Native results used by the public Python wrapper.
        """
        ...
    def calculate_hero_stats(
        self,
        /,
        *,
        ticks: int | Sequence[int],
        data_version: str,
        stats: Sequence[HeroStat | str] = ...,
        mode: StatMode | str = ...,
        steam_ids: Sequence[int] | None = ...,
        heroes: Sequence[int] | None = ...,
        rulesets: Mapping[HeroStat | str, Rule] | None = ...,
        explain: bool = ...,
        strict: bool = ...,
    ) -> StatResult:
        """Calculate selected Deadlock hero stats after each exact tick.

        Args:
            ticks: One nonnegative demo tick or a sequence of ticks below 2147483647.
            data_version: Explicit Deadlock client version from `boon versions`. Download and verify it if missing.
            stats: HeroStat members or their exact string values. The default is `clip_size` (ammo capacity).
            mode: `current` or StatMode.CURRENT includes supported active effects. `baseline` or StatMode.BASELINE selects passive and permanent inputs.
            steam_ids: Steam account IDs from `demo.players`. None selects all players; an empty sequence selects none.
            heroes: Current hero IDs to select. If steam_ids is also set, both filters apply.
            rulesets: One supported Rule for each selected stat. None selects boon.rulesets.<stat>.v1.
            explain: Include source values and catalog paths in contributions when True.
            strict: Raise CalculationError for missing inputs when True. False keeps unresolved rows with null values.

        Returns:
            StatResult with Polars values, contributions, and metadata.
            Keep status and diagnostic with exported values. A partial value is a known subtotal.

        Raises:
            ValueError: A selection, mode, stat name, or ruleset is invalid.
            CalculationError: A requested tick/player is absent, or required inputs are missing with strict=True.
            data.DataError: The selected catalog cannot be downloaded, verified, or read.

        Notes:
            Use tick and steam_id for player joins. Missing Steam IDs stay null.
            Fire rate and weapon damage are bonus percentages. Clip size stays finite
            during unlimited ammo. Move speed and sprint speed are nominal values;
            they do not simulate crouching, sprint acceleration, or bullet-hit slows.
            Baseline gravity scale is unavailable. Units and equations do not change by mode.

        Examples:
            >>> from boon import Demo, HeroStat
            >>> demo = Demo("match.dem", preload=False)
            >>> result = demo.calculate_hero_stats(
            ...     ticks=10000, data_version="6712",
            ...     stats=[HeroStat.CLIP_SIZE, HeroStat.FIRE_RATE], strict=False,
            ... )
            >>> print(result.values)
        """
        ...
    def _calculate_hero_stats(
        self,
        directory: Path,
        ticks: list[int],
        *,
        stats: list[str],
        mode: str = ...,
        steam_ids: list[int] | None = ...,
        heroes: list[int] | None = ...,
        explain: bool = ...,
        strict: bool = ...,
    ) -> str:
        """Serialize native hero-stat results using a local catalog.

        Args:
            directory: Verified local boon-data directory.
            ticks: Exact demo ticks to read.
            stats: Exact selected stat strings.
            mode: current or baseline input mode.
            steam_ids: Optional selected Steam account IDs.
            heroes: Optional selected current hero IDs.
            explain: Include source contributions when True.
            strict: Raise for missing calculation inputs when True.

        Returns:
            str: Native results used by the public Python wrapper.
        """
        ...
    def _player_positions(self, ticks: list[int]) -> pl.DataFrame:
        """Read selected player coordinates without a full player-tick frame.

        Args:
            ticks: Exact demo ticks to read.

        Returns:
            pl.DataFrame: Native results used by the public Python wrapper.
        """
        ...
    def in_combat(self, /) -> pl.DataFrame:
        """Report the combat state of each player for each tick.

        Deadlock stores combat state as a time window on the player pawn. The player
        is in combat while the current game time is less than
        ``in_combat_end_time``. Each hit sets the end time to the last damage time
        plus a delay. The delay is approximately 0.5 seconds for NPC damage and 3.0
        seconds for hero damage. This function calculates the state from
        ``player_ticks``.

        The comparison requires the current game time for each tick. Calculate this
        time from non-paused ticks, as `Demo.tick_to_seconds` does. Then add one
        constant offset between the demo clock and the engine clock. At a damage
        tick, ``last_damage_time`` equals the current engine time. It cannot be later
        than the current engine time. Thus, the maximum value of
        ``last_damage_time - elapsed_seconds`` is the clock offset.

        Returns:
            A Polars DataFrame with one row for each ``(tick, hero_id)``. Rows are
            sorted by ``tick`` and then ``hero_id``. The frame has these columns:

            - ``tick`` (*int*) -- The game tick.
            - ``hero_id`` (*int*) -- The player's hero ID.
            - ``in_combat`` (*bool*) -- True when the player is in combat.
        """
        ...

    def kill_participation(
        self, /, *, start_tick: int | None = ..., end_tick: int | None = ...
    ) -> pl.DataFrame:
        """Kill participation per player: ``(kills + assists) / team_kills``.

        A player participates in a team kill when they are credited as either the
        killer or an assister (never both on the same kill), so the value is a
        fraction in ``[0, 1]`` — the share of their team's kills they were involved
        in.

        Args:
            start_tick: If given, only count kills at or after this tick.
            end_tick: If given, only count kills at or before this tick.

        Returns:
            A Polars DataFrame with one row per player on the roster, sorted by
            ``team_num`` then ``hero_id``, with columns:

            - ``hero_id`` (*int*) -- The player's hero ID.
            - ``team_num`` (*int*) -- The player's team number.
            - ``kills`` (*int*) -- Kills credited to the player (in the window).
            - ``assists`` (*int*) -- Assists credited to the player (in the window).
            - ``team_kills`` (*int*) -- Total kills by the player's team (in the window).
            - ``kill_participation`` (*float*) -- ``(kills + assists) / team_kills``,
              or null when the team had zero kills in the window.
        """
        ...

    def time_dead(self, /) -> pl.DataFrame:
        """Time each player spent dead during regulation play.

        A player is counted as dead on any tick where they are not alive
        (``is_alive == False``). Only non-paused ticks up to the game-over event are
        counted, so the totals line up with ``demo.regulation_ticks`` /
        ``demo.regulation_seconds`` (the active, paused-time-excluded duration of
        regulation play).

        Returns:
            A Polars DataFrame with one row per player on the roster, sorted by
            ``team_num`` then ``hero_id``, with columns:

            - ``hero_id`` (*int*) -- The player's hero ID.
            - ``team_num`` (*int*) -- The player's team number.
            - ``ticks_dead`` (*int*) -- Non-paused regulation ticks spent dead.
            - ``seconds_dead`` (*float*) -- ``ticks_dead / tick_rate``.
            - ``pct_regulation_dead`` (*float*) -- ``ticks_dead / regulation_ticks``
              as a percentage in ``[0, 100]``.

        Raises:
            ValueError: If the demo has no game-over event, in which case regulation
                time (and therefore this metric) is undefined.
        """
        ...

    def teamfights(
        self,
        /,
        *,
        gap_seconds: float = ...,
        radius: float = ...,
        min_players: int = ...,
    ) -> pl.DataFrame:
        """Detect teamfights from hero damage in a local area and time window.

        A teamfight does not require a kill. It can start before the first kill and
        can end with all players alive. Use damage between heroes on opposing teams.
        Combine events that are close in time and location. This rule keeps fights
        in different lanes separate.

        Use the victim position for each event. Use the attacker position when the
        victim position is not available. Process events in tick order. An event
        joins the nearest active fight when its distance from the fight center is
        less than ``radius``. The last event must also be less than ``gap_seconds``
        earlier. Otherwise, the event starts a new fight. A fight becomes inactive
        after ``gap_seconds`` without a nearby event. Drop fights that have fewer
        than ``min_players`` different heroes.

        Args:
            gap_seconds: Maximum time without a nearby event before a fight ends.
            radius: Maximum map distance between an event and the current fight
                center. The default is for Deadlock coordinates.
            min_players: Minimum number of different heroes that deal or take hero
                damage in a fight.

        Returns:
            A Polars DataFrame with one row per teamfight, sorted by ``start_tick``:

            - ``fight_id`` (*int*) -- Sequential fight number (1-indexed).
            - ``start_tick`` / ``end_tick`` (*int*) -- First and last damage tick.
            - ``start_seconds`` / ``end_seconds`` (*float*) -- Start and end times.
              The values exclude paused time and use `Demo.tick_to_seconds`.
            - ``duration_seconds`` (*float*) -- ``end_seconds - start_seconds``.
            - ``center_x`` / ``center_y`` (*float*) -- Mean event position.
            - ``participants`` (*list[int]*) -- Hero IDs that dealt or took hero
              damage in the fight (sorted).
            - ``num_participants`` (*int*) -- ``len(participants)``.
            - ``hero_damage`` (*int*) -- Total hero-vs-hero damage dealt in the fight.
            - ``kills`` (*int*) -- Hero kills in the fight time window.

        Raises:
            ValueError: If the demo's tick rate is 0 (cannot cluster by time).
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

    def tick_to_match_seconds(self, tick: int) -> float | None:
        """Convert a tick to on-screen match-clock seconds, excluding paused time.

        ``0.0`` at ``game_start_tick``; negative during the pre-game lobby (matching
        the spectator clock's pre-game countdown). Equal to
        ``tick_to_seconds(tick) - pregame_seconds``.

        Args:
            tick: The game tick to convert.

        Returns:
            Match-clock seconds, or ``None`` if ``pregame_seconds`` is unavailable.
        """
        ...

    def tick_to_match_clock(self, tick: int) -> str | None:
        """Convert a tick to an on-screen match-clock string (for example, ``"3:14"``).

        Args:
            tick: Nonnegative demo tick to convert.

        Returns:
            str | None: Convert a tick to an on-screen match-clock string (for example, ``"3:14"``).

        Notes:
            The formatted counterpart to ``tick_to_match_seconds``; pre-game ticks read as
            a negative clock (for example, ``"-0:12"``). Returns ``None`` if
            ``pregame_seconds`` is unavailable.
        """
        ...

    @property
    def winning_team_num(self) -> int | None:
        """The team number of the winning team, or ``None`` if no game-over event was found.

        Returns:
            int | None: The team number of the winning team, or ``None`` if no game-over event was found.
        """
        ...

    @property
    def game_over_tick(self) -> int | None:
        """The tick when the game ended, or ``None`` if no game-over event was found.

        Returns:
            int | None: The tick when the game ended, or ``None`` if no game-over event was found.
        """
        ...

    @property
    def pregame_seconds(self) -> float | None:
        """The pre-game lobby duration in seconds.

        Returns:
            float | None: The pre-game lobby duration in seconds.

        Notes:
            The demo starts recording during the pre-game, so tick 0 leads the on-screen
            match clock (which reaches ``0:00`` only when the barrier drops and the game
            begins) by this amount: ``match clock = tick_to_seconds - pregame_seconds``.
            Derived from the replicated match clock at game over, not a heuristic. Returns
            ``None`` if the demo has no game-over event, does not replicate the match clock
            (older builds), or does not start in the pre-game (a recording that begins after
            the barrier drop, where the offset would be negative).
        """
        ...

    @property
    def game_start_tick(self) -> int | None:
        """Read the demo tick at match start.

        Returns:
            Tick at match clock 0:00, or None when pregame_seconds is unavailable.

        Notes:
            This is the end of the pregame lobby, when the starting barriers drop.
        """
        ...

    @property
    def regulation_ticks(self) -> int | None:
        """Match-clock ticks at game over.

        Returns:
            int | None: Match-clock ticks at game over.

        Notes:
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

        Returns:
            float | None: HUD match-clock value at game over, in seconds.

        Notes:
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

        Returns:
            str | None: Match-clock value at game over (for example, ``"32:45"``).

        Notes:
            This is the formatted counterpart to ``regulation_seconds``. Returns
            ``None`` if no game-over event was found.
        """
        ...

    @property
    def players(self) -> pl.DataFrame:
        """Player information as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

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

        Returns:
            Polars DataFrame with these columns:

            - **hero_id** (*int*) -- The banned hero's ID (joins to ``players.hero_id``).
            - **hero_name** (*str*) -- The resolved hero name, or ``"HERO_NOT_FOUND"`` for an ID absent from the selected boon-data catalog.

        Notes:
            Read from the single ``BannedHeroes`` user message, which the server
            sends early in the demo (before the match starts) only when the match
            has bans. The message carries nothing but the hero IDs -- no team, no
            banning player, and no pick/ban ordering -- so this cannot be used to
            build a draft.

            An empty DataFrame means no bans were recorded for this match. Demos
            from builds that never emit the message are indistinguishable from
            ban-free matches, so treat empty as "nothing recorded" rather than as
            positive proof that nothing was banned.
        """
        ...

    def enemy_minimap_ticks(
        self, *, ticks: Sequence[int] | None = None, every: int | None = None
    ) -> pl.DataFrame:
        """Team-specific enemy hero minimap observations from replay FOW state.

        Args:
            ticks: Exact tick sequence. An empty sequence returns no rows. Cannot be combined with every.
            every: Positive tick interval. Omit both selectors for all ticks.

        Returns:
            Polars DataFrame with observer team, target identity, recorded visibility,
            native 0–255 map coordinates, and per-pawn last-seen values.

        Raises:
            ValueError: Ticks or stride are invalid or both selectors are supplied.

        Notes:
            `observer_team` identifies the team's shared map channel. `visible_on_map`
            is directly recorded; `map_x` and `map_y` are native byte coordinates
            only for shown icons. `last_seen_*` is carried from the last shown row
            of the same resolved pawn lifetime. Unbound rows retain their raw FOW
            entity index and leave identity and last-seen columns null.
            `target_steam_id` is present only when one active controller with a
            recorded Steam ID points to that exact pawn lifetime.
            Omit both arguments for all ticks. Pass either exact ``ticks`` or a
            positive ``every`` interval. An empty tick list returns an empty frame.
        """
        ...

    @property
    def player_ticks(self) -> pl.DataFrame:
        """Per-tick, per-player state as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick.
            - **steam_id** (*int, nullable*) -- Steam account ID (UInt64).
            - **hero_id** (*int*) -- The player's hero ID.
            - **boon_count** (*int, nullable*) -- Directly recorded boon count, when present in the replay schema.
            - **upgrade_ids** (*list[int], nullable*) -- Complete current controller upgrade-token vector; classify item tokens with matching game data. Null means unavailable or undecodable.
            - **ammo_fraction** (*float, nullable*) -- Recorded primary-gun ammo fraction.
                  Use snapshots(data_version=...) to calculate ammo and max_ammo.
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

        Notes:
            Boon loads this dataset on first access.
            Returns a DataFrame with one row per player per tick, containing
            position, health, combat timers, kills, deaths, net worth, and more.
            Rows where the pawn is not found or ``hero_id == 0`` are skipped.
        """
        ...

    @property
    def world_ticks(self) -> pl.DataFrame:
        """World state at every tick as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick.
            - **is_paused** (*bool*) -- Whether the game is paused.
            - **next_midboss** (*float*) -- Time until next midboss spawn.

        Notes:
            Boon loads this dataset on first access.
        """
        ...

    @property
    def kills(self) -> pl.DataFrame:
        """Hero kill events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the kill occurred.
            - **victim_hero_id** (*int*) -- The hero ID of the killed player.
            - **attacker_hero_id** (*int*) -- The hero ID of the attacker.
            - **assister_hero_ids** (*list[int]*) -- List of hero IDs of players who assisted.

        Notes:
            Boon preloads this dataset unless ``preload=False``.
            With preloading disabled, the first access loads and caches the data.
        """
        ...

    @property
    def damage(self) -> pl.DataFrame:
        """Damage events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The enclosing demo command tick.
            - **damage** (*int*) -- The damage dealt.
            - **pre_damage** (*float*) -- The damage before mitigation.
            - **damage_absorbed** (*float | None*) -- Recorded absorption; null when absent.
            - **victim_shield_new** (*int | None*) -- Remaining shield after the hit.
            - **victim_shield_max** (*int | None*) -- Shield capacity.
            - **server_tick** (*int | None*) -- Server tick recorded in the damage message.
            - **victim_hero_id** (*int*) -- The hero ID of the victim (0 if not a hero).
            - **attacker_hero_id** (*int*) -- The hero ID of the attacker (0 if not a hero).
            - **victim_health_new** (*int*) -- The victim's health after damage.
            - **hitgroup_id** (*int*) -- The hitgroup that was hit (use ``hitgroup_names()`` to resolve).
            - **crit_damage** (*float*) -- Critical damage amount.
            - **attacker_class** (*int*) -- The attacker's entity class ID.
            - **victim_class** (*int*) -- The victim's entity class ID.
            - **victim_entity_id** (*int*) -- Victim entity index; -1 when absent.
            - **ability_id** (*int*) -- The ability/weapon responsible, or 0 when absent.
            - **damage_type** (*int*) -- Raw Source ``type`` damage bitfield.
            - **citadel_type** (*int*) -- Deadlock damage category; 3 is melee-typed damage.
            - **damage_flags** (*int*) -- Raw Valve damage flags.
            - **is_melee** (*bool*) -- Whether this is melee-typed damage (``citadel_type == 3``).
            - **melee_type** (*str | None*) -- ``"light"``, ``"heavy"``, or ``"other"`` for melee-typed damage; null otherwise.

        Notes:
            Boon preloads this dataset unless ``preload=False``.
            With preloading disabled, the first access loads and caches the data.
        """
        ...

    @property
    def flex_slots(self) -> pl.DataFrame:
        """Flex slot unlock events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the flex slot was unlocked.
            - **team_num** (*int*) -- The team number that unlocked the flex slot.

        Notes:
            Boon loads this dataset on first access.
        """
        ...

    @property
    def abilities(self) -> pl.DataFrame:
        """Important ability usage events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the ability was used.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability** (*str*) -- The ability name.

        Notes:
            Boon preloads this dataset unless ``preload=False``.
            With preloading disabled, the first access loads and caches the data.
        """
        ...

    @property
    def ability_upgrades(self) -> pl.DataFrame:
        """Hero ability point spending events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the upgrade occurred.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability_id** (*int*) -- The raw MurmurHash2 ability ID.
            - **tier** (*int*) -- Upgrade tier (1, 2, or 3).

        Notes:
            Boon loads this dataset on first access.
        """
        ...

    @property
    def item_purchases(self) -> pl.DataFrame:
        """Item shop transactions as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The tick of the transaction.
            - **steam_id** (*int | None*) -- The player's Steam ID, if recorded.
            - **hero_id** (*int*) -- The hero ID of the player.
            - **ability_id** (*int*) -- The raw MurmurHash2 item/ability ID.
            - **change** (*str*) -- Recorded change: ``"purchased"``, ``"upgraded"``, ``"sold"``, ``"swapped"``, ``"leveled_up"``, ``"failure"``, ``"unknown"``.
            - **upgraded_from_ability_ids** (*list[int]*) -- Catalog components
                  sold by this player at this tick. Empty when no unique match exists.

        Notes:
            Boon loads this dataset on first access. Use the newest installed
            boon-data version, or download latest if none is installed.
        """
        ...

    def get_item_purchases(self, /, *, data_version: str | None = None) -> pl.DataFrame:
        """Read Deadlock item events with catalog-based component links.

        Args:
            data_version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

        Returns:
            Polars DataFrame with recorded event columns and
            upgraded_from_ability_ids. Recorded change strings remain unchanged.

        Raises:
            data.DataError: The selected component catalog cannot be installed or read.

        Notes:
            Only unambiguous purchase/sale pairs for the same Steam ID and tick
            receive component links. Missing accounts or links do not produce inferred
            upgrades. An empty event table needs no catalog.
        """
        ...

    @property
    def _item_purchases(self) -> pl.DataFrame:
        """Read raw item events without catalog component links.

        Returns:
            pl.DataFrame: Native results used by the public Python wrapper.
        """
        ...
    @property
    def chat(self) -> pl.DataFrame:
        """In-game chat messages as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the message was sent.
            - **hero_id** (*int*) -- The hero ID of the sender.
            - **text** (*str*) -- The message text.
            - **chat_type** (*str*) -- ``"all"`` or ``"team"``.

        Notes:
            Boon loads this dataset on first access.
        """
        ...

    @property
    def objectives(self) -> pl.DataFrame:
        """Objective health state changes as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

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
            - **entity_serial** (*int*) -- Entity lifetime serial; pair with entity_id to distinguish slot reuse.

        Notes:
            Emits a row when an objective's health, max_health, or phase changes.
            Boon loads this dataset on first access.
        """
        ...

    @property
    def zipline_nodes(self) -> pl.DataFrame:
        """Initial zipline geometry and later revisions, with full node handles.

        Returns:
            pl.DataFrame: Initial zipline geometry and later revisions, with full node handles.

        Notes:
            Columns: tick, node_handle, lane, x, y, z, connections,
            cumulative_distance, capturable. No catalog is required.
            Connections contain full handles; null differs from an empty vector.
        """
        ...

    @property
    def zipline_changes(self) -> pl.DataFrame:
        """Recorded zipline ownership and visibility transitions.

        Returns:
            pl.DataFrame: Recorded zipline ownership and visibility transitions.

        Notes:
            Columns: tick, node_handle, change, team_num. Change values:
            observed, owner_changed, left_pvs, removed. Missing teams remain null.
            A PVS exit is not a deletion. No timeout or lane pressure is inferred.
        """
        ...

    @property
    def mid_boss(self) -> pl.DataFrame:
        """Mid boss lifecycle events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick.
            - **team_num** (*int*) -- The team involved.
            - **event** (*str*) -- ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.

        Notes:
            Boon loads this dataset on first access.
        """
        ...

    @property
    def rift(self) -> pl.DataFrame:
        """Rift lifecycle as a Polars DataFrame -- one row per Rift.

        Returns:
            Polars DataFrame with these columns:

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

        Notes:
            The Rift is a periodic king-of-the-hill objective (``Koth`` in the game
            files). It is announced, becomes contestable, and then either is captured
            by a team -- which grants that team buffed troopers in the Rift's lane --
            or expires uncaptured.

            Exactly one of ``capture_tick`` / ``expire_tick`` is set per row. Only
            completed Rifts appear: one still live when the demo ends is omitted.

            Boon loads this dataset on first access.
        """
        ...

    @property
    def troopers(self) -> pl.DataFrame:
        """Per-tick alive lane trooper state as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

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

        Notes:
            **Warning:** This is a large dataset (~5M+ rows). Not loaded by default.
            Access this property or call ``load("troopers")`` explicitly.
        """
        ...

    @property
    def neutrals(self) -> pl.DataFrame:
        """Neutral creep state changes as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the state changed.
            - **team_num** (*int*) -- The neutral's team.
            - **health** (*int*) -- Current health.
            - **max_health** (*int*) -- Maximum health.
            - **x** (*float*) -- X position.
            - **y** (*float*) -- Y position.
            - **z** (*float*) -- Z position.
            - **entity_id** (*int*) -- Entity index (stable per neutral across ticks).

        Notes:
            Not loaded by default. Access this property or call ``load("neutrals")`` explicitly.
        """
        ...

    @property
    def breakables(self) -> pl.DataFrame:
        """Breakable map-prop destruction events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

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

        Notes:
            Deadlock represents a broken CCitadel_BreakableProp as an entity
            leaving the PVS. Boon emits the candidate only if the same index and
            serial never reactivate later in the demo, and ignores full-packet
            delete/create replacements.

            Health and lifestate are intentionally omitted because the server
            does not report a final health-zero or dead state.

            Not loaded by default. Access this property or call load("breakables") explicitly.
            Subclass names use boon-data misc.json, downloading latest if no verified
            local data exists. Recorded IDs are preserved even when names are unknown.
        """
        ...

    @property
    def sinners_sacrifice(self) -> pl.DataFrame:
        """Sinner's Sacrifice machine lifecycle and hit events.

        Returns:
            Polars DataFrame with these columns:

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

        Notes:
            Tracks both the standard and Hideout machine entity classes. Damage
            messages provide the exact victim entity, incoming damage, and attacker
            hero. An unmatched entity health decrease is retained as a hit with an
            unresolved attacker. ``health`` is the machine state at the end of the
            tick, so multiple hits in one tick can share the same value.

            Not loaded by default. Access this property or call
            ``load("sinners_sacrifice")`` explicitly.
        """
        ...

    @property
    def stat_modifier_events(self) -> pl.DataFrame:
        """Permanent stat bonus change events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the stat changed.
            - **hero_id** (*int*) -- The player's hero ID.
            - **stat_type** (*str*) -- ``"health"``, ``"spirit_power"``, ``"fire_rate"``, ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``, ``"bullet_resist"``, or ``"spirit_resist"``.
            - **amount** (*float*) -- The signed change from this event.

        Notes:
            Not loaded by default. Access this property or call ``load("stat_modifier_events")`` explicitly.

            Types come from the newest installed boon-data catalog. If no catalog is
            installed, Boon downloads the latest version. Use data for the replay's client.
            Emits a row whenever a recorded stat total changes.
        """
        ...

    @property
    def active_modifiers(self) -> pl.DataFrame:
        """Active buff/debuff modifier events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the modifier event occurred.
            - **hero_id** (*int*) -- The affected player's hero ID.
            - **event** (*str*) -- ``"applied"``, ``"changed"`` (stack count
                  changed while active), or ``"removed"``.
            - **modifier_id** (*int*) -- Raw modifier subclass hash ID.
            - **ability_id** (*int*) -- Raw ability subclass hash ID.
            - **duration** (*float*) -- Modifier duration.
            - **caster_hero_id** (*int*) -- Hero ID of the caster.
            - **stacks** (*int*) -- Number of stacks. On ``removed``, the final count.

        Notes:
            Not loaded by default. Access this property or call ``load("active_modifiers")`` explicitly.
        """
        ...

    @property
    def ability_ticks(self) -> pl.DataFrame:
        """Ability cooldown / charge state changes as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

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

        Notes:
            Change-only: a row is emitted for an ability only on the tick its
            cooldown or charge state changes, keeping the frame compact. One ability
            entity exists per ability a player owns, including innate movement
            abilities (jump, dash, slide, ...) which can be filtered out with ``slot``.

            Not loaded by default. Access this property or call ``load("ability_ticks")`` explicitly.
        """
        ...

    @property
    def urn(self) -> pl.DataFrame:
        """Urn (idol) lifecycle events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick when the event occurred.
            - **event** (*str*) -- ``"picked_up"``, ``"dropped"``, ``"returned"``, ``"delivery_active"``, or ``"delivery_inactive"``.
            - **hero_id** (*int*) -- The hero involved (0 for delivery events).
            - **team_num** (*int*) -- Team of the delivery point (0 for modifier events).
            - **x** (*float*) -- Delivery point X position (0.0 for modifier events).
            - **y** (*float*) -- Delivery point Y position (0.0 for modifier events).
            - **z** (*float*) -- Delivery point Z position (0.0 for modifier events).

        Notes:
            Not loaded by default. Access this property or call ``load("urn")`` explicitly.
        """
        ...

    @property
    def street_brawl_ticks(self) -> pl.DataFrame:
        """Per-tick street brawl state as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **tick** (*int*) -- The game tick.
            - **round** (*int*) -- Current round number.
            - **state** (*int*) -- Street brawl state enum value.
            - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) score.
            - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) score.
            - **buy_countdown** (*int*) -- Last buy phase countdown value.
            - **next_state_time** (*float*) -- Time of next state transition.
            - **state_start_time** (*float*) -- Time the current state started.
            - **non_combat_time** (*float*) -- Total non-combat time elapsed.

        Raises:
            NotStreetBrawlError: The demo is not a Street Brawl match.

        Notes:
            Only available for street brawl demos (game_mode=4).
            Boon loads this dataset on first access.
        """
        ...

    @property
    def street_brawl_rounds(self) -> pl.DataFrame:
        """Street brawl round scoring events as a Polars DataFrame.

        Returns:
            Polars DataFrame with these columns:

            - **round** (*int*) -- Sequential round number (1-indexed).
            - **tick** (*int*) -- The game tick when the round ended.
            - **scoring_team** (*int*) -- The team that scored.
            - **amber_score** (*int*) -- The Hidden King (old name: Amber Hand) cumulative score.
            - **sapphire_score** (*int*) -- The Archmother (old name: Sapphire Flame) cumulative score.

        Raises:
            NotStreetBrawlError: The demo is not a Street Brawl match.

        Notes:
            Only available for street brawl demos (game_mode=4).
            Boon loads this dataset on first access.
        """
        ...
