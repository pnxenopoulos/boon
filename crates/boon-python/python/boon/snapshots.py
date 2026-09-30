"""Sample recorded state and optionally calculate primary-gun ammo."""

from __future__ import annotations

from typing import TYPE_CHECKING

import polars as pl

from boon.hero_stats import HeroStat

if TYPE_CHECKING:
    from boon import Demo


_AMMO_SCHEMA = {
    "ammo": pl.UInt32,
    "max_ammo": pl.UInt32,
    "unlimited_ammo": pl.Boolean,
    "ammo_status": pl.String,
    "ammo_diagnostic": pl.String,
}


def snapshots(
    demo: Demo,
    /,
    datasets: str | list[str] | None = None,
    *,
    ticks: int | list[int] | None = None,
    every: int | None = None,
    seconds: float | None = None,
    events: str | list[str] | None = None,
    start_tick: int | None = None,
    end_tick: int | None = None,
    data_version: str | None = None,
) -> pl.DataFrame | dict[str, pl.DataFrame]:
    """Read player_ticks, world_ticks, or troopers at selected ticks.

    Select ticks, a stride (every or seconds), event ticks, or a tick window.
    One dataset returns a DataFrame; multiple datasets return a dict.
    Player rows always include the primary weapon's recorded ammo_fraction.

    Set data_version to a boon-data client version to add ammo, max_ammo,
    unlimited_ammo, ammo_status, and ammo_diagnostic to player_ticks. A missing
    version is downloaded. This option requires player_ticks and adds stat and
    state queries at the sampled ticks. Without it, no catalog is needed.

    max_ammo is finite capacity. ammo is ammo_fraction * max_ammo, rounded to
    the nearest integer (halves round up). Partial capacity produces partial
    ammo. Missing inputs produce null values and diagnostics. unlimited_ammo
    reports INFINITE_CLIP in the recorded predicted-state mask; unknown state
    bits make it null unless INFINITE_CLIP is present. It does not change capacity.
    """
    if data_version is not None:
        if not isinstance(data_version, str) or not data_version:
            raise ValueError("data_version must be a version from `boon versions`")
        names = [datasets] if isinstance(datasets, str) else datasets
        if names is not None and "player_ticks" not in names:
            raise ValueError("data_version requires the player_ticks dataset")

    result = demo._snapshots(
        datasets,
        ticks=ticks,
        every=every,
        seconds=seconds,
        events=events,
        start_tick=start_tick,
        end_tick=end_tick,
    )
    if data_version is None:
        return result
    if isinstance(result, dict):
        result["player_ticks"] = _with_ammo(demo, result["player_ticks"], data_version)
        return result
    return _with_ammo(demo, result, data_version)


def _with_ammo(demo: Demo, frame: pl.DataFrame, version: str) -> pl.DataFrame:
    if frame.is_empty():
        return frame.with_columns(
            pl.lit(None, dtype=dtype).alias(name)
            for name, dtype in _AMMO_SCHEMA.items()
        )
    columns = frame.columns
    ticks = frame["tick"].unique().sort().to_list()
    keys = ["tick", "player_slot", "hero_id"]
    capacity = demo.calculate_hero_stats(
        ticks=ticks, data_version=version, stats=[HeroStat.CLIP_SIZE], strict=False
    ).values.select(
        *keys,
        pl.col("value").cast(pl.UInt32, strict=False).alias("max_ammo"),
        pl.col("status").alias("ammo_status"),
        pl.col("diagnostic").alias("ammo_diagnostic"),
    )
    states = demo.player_states(ticks=ticks, data_version=version).select(
        *keys,
        pl.when(pl.col("states").list.contains("INFINITE_CLIP"))
        .then(True)
        .when(pl.col("unknown_states").list.len() == 0)
        .then(pl.col("states").list.contains("INFINITE_CLIP"))
        .otherwise(None)
        .alias("unlimited_ammo"),
    )
    # Slots identify rows within one tick, including bots with no Steam ID and
    # duplicate heroes. Keep the exact UInt64 Steam ID in the returned snapshot.
    frame = frame.join(capacity, on=keys, how="left", validate="1:1").join(
        states, on=keys, how="left", validate="1:1"
    )
    missing = pl.col("ammo_fraction").is_null() | pl.col("max_ammo").is_null()
    return frame.with_columns(
        (pl.col("ammo_fraction").cast(pl.Float64) * pl.col("max_ammo") + 0.5)
        .floor()
        .cast(pl.UInt32, strict=False)
        .alias("ammo"),
        pl.when(missing)
        .then(pl.lit("unresolved"))
        .otherwise(pl.col("ammo_status"))
        .alias("ammo_status"),
        pl.concat_str(
            pl.col("ammo_diagnostic"),
            pl.when(pl.col("ammo_fraction").is_null()).then(
                pl.lit("ammo fraction unavailable")
            ),
            pl.when(pl.col("ammo_status").is_null()).then(
                pl.lit("ammo capacity unavailable")
            ),
            separator="; ",
            ignore_nulls=True,
        )
        .replace("", None)
        .alias("ammo_diagnostic"),
    ).select(*columns, *_AMMO_SCHEMA)
