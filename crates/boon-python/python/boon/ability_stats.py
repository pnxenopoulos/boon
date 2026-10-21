"""Recorded imbues and ability-scoped percentage bonuses."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from enum import StrEnum
from typing import TYPE_CHECKING

import polars as pl

from boon import data
from boon import rulesets as builtin_rulesets
from boon._selection import select_ticks, validate_steam_ids, validate_version
from boon.hero_stats import CalculationError, StatMode
from boon.rulesets import Rule

if TYPE_CHECKING:
    from boon import Demo


class AbilityStat(StrEnum):
    """Ability bonus names accepted by Boon calculation queries.

    Use the enum members or their exact string values. Values are percentages,
    not durations in seconds, distances in meters, or remaining cooldown timers.
    """

    COOLDOWN_REDUCTION = "cooldown_reduction"
    ITEM_COOLDOWN_REDUCTION = "item_cooldown_reduction"
    DURATION_BONUS = "duration_bonus"
    RANGE_BONUS = "range_bonus"
    RADIUS_BONUS = "radius_bonus"


@dataclass(frozen=True)
class ImbueResult:
    """Recorded item selections and their individual catalog effects.

    Attributes:
        bindings: Polars rows with tick, steam_id, hero_id, item ID, selected ability ID, status, and diagnostic.
        effects: Individual effect values, units, targeting filters, and source paths. Values are not combined.
        metadata: Selected client version and catalog source details.
    """

    bindings: pl.DataFrame
    effects: pl.DataFrame
    metadata: dict


@dataclass(frozen=True)
class AbilityStatResult:
    """Calculated Deadlock ability bonus percentages as Polars tables.

    Attributes:
        values: One row per selected tick, player, ability, and stat, with unit, mode, ruleset, status, and diagnostic.
        contributions: Effect sources, scope, activation state, and inclusion. Empty unless explain=True.
        metadata: Selected client version, catalog source, mode, and ruleset details.
    """

    values: pl.DataFrame
    contributions: pl.DataFrame
    metadata: dict


_IDENTITY = {
    "tick": pl.Int32,
    "steam_id": pl.UInt64,
    "hero_id": pl.Int64,
}
_BINDING = {
    **_IDENTITY,
    "item_id": pl.UInt32,
    "item_name": pl.String,
    "ability_id": pl.UInt32,
    "ability_name": pl.String,
    "status": pl.String,
    "diagnostic": pl.String,
}


def imbues(
    demo: Demo,
    /,
    *,
    ticks: int | Sequence[int],
    data_version: str,
    steam_ids: Sequence[int] | None = None,
) -> ImbueResult:
    """Read recorded item imbues and their catalog effects at exact ticks.

    Args:
        demo: Boon Demo to query.
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
        >>> demo = Demo("108575009.dem", preload=False)
        >>> result = demo.imbues(ticks=187554, data_version="6712")
        >>> print(result.bindings)
    """
    validate_version(data_version)
    ticks = select_ticks(ticks)
    validate_steam_ids(steam_ids)
    directory = data.update(data_version)
    try:
        payload = json.loads(demo._imbues(directory, ticks, steam_ids=steam_ids))
    except ValueError as error:
        raise CalculationError(str(error)) from error
    payload["metadata"]["data_version"] = data_version
    return ImbueResult(
        pl.DataFrame(payload["bindings"], schema=_BINDING),
        pl.DataFrame(
            payload["effects"],
            schema={
                **_BINDING,
                "stat": pl.String,
                "property_name": pl.String,
                "value": pl.Float64,
                "unit": pl.String,
                "apply_filter": pl.String,
                "source": pl.String,
                "definition_path": pl.String,
            },
        ),
        payload["metadata"],
    )


def calculate_ability_stats(
    demo: Demo,
    /,
    *,
    ticks: int | Sequence[int],
    data_version: str,
    stats: Sequence[AbilityStat | str] = (
        AbilityStat.COOLDOWN_REDUCTION,
        AbilityStat.DURATION_BONUS,
        AbilityStat.RANGE_BONUS,
        AbilityStat.RADIUS_BONUS,
    ),
    mode: StatMode | str = StatMode.CURRENT,
    steam_ids: Sequence[int] | None = None,
    abilities: Sequence[int] | None = None,
    include_items: bool = False,
    rulesets: Mapping[AbilityStat | str, Rule] | None = None,
    explain: bool = False,
    strict: bool = True,
) -> AbilityStatResult:
    """Calculate bonus percentages for each selected Deadlock ability.

    Args:
        demo: Boon Demo to query.
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
        >>> demo = Demo("108575009.dem", preload=False)
        >>> result = demo.calculate_ability_stats(
        ...     ticks=187554, data_version="6712",
        ...     stats=[AbilityStat.COOLDOWN_REDUCTION], strict=False,
        ... )
        >>> print(result.values)
    """
    mode = StatMode(mode)
    validate_version(data_version)
    ticks = select_ticks(ticks)
    validate_steam_ids(steam_ids)
    if abilities is not None and any(
        type(value) is not int or not 0 <= value < 2**32 for value in abilities
    ):
        raise ValueError("abilities must contain unsigned 32-bit integers")
    selected = list(dict.fromkeys(AbilityStat(stat) for stat in stats))
    if not selected:
        raise ValueError("provide at least one ability stat")
    defaults = {stat: getattr(builtin_rulesets, stat.value).v1 for stat in selected}
    chosen = (
        defaults
        if rulesets is None
        else {AbilityStat(stat): rule for stat, rule in rulesets.items()}
    )
    if chosen != defaults:
        raise ValueError(
            "select one supported v1 ruleset for each requested ability stat"
        )
    directory = data.update(data_version)
    try:
        payload = json.loads(
            demo._calculate_ability_stats(
                directory,
                ticks,
                stats=[stat.value for stat in selected],
                mode=mode.value,
                steam_ids=steam_ids,
                abilities=abilities,
                include_items=include_items,
                explain=explain,
                strict=strict,
            )
        )
    except ValueError as error:
        raise CalculationError(str(error)) from error
    payload["metadata"]["data_version"] = data_version
    return AbilityStatResult(
        pl.DataFrame(
            payload["values"],
            schema={
                "mode": pl.String,
                **_IDENTITY,
                "ability_id": pl.UInt32,
                "ability_name": pl.String,
                "stat": pl.String,
                "value": pl.Float64,
                "unit": pl.String,
                "ruleset": pl.String,
                "status": pl.String,
                "diagnostic": pl.String,
            },
        ),
        pl.DataFrame(
            payload["contributions"],
            schema={
                "mode": pl.String,
                **_IDENTITY,
                "ability_id": pl.UInt32,
                "stat": pl.String,
                "value": pl.Float64,
                "source": pl.String,
                "source_ability_id": pl.UInt32,
                "property_name": pl.String,
                "definition_path": pl.String,
                "modifier_serial": pl.UInt32,
                "scope": pl.String,
                "state": pl.String,
                "included": pl.Boolean,
                "diagnostic": pl.String,
            },
        ),
        payload["metadata"],
    )
