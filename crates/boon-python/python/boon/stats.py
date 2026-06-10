"""Derived statistics computed from parsed Boon demo data.

This module is the analysis layer on top of the parser: each function takes a
:class:`boon.Demo` and returns a Polars DataFrame of a derived metric. The same
functions are surfaced as convenience methods on ``Demo`` (e.g.
``demo.kill_participation()`` delegates to :func:`kill_participation`).

Stats are keyed on ``hero_id`` so they join cleanly to the parser's other
frames (``players``, ``kills``, ``player_ticks``, ``summary()`` outputs, ...).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import polars as pl

if TYPE_CHECKING:
    from boon import Demo

__all__ = ["kill_participation", "time_dead"]


def kill_participation(
    demo: Demo,
    *,
    start_tick: int | None = None,
    end_tick: int | None = None,
) -> pl.DataFrame:
    """Kill participation per player: ``(kills + assists) / team_kills``.

    A player participates in a team kill when they are credited as either the
    killer or an assister (never both on the same kill), so the value is a
    fraction in ``[0, 1]`` — the share of their team's kills they were involved
    in.

    Args:
        demo: The demo to compute over.
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
    kills = demo.kills
    if start_tick is not None:
        kills = kills.filter(pl.col("tick") >= start_tick)
    if end_tick is not None:
        kills = kills.filter(pl.col("tick") <= end_tick)

    players = demo.players.select("hero_id", "team_num")

    # Kills credited to each attacker hero.
    per_kills = (
        kills.group_by("attacker_hero_id")
        .len()
        .rename({"attacker_hero_id": "hero_id", "len": "kills"})
    )

    # Assists credited to each hero (a single kill can have several assisters).
    per_assists = (
        kills.select("assister_hero_ids")
        .explode("assister_hero_ids")
        .drop_nulls()
        .group_by("assister_hero_ids")
        .len()
        .rename({"assister_hero_ids": "hero_id", "len": "assists"})
    )

    # Total kills per team (each kill credited to its attacker's team).
    team_kills = (
        kills.join(players, left_on="attacker_hero_id", right_on="hero_id")
        .group_by("team_num")
        .len()
        .rename({"len": "team_kills"})
    )

    return (
        players.join(per_kills, on="hero_id", how="left")
        .join(per_assists, on="hero_id", how="left")
        .join(team_kills, on="team_num", how="left")
        .with_columns(
            pl.col("kills", "assists", "team_kills").fill_null(0).cast(pl.Int64),
        )
        .with_columns(
            pl.when(pl.col("team_kills") > 0)
            .then((pl.col("kills") + pl.col("assists")) / pl.col("team_kills"))
            .otherwise(None)
            .alias("kill_participation"),
        )
        .select(
            "hero_id",
            "team_num",
            "kills",
            "assists",
            "team_kills",
            "kill_participation",
        )
        .sort("team_num", "hero_id")
    )


def time_dead(demo: Demo) -> pl.DataFrame:
    """Time each player spent dead during regulation play.

    A player is counted as dead on any tick where they are not alive
    (``is_alive == False``). Only non-paused ticks up to the game-over event are
    counted, so the totals line up with ``demo.regulation_ticks`` /
    ``demo.regulation_seconds`` (the active, paused-time-excluded duration of
    regulation play).

    Args:
        demo: The demo to compute over.

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
    game_over_tick = demo.game_over_tick
    regulation_ticks = demo.regulation_ticks
    tick_rate = demo.tick_rate
    if (
        game_over_tick is None
        or regulation_ticks is None
        or regulation_ticks == 0
        or tick_rate == 0
    ):
        raise ValueError(
            "regulation time is undefined: this demo has no game-over event"
        )

    players = demo.players.select("hero_id", "team_num")

    # Ticks the game was paused (usually a small set; empty for unpaused matches).
    paused_ticks = demo.world_ticks.filter(pl.col("is_paused")).select("tick")

    # Dead = not alive, within regulation (tick <= game_over), on non-paused ticks.
    dead = (
        demo.player_ticks.select("tick", "hero_id", "is_alive")
        .filter(~pl.col("is_alive") & (pl.col("tick") <= game_over_tick))
        .join(paused_ticks, on="tick", how="anti")
        .group_by("hero_id")
        .len()
        .rename({"len": "ticks_dead"})
    )

    return (
        players.join(dead, on="hero_id", how="left")
        .with_columns(pl.col("ticks_dead").fill_null(0).cast(pl.Int64))
        .with_columns(
            (pl.col("ticks_dead") / tick_rate).alias("seconds_dead"),
            (pl.col("ticks_dead") / regulation_ticks * 100).alias(
                "pct_regulation_dead"
            ),
        )
        .select(
            "hero_id",
            "team_num",
            "ticks_dead",
            "seconds_dead",
            "pct_regulation_dead",
        )
        .sort("team_num", "hero_id")
    )
