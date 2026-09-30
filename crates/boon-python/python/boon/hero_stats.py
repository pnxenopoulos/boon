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
from boon._selection import validate_steam_ids
from boon.rulesets import Rule

if TYPE_CHECKING:
    from boon import Demo


class HeroStat(StrEnum):
    """Stats with implemented rules. These values are not network enum IDs."""

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
    """Select current effects or baseline passive and permanent inputs."""

    CURRENT = "current"
    BASELINE = "baseline"


class CalculationError(ValueError):
    """A requested stat cannot be calculated from the selected inputs."""


@dataclass(frozen=True)
class StatResult:
    """Calculated rows, optional input trace, and catalog/ruleset provenance."""

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
    """Calculate hero stats after each selected tick.

    Use ``HeroStat`` enum members for ``stats``. Matching strings also work.
    The result's ``stat`` column contains strings. ``HeroStat.AMMO`` selects
    the same stat as ``HeroStat.CLIP_SIZE``. Omit stats to select ammo capacity.

    Select Steam accounts with ``steam_ids`` or current hero IDs with ``heroes``.
    Both filters apply when set. Get Steam IDs from ``demo.players``.
    Omit both filters to include all players. An empty filter selects no players.
    Results contain ``steam_id`` (UInt64), or null when the ID is missing.
    Use ``steam_id`` for player joins. Do not join missing IDs as one player.

    Set ``data_version`` to a client version from ``boon versions``.
    Boon downloads and verifies a missing version. Omit ``rulesets`` to use V1.
    Otherwise, supply one supported rule for each selected stat.

    Use ``mode="current"`` (default) to include supported active effects.
    Use ``mode="baseline"`` for hero values, owned passive effects, and permanent
    recorded changes. Both modes use the selected tick and apply to dependencies.
    Unknown effect roles produce partial baseline values. Baseline gravity scale
    is unavailable. Mode does not change stat units or the selected equation.

    Set ``explain=True`` to include input sources and catalog paths.
    Unknown modifiers and assumed links produce partial values with diagnostics.
    Other missing inputs raise ``CalculationError``. With ``strict=False``, those
    rows have null values and status ``unresolved``. Invalid queries still fail.

    Ammo is capacity, not remaining rounds. Fire rate is the bonus percentage.
    Move speed and sprint speed are nominal values, without current movement states.
    See the hero stats guide for units, equations, and limits for each stat.
    """
    if not isinstance(data_version, str):
        raise ValueError(
            "data_version must be an explicit client version from `boon versions`"
        )
    mode = StatMode(mode)
    selected = list(dict.fromkeys(HeroStat(stat) for stat in stats))
    requested_ticks = [ticks] if isinstance(ticks, int) else list(ticks)
    if not selected or not requested_ticks:
        raise ValueError("provide ticks and at least one stat")
    if any(type(tick) is not int or tick < 0 for tick in requested_ticks):
        raise ValueError("ticks must be nonnegative integers")
    validate_steam_ids(steam_ids)
    if heroes is not None and any(
        type(value) is not int or value < 0 for value in heroes
    ):
        raise ValueError("heroes must contain nonnegative integers")
    defaults = {
        HeroStat.CLIP_SIZE: builtin_rulesets.clip_size.v1,
        HeroStat.BULLET_VELOCITY: builtin_rulesets.bullet_velocity.v1,
        HeroStat.MELEE_DISTANCE: builtin_rulesets.melee_distance.v1,
        HeroStat.RELOAD_TIME: builtin_rulesets.reload_time.v1,
        HeroStat.FIRE_RATE: builtin_rulesets.fire_rate.v1,
        HeroStat.FALLOFF_START: builtin_rulesets.falloff_start.v1,
        HeroStat.FALLOFF_END: builtin_rulesets.falloff_end.v1,
        HeroStat.LIGHT_MELEE_DAMAGE: builtin_rulesets.light_melee_damage.v1,
        HeroStat.HEAVY_MELEE_DAMAGE: builtin_rulesets.heavy_melee_damage.v1,
        HeroStat.SLIDE_DISTANCE: builtin_rulesets.slide_distance.v1,
        HeroStat.BULLET_EVASION: builtin_rulesets.bullet_evasion.v1,
        HeroStat.DEBUFF_RESIST: builtin_rulesets.debuff_resist.v1,
        HeroStat.WEAPON_DAMAGE: builtin_rulesets.weapon_damage.v1,
        HeroStat.MELEE_RESIST: builtin_rulesets.melee_resist.v1,
        HeroStat.SPIRIT_RESIST: builtin_rulesets.spirit_resist.v1,
        HeroStat.BULLET_RESIST: builtin_rulesets.bullet_resist.v1,
        HeroStat.MELEE_LIFESTEAL: builtin_rulesets.melee_lifesteal.v1,
        HeroStat.SPIRIT_LIFESTEAL: builtin_rulesets.spirit_lifesteal.v1,
        HeroStat.BULLET_LIFESTEAL: builtin_rulesets.bullet_lifesteal.v1,
        HeroStat.GRAVITY_SCALE: builtin_rulesets.gravity_scale.v1,
        HeroStat.STAMINA: builtin_rulesets.stamina.v1,
        HeroStat.STAMINA_COOLDOWN: builtin_rulesets.stamina_cooldown.v1,
        HeroStat.DASH_SPEED: builtin_rulesets.dash_speed.v1,
        HeroStat.DASH_DURATION: builtin_rulesets.dash_duration.v1,
        HeroStat.AIR_DASH_SPEED: builtin_rulesets.air_dash_speed.v1,
        HeroStat.AIR_DASH_DURATION: builtin_rulesets.air_dash_duration.v1,
        HeroStat.MOVE_SPEED: builtin_rulesets.move_speed.v1,
        HeroStat.SPRINT_SPEED: builtin_rulesets.sprint_speed.v1,
    }
    chosen = (
        {stat: defaults[stat] for stat in selected}
        if rulesets is None
        else {HeroStat(stat): rule for stat, rule in rulesets.items()}
    )
    if set(chosen) != set(selected):
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
            "input": pl.String,
            "kind": pl.String,
            "value": pl.Float64,
            "source": pl.String,
            "definition_path": pl.String,
            "modifier_serial": pl.UInt32,
        },
    )
    return StatResult(values, contributions, payload["metadata"])
