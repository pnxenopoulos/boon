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
    """Read selected Deadlock ticks as Polars DataFrames.

    Args:
        demo: Boon Demo to query.
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
    """Add calculated capacity, remaining ammo, and unlimited-ammo state.

    Args:
        demo: Boon Demo used for stat and state queries.
        frame: Sampled player rows with tick, steam_id, hero_id, and ammo_fraction.
        version: Explicit client version used by both queries.

    Returns:
        The input columns plus nullable ammo columns, status, and diagnostics.
        Row order is preserved. Missing or duplicate accounts are not matched.
    """
    if frame.is_empty():
        return frame.with_columns(
            pl.lit(None, dtype=dtype).alias(name)
            for name, dtype in _AMMO_SCHEMA.items()
        )
    columns = frame.columns
    ticks = frame["tick"].unique().sort().to_list()
    keys = ["tick", "steam_id", "hero_id"]
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
    # Never match missing IDs or choose between duplicate account rows.
    unique_account = (
        pl.col("steam_id").is_not_null() & pl.struct("tick", "steam_id").is_unique()
    )
    frame = (
        frame.join(
            capacity.filter(unique_account),
            on=keys,
            how="left",
            validate="m:1",
            maintain_order="left",
        )
        .join(
            states.filter(unique_account),
            on=keys,
            how="left",
            validate="m:1",
            maintain_order="left",
        )
        .with_columns(
            pl.when(unique_account).then(pl.col(name)).otherwise(None).alias(name)
            for name in ("max_ammo", "unlimited_ammo", "ammo_status", "ammo_diagnostic")
        )
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
            pl.when(~unique_account).then(
                pl.lit("Steam ID unavailable or not unique at this tick")
            ),
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
