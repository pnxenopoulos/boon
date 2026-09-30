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
from boon._selection import validate_steam_ids
from boon.hero_stats import CalculationError, StatMode
from boon.rulesets import Rule

if TYPE_CHECKING:
    from boon import Demo


class AbilityStat(StrEnum):
    """Bonus percentages, separate from seconds, metres, and running timers."""

    COOLDOWN_REDUCTION = "cooldown_reduction"
    ITEM_COOLDOWN_REDUCTION = "item_cooldown_reduction"
    DURATION_BONUS = "duration_bonus"
    RANGE_BONUS = "range_bonus"
    RADIUS_BONUS = "radius_bonus"


@dataclass(frozen=True)
class ImbueResult:
    """Recorded selections and individual catalog effects; no stacking applied."""

    bindings: pl.DataFrame
    effects: pl.DataFrame
    metadata: dict


@dataclass(frozen=True)
class AbilityStatResult:
    """Per-ability percentages, optional source trace, and catalog provenance."""

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


def _query(ticks, data_version, steam_ids):
    if not isinstance(data_version, str):
        raise ValueError(
            "data_version must be an explicit version from `boon versions`"
        )
    ticks = [ticks] if isinstance(ticks, int) else list(ticks)
    if not ticks or any(type(tick) is not int or tick < 0 for tick in ticks):
        raise ValueError("provide nonnegative integer ticks")
    validate_steam_ids(steam_ids)
    return ticks


def _ids(name, values):
    if values is not None and any(
        type(v) is not int or not 0 <= v <= 2**32 - 1 for v in values
    ):
        raise ValueError(f"{name} must contain unsigned 32-bit integers")


def imbues(
    demo: Demo,
    /,
    *,
    ticks: int | Sequence[int],
    data_version: str,
    steam_ids: Sequence[int] | None = None,
) -> ImbueResult:
    """Read item imbues after each exact tick using an explicit catalog version.

    Select Steam accounts with ``steam_ids``; omit it to include all players.
    All tables include ``steam_id`` (UInt64), or null when it is unavailable.
    A missing installation is downloaded and verified. Missing catalog records
    do not discard recorded bindings. Each effect retains its targeting filter.
    Effects contain catalog values, not combined bonuses or final cast values.
    Temporary next-cast effects are reported by calculate_ability_stats instead.
    """
    ticks = _query(ticks, data_version, steam_ids)
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
    """Calculate applicable bonus percentages for each selected ability.

    Select stats with ``AbilityStat`` enum members. Matching strings also work.
    The result's ``stat`` column contains strings.
    Select Steam accounts with ``steam_ids``; get IDs from ``demo.players``.
    All tables include ``steam_id`` (UInt64), or null when it is unavailable.
    The default selection contains the hero's signature abilities.
    Select owned ability or item IDs with ``abilities``.
    Use ``include_items=True`` to include all owned items.
    Ability and item cooldown reductions remain separate. Range and radius are
    separate stats. These values are not seconds, metres, or remaining cooldowns.

    Use ``mode="current"`` (default) to include supported active effects.
    Use ``mode="baseline"`` for owned passive bonuses, persistent imbues, and
    permanent recorded changes. Temporary and next-cast effects are excluded.
    Unknown source roles retain diagnostics. Results and metadata include mode.

    V1 combines source percentages as 100 * (1 - product(1 - bonus / 100)).
    Permanent totals, active modifiers, and recorded dynamic values are included
    once. Catalog filters restrict imbued and charged-ability effects.
    With explain=True, contributions include scope, activation, and whether each
    source was included. Ready next-cast effects are never applied to every skill.
    Unsupported activation is partial with a diagnostic. Missing inputs raise
    CalculationError; strict=False returns unresolved rows with null values.
    """
    mode = StatMode(mode)
    ticks = _query(ticks, data_version, steam_ids)
    _ids("abilities", abilities)
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
