"""Calculate statistics from parsed Boon demo data.

Each function accepts a :class:`boon.Demo` and returns a Polars DataFrame. The
same functions are convenience methods on ``Demo``. For example,
``demo.kill_participation()`` calls :func:`kill_participation`.

Results use ``hero_id`` as a key. Join them to ``players``, ``kills``,
``player_ticks``, or the ``summary()`` frames.
"""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING

import polars as pl

if TYPE_CHECKING:
    from boon import Demo

__all__ = ["in_combat", "kill_participation", "teamfights", "time_dead"]


def in_combat(demo: Demo) -> pl.DataFrame:
    """Report the combat state of each player for each tick.

    Deadlock stores combat state as a time window on the player pawn. The player
    is in combat while the current game time is less than
    ``in_combat_end_time``. Each hit sets the end time to the last damage time
    plus a delay. The delay is approximately 0.5 seconds for NPC damage and 3.0
    seconds for hero damage. This function calculates the state from
    ``player_ticks``.

    The comparison requires the current game time for each tick. Calculate this
    time from non-paused ticks, as :meth:`Demo.tick_to_seconds` does. Then add one
    constant offset between the demo clock and the engine clock. At a damage
    tick, ``last_damage_time`` equals the current engine time. It cannot be later
    than the current engine time. Thus, the maximum value of
    ``last_damage_time - elapsed_seconds`` is the clock offset.

    Args:
        demo: The demo to analyze.

    Returns:
        A Polars DataFrame with one row for each ``(tick, hero_id)``. Rows are
        sorted by ``tick`` and then ``hero_id``. The frame has these columns:

        - ``tick`` (*int*) -- The game tick.
        - ``hero_id`` (*int*) -- The player's hero ID.
        - ``in_combat`` (*bool*) -- True when the player is in combat.
    """
    tick_rate = demo.tick_rate
    if tick_rate == 0:
        raise ValueError("tick_rate is 0: cannot calculate the game clock")

    # Both inputs are full snapshots for each tick. Request them together.
    # Boon then collects them in one parallel pass for a new Demo.
    demo.load("player_ticks", "world_ticks")

    # Calculate elapsed game time from the cumulative number of non-paused ticks.
    # This calculation is the same as Demo.tick_to_seconds.
    clock = (
        demo.world_ticks.sort("tick")
        .with_columns(
            elapsed_seconds=(~pl.col("is_paused")).cum_sum().cast(pl.Float64)
            / tick_rate
        )
        .select("tick", "elapsed_seconds")
    )

    pt = demo.player_ticks.select(
        "tick", "hero_id", "in_combat_end_time", "in_combat_last_damage_time"
    ).join(clock, on="tick", how="left")

    # Offset between the engine game clock and our elapsed-seconds clock.
    offset = (
        pt.filter(pl.col("in_combat_last_damage_time") > 0.0)
        .select(
            (pl.col("in_combat_last_damage_time") - pl.col("elapsed_seconds")).max()
        )
        .item()
    )
    offset = offset if offset is not None else 0.0

    return (
        pt.with_columns(
            (
                (pl.col("in_combat_end_time") > 0.0)
                & (pl.col("elapsed_seconds") + offset < pl.col("in_combat_end_time"))
            )
            .fill_null(False)
            .alias("in_combat")
        )
        .select("tick", "hero_id", "in_combat")
        .sort("tick", "hero_id")
    )


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
    # Both inputs are full per-tick snapshots. Request them together so Boon
    # collects them in one parallel keyframe-segmented pass on a cold Demo.
    demo.load("player_ticks", "world_ticks")

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


def teamfights(
    demo: Demo,
    *,
    gap_seconds: float = 5.0,
    radius: float = 1500.0,
    min_players: int = 3,
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
        demo: The demo to analyze.
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
          The values exclude paused time and use :meth:`Demo.tick_to_seconds`.
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
    tick_rate = demo.tick_rate
    if tick_rate == 0:
        raise ValueError("tick_rate is 0: cannot cluster damage by time")

    # Damage and kills share one filtered event pass; world_ticks is collected
    # on the parallel snapshot path and later backs the tick-to-seconds calls.
    # Loading these before players also populates the game-over cache used to
    # choose the roster snapshot tick.
    demo.load("damage", "kills", "world_ticks")

    schema = {
        "fight_id": pl.Int64,
        "start_tick": pl.Int64,
        "end_tick": pl.Int64,
        "start_seconds": pl.Float64,
        "end_seconds": pl.Float64,
        "duration_seconds": pl.Float64,
        "center_x": pl.Float64,
        "center_y": pl.Float64,
        "participants": pl.List(pl.Int64),
        "num_participants": pl.Int64,
        "hero_damage": pl.Int64,
        "kills": pl.Int64,
    }

    teams = demo.players.select("hero_id", "team_num")
    attacker_teams = teams.rename({"hero_id": "attacker_hero_id", "team_num": "at"})
    victim_teams = teams.rename({"hero_id": "victim_hero_id", "team_num": "vt"})

    # Hero-vs-hero damage between opposing teams -- the actual "fighting" signal --
    # placed at the victim's position (falling back to the attacker's).
    dmg = (
        demo.damage.filter(
            (pl.col("attacker_hero_id") != 0)
            & (pl.col("victim_hero_id") != 0)
            & (pl.col("attacker_hero_id") != pl.col("victim_hero_id"))
        )
        .join(attacker_teams, on="attacker_hero_id", how="inner")
        .join(victim_teams, on="victim_hero_id", how="inner")
        .filter(pl.col("at") != pl.col("vt"))
    )
    if dmg.is_empty():
        return pl.DataFrame(schema=schema)

    # Only positions at damage ticks can contribute to a fight. Request only
    # those positions. This prevents creation of the full player_ticks frame.
    damage_ticks = dmg.get_column("tick").unique().to_list()
    pos = demo.snapshots(ticks=damage_ticks).select("tick", "hero_id", "x", "y")
    dmg = (
        dmg.join(
            pos.rename({"hero_id": "victim_hero_id", "x": "vx", "y": "vy"}),
            on=["tick", "victim_hero_id"],
            how="left",
        )
        .join(
            pos.rename({"hero_id": "attacker_hero_id", "x": "ax", "y": "ay"}),
            on=["tick", "attacker_hero_id"],
            how="left",
        )
        .with_columns(x=pl.coalesce("vx", "ax"), y=pl.coalesce("vy", "ay"))
        .filter(pl.col("x").is_not_null())
        .sort("tick")
    )
    if dmg.is_empty():
        return pl.DataFrame(schema=schema)

    # Sweep events in tick order, assigning each to the nearest active fight
    # (centroid within `radius`, last event within `gap_seconds`) or a new one.
    time_eps = max(1, round(gap_seconds * tick_rate))
    r2 = radius * radius
    ticks = dmg["tick"].to_list()
    xs = dmg["x"].to_list()
    ys = dmg["y"].to_list()
    active: list[list[float]] = []  # [last_tick, cx, cy, count, label]
    labels: list[int] = []
    next_label = 0
    for t, x, y in zip(ticks, xs, ys):
        active = [f for f in active if t - f[0] <= time_eps]
        best = None
        best_d = r2
        for f in active:
            d = (x - f[1]) ** 2 + (y - f[2]) ** 2
            if d <= best_d:
                best = f
                best_d = d
        if best is None:
            best = [t, x, y, 0, next_label]
            active.append(best)
            next_label += 1
        n = best[3] + 1
        best[0] = t
        best[1] += (x - best[1]) / n  # running-mean centroid
        best[2] += (y - best[2]) / n
        best[3] = n
        labels.append(int(best[4]))

    dmg = dmg.with_columns(pl.Series("_fight", labels, dtype=pl.Int64))

    windows = dmg.group_by("_fight").agg(
        start_tick=pl.col("tick").min().cast(pl.Int64),
        end_tick=pl.col("tick").max().cast(pl.Int64),
        center_x=pl.col("x").mean().cast(pl.Float64),
        center_y=pl.col("y").mean().cast(pl.Float64),
        hero_damage=pl.col("damage").sum().cast(pl.Int64),
    )
    participants = (
        dmg.select("_fight", "attacker_hero_id", "victim_hero_id")
        .unpivot(
            index="_fight",
            on=["attacker_hero_id", "victim_hero_id"],
            value_name="hero_id",
        )
        .group_by("_fight")
        .agg(participants=pl.col("hero_id").unique())
        .with_columns(
            participants=pl.col("participants").list.sort(),
            num_participants=pl.col("participants").list.len().cast(pl.Int64),
        )
    )

    fights = (
        windows.join(participants, on="_fight")
        .filter(pl.col("num_participants") >= min_players)
        .sort("start_tick")
    )
    if fights.is_empty():
        return pl.DataFrame(schema=schema)

    # Assign each kill to the fight that last damaged the victim before the kill.
    # This rule prevents duplicate kills in fights with overlapping time windows.
    victim_dmg = dmg.select("tick", "victim_hero_id", "_fight").sort("tick")
    kills_sorted = demo.kills.select("tick", "victim_hero_id").sort("tick")
    with warnings.catch_warnings():
        # The frames are sorted by tick and by victim group.
        # Polars cannot verify this condition efficiently with a `by` group.
        warnings.filterwarnings(
            "ignore", message="Sortedness of columns cannot be checked"
        )
        attributed = kills_sorted.join_asof(
            victim_dmg, on="tick", by="victim_hero_id", strategy="backward"
        )
    kills_by_fight = (
        attributed.drop_nulls("_fight")
        .group_by("_fight")
        .agg(kills=pl.len().cast(pl.Int64))
    )
    fights = (
        fights.join(kills_by_fight, on="_fight", how="left")
        .with_columns(pl.col("kills").fill_null(0))
        .sort("start_tick")
    )

    starts = fights["start_tick"].to_list()
    ends = fights["end_tick"].to_list()
    return (
        fights.with_columns(
            fight_id=pl.int_range(1, pl.len() + 1, dtype=pl.Int64),
            start_seconds=pl.Series(
                [demo.tick_to_seconds(int(t)) for t in starts], dtype=pl.Float64
            ),
            end_seconds=pl.Series(
                [demo.tick_to_seconds(int(t)) for t in ends], dtype=pl.Float64
            ),
        )
        .with_columns(
            (pl.col("end_seconds") - pl.col("start_seconds")).alias("duration_seconds")
        )
        .select(
            "fight_id",
            "start_tick",
            "end_tick",
            "start_seconds",
            "end_seconds",
            "duration_seconds",
            "center_x",
            "center_y",
            "participants",
            "num_participants",
            "hero_damage",
            "kills",
        )
    )
