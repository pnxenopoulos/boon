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
    players: Sequence[int] | None = None,
    heroes: Sequence[int] | None = None,
    rulesets: Mapping[HeroStat | str, Rule] | None = None,
    explain: bool = False,
    strict: bool = True,
) -> StatResult:
    """Calculate hero stats at exact demo ticks, after tick updates.

    Select player slots with ``players`` or current hero IDs with ``heroes``.
    Omit both to include all players. ``data_version`` is a client version from
    ``boon versions``. A missing installation is downloaded and verified.

    ``HeroStat.AMMO`` is an alias for ``HeroStat.CLIP_SIZE``. Capacity remains
    finite during unlimited-ammo effects. It is not the number of rounds left.
    ``HeroStat.BULLET_VELOCITY`` returns nominal weapon bullet speed in m/s.
    ``HeroStat.MELEE_DISTANCE`` returns the heavy-melee travel bonus in percent.
    ``HeroStat.RELOAD_TIME`` returns nominal reload seconds, per round for
    single-round reload weapons. It excludes their initial delay.
    ``HeroStat.FIRE_RATE`` returns the UI modifier in percentage points,
    with additive bonuses, multiplicative slows, and a -50% minimum.
    ``HeroStat.FALLOFF_START`` and ``HeroStat.FALLOFF_END`` return nominal weapon
    damage falloff endpoints in metres, not maximum bullet travel distance.
    V1 supports at most one nonzero range bonus; stacking is not yet verified.

    ``HeroStat.LIGHT_MELEE_DAMAGE`` and ``HeroStat.HEAVY_MELEE_DAMAGE`` return
    nominal damage before target resistance and on-hit effects, without rounding.

    ``HeroStat.SLIDE_DISTANCE`` multiplies bonus factors and returns percent.
    ``HeroStat.BULLET_EVASION`` returns a chance from stat bindings. For an
    undeclared ``EvasionPercent``, its owner's unique effect modifier supplies
    an inferred binding, marked partial. Other undeclared effects can be omitted;
    zero does not prove no evasion.
    ``HeroStat.DEBUFF_RESIST`` returns duration resistance in percentage points.
    Remaining-duration factors multiply; negative resistance extends duration.
    It includes catalog innate values and effective bound modifiers, not immunity
    or cleansing. An absent innate value contributes zero.
    ``HeroStat.GRAVITY_SCALE`` returns the recorded pawn multiplier unchanged.

    ``HeroStat.STAMINA`` returns maximum capacity in points. Free dashes do
    not change this value. ``HeroStat.STAMINA_COOLDOWN`` returns seconds to
    recover one point, not the time remaining on the current refill.
    ``DASH_SPEED`` and ``AIR_DASH_SPEED`` return nominal average speed in m/s.
    ``DASH_DURATION`` and ``AIR_DASH_DURATION`` return catalog duration in seconds.
    Distance bonuses change speed, not duration. These are ordinary dash stats;
    flight, attack movement, interruptions, and acceleration are outside V1.
    Multiple nonzero recovery or dash-distance percentages remain unresolved.
    Paused recovery and mixed flat/percentage recovery also remain unresolved.

    ``HeroStat.MOVE_SPEED`` returns nominal movement speed in m/s with
    diminishing flat bonuses. ``HeroStat.SPRINT_SPEED`` returns the additional
    sprint component in m/s with additive bonuses. Their sum is full sprint
    speed. Both use catalog hero spirit scaling when present. They exclude
    firing, crouching, slows, speed limits, sprint eligibility and ramp-up.
    Unbound effects are partial; unsupported stacking remains unresolved.

    With ``explain=True``, include each resolved contribution and its source.
    Failed modifier lookups are skipped. Affected rows have status ``partial``
    and a diagnostic with the skipped IDs, even when ``strict=True``.
    Conditional bonuses inferred from a unique temporary modifier also produce
    partial rows, with a diagnostic that identifies the assumed activation link.
    Other unresolved inputs raise CalculationError. With ``strict=False``,
    affected values are null and the diagnostic column gives the reason.
    Invalid queries and missing ticks always raise an error.
    """
    if not isinstance(data_version, str):
        raise ValueError(
            "data_version must be an explicit client version from `boon versions`"
        )
    selected = list(dict.fromkeys(HeroStat(stat) for stat in stats))
    requested_ticks = [ticks] if isinstance(ticks, int) else list(ticks)
    if not selected or not requested_ticks:
        raise ValueError("provide ticks and at least one stat")
    if any(type(tick) is not int or tick < 0 for tick in requested_ticks):
        raise ValueError("ticks must be nonnegative integers")
    for name, ids in (("players", players), ("heroes", heroes)):
        if ids is not None and any(
            type(value) is not int or value < 0 for value in ids
        ):
            raise ValueError(f"{name} must contain nonnegative integers")
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
                players=None if players is None else list(players),
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
            "tick": pl.Int32,
            "player_slot": pl.UInt32,
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
            "tick": pl.Int32,
            "player_slot": pl.UInt32,
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
