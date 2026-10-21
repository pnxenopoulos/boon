"""Calculate hero stats from replay state and a selected boon-data catalog."""

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
from boon.rulesets import Rule

if TYPE_CHECKING:
    from boon import Demo


class HeroStat(StrEnum):
    """Hero stat names accepted by Boon calculation queries.

    Use enum members or their exact string values. AMMO is an alias for
    CLIP_SIZE. These strings are not Source 2 network enum IDs.
    """

    CLIP_SIZE = "clip_size"
    AMMO = "clip_size"
    BULLET_VELOCITY = "bullet_velocity"
    MELEE_DISTANCE = "melee_distance"
    RELOAD_TIME = "reload_time"
    FIRE_RATE = "fire_rate"
    FALLOFF_START = "falloff_start"
    FALLOFF_END = "falloff_end"
    LIGHT_MELEE_DAMAGE = "light_melee_damage"
    HEAVY_MELEE_DAMAGE = "heavy_melee_damage"
    SLIDE_DISTANCE = "slide_distance"
    BULLET_EVASION = "bullet_evasion"
    DEBUFF_RESIST = "debuff_resist"
    WEAPON_DAMAGE = "weapon_damage"
    MELEE_RESIST = "melee_resist"
    SPIRIT_RESIST = "spirit_resist"
    BULLET_RESIST = "bullet_resist"
    MELEE_LIFESTEAL = "melee_lifesteal"
    SPIRIT_LIFESTEAL = "spirit_lifesteal"
    BULLET_LIFESTEAL = "bullet_lifesteal"
    GRAVITY_SCALE = "gravity_scale"
    STAMINA = "stamina"
    STAMINA_COOLDOWN = "stamina_cooldown"
    DASH_SPEED = "dash_speed"
    DASH_DURATION = "dash_duration"
    AIR_DASH_SPEED = "air_dash_speed"
    AIR_DASH_DURATION = "air_dash_duration"
    MOVE_SPEED = "move_speed"
    SPRINT_SPEED = "sprint_speed"


class StatMode(StrEnum):
    """Input modes for Boon hero and ability calculations.

    CURRENT includes supported active effects. BASELINE includes hero values,
    owned passive effects, and permanent recorded bonuses at the selected tick.
    """

    CURRENT = "current"
    BASELINE = "baseline"


class CalculationError(ValueError):
    """A requested stat cannot be calculated from the selected inputs."""


@dataclass(frozen=True)
class StatResult:
    """Polars tables from a Boon hero-stat query.

    Attributes:
        values: Rows keyed by tick, steam_id, hero_id, and stat, with value, unit, mode, ruleset, status, and diagnostic.
        contributions: Source values and catalog paths. Empty unless explain=True.
        metadata: Selected data version, source revision, mode, and ruleset details.
    """

    values: pl.DataFrame
    contributions: pl.DataFrame
    metadata: dict


def calculate_hero_stats(
    demo: Demo,
    /,
    *,
    ticks: int | Sequence[int],
    data_version: str,
    stats: Sequence[HeroStat | str] = (HeroStat.CLIP_SIZE,),
    mode: StatMode | str = StatMode.CURRENT,
    steam_ids: Sequence[int] | None = None,
    heroes: Sequence[int] | None = None,
    rulesets: Mapping[HeroStat | str, Rule] | None = None,
    explain: bool = False,
    strict: bool = True,
) -> StatResult:
    """Calculate selected Deadlock hero stats after each exact tick.

    Args:
        demo: Boon Demo to query. Reuse this object for repeated queries.
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
        >>> demo = Demo("108575009.dem", preload=False)
        >>> result = demo.calculate_hero_stats(
        ...     ticks=187554, data_version="6712",
        ...     stats=[HeroStat.CLIP_SIZE, HeroStat.FIRE_RATE], strict=False,
        ... )
        >>> print(result.values)
    """
    validate_version(data_version)
    mode = StatMode(mode)
    selected = list(dict.fromkeys(HeroStat(stat) for stat in stats))
    requested_ticks = select_ticks(ticks)
    if not selected:
        raise ValueError("provide ticks and at least one stat")
    validate_steam_ids(steam_ids)
    if heroes is not None and any(
        type(value) is not int or value < 0 for value in heroes
    ):
        raise ValueError("heroes must contain nonnegative integers")
    defaults = {stat: getattr(builtin_rulesets, stat.value).v1 for stat in selected}
    chosen = (
        defaults
        if rulesets is None
        else {HeroStat(stat): rule for stat, rule in rulesets.items()}
    )
    if chosen.keys() != defaults.keys():
        raise ValueError("select one ruleset for each requested stat")
    for stat in selected:
        if chosen[stat] != defaults[stat]:
            raise ValueError(f"select boon.rulesets.{stat}.v1 for {stat}")

    directory = data.update(data_version)
    try:
        payload = json.loads(
            demo._calculate_hero_stats(
                directory,
                requested_ticks,
                stats=[stat.value for stat in selected],
                mode=mode.value,
                steam_ids=None if steam_ids is None else list(steam_ids),
                heroes=None if heroes is None else list(heroes),
                explain=explain,
                strict=strict,
            )
        )
    except ValueError as error:
        raise CalculationError(str(error)) from error
    payload["metadata"]["data_version"] = data_version
    values = pl.DataFrame(
        payload["values"],
        schema={
            "mode": pl.String,
            "tick": pl.Int32,
            "steam_id": pl.UInt64,
            "hero_id": pl.Int64,
            "stat": pl.String,
            "value": pl.Float64,
            "unit": pl.String,
            "ruleset": pl.String,
            "status": pl.String,
            "diagnostic": pl.String,
        },
    )
    contributions = pl.DataFrame(
        payload["contributions"],
        schema={
            "mode": pl.String,
            "tick": pl.Int32,
            "steam_id": pl.UInt64,
            "hero_id": pl.Int64,
            "stat": pl.String,
            "input": pl.String,
            "kind": pl.String,
            "value": pl.Float64,
            "source": pl.String,
            "definition_path": pl.String,
            "modifier_serial": pl.UInt32,
        },
    )
    return StatResult(values, contributions, payload["metadata"])
