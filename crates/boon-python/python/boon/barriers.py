"""Barrier flow per hero: barrier gained, and how much of it stopped damage.

A barrier is a temporary pool that absorbs damage before health. ``player_ticks.barrier``
samples that pool per hero; this reads it as a flow. A rise opens a barrier; each fall is
charged against the barriers still standing and booked as ``absorbed`` when ``damage`` shows
the hero taking a hit on that tick, or ``expired`` otherwise. With several barriers up the
pool is a single number, so a fall is split across them in proportion to what each has left.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import polars as pl

if TYPE_CHECKING:
    from boon import Demo

__all__ = ["barriers"]

_RISE = 1.0  # ignore sub-HP pool jitter


def barriers(demo: Demo) -> pl.DataFrame:
    """Every barrier gained in the match, with how much of it it absorbed.

    Args:
        demo: The demo to analyze.

    Returns:
        A Polars DataFrame with one row per barrier, sorted by ``tick``. Columns:

        - ``tick`` (*int*) -- The tick it landed.
        - ``hero_id`` (*int*) -- The hero it was on.
        - ``granted`` (*float*) -- Barrier HP gained (amplification included).
        - ``absorbed`` (*float*) -- How much of it stopped damage.
        - ``expired`` (*float*) -- How much went unspent.
        - ``hits`` (*int*) -- Damage instances it absorbed.
    """
    demo.load("player_ticks", "damage")
    rows: list[dict] = []
    hurt_by_hero = {
        int(frame["victim_hero_id"][0]): set(frame["tick"].to_list())
        for frame in demo.damage.select("victim_hero_id", "tick").partition_by(
            "victim_hero_id", maintain_order=False
        )
    }
    pools = (
        demo.player_ticks.select("hero_id", "tick", "barrier")
        .sort("hero_id", "tick")
        .partition_by("hero_id", maintain_order=True)
    )
    for pool in pools:
        hero = int(pool["hero_id"][0])
        rows.extend(_barriers_for_hero(pool, hero, hurt_by_hero.get(hero, set())))
    rows.sort(key=lambda r: r["tick"])
    return pl.DataFrame(
        rows,
        schema={
            "tick": pl.Int64,
            "hero_id": pl.Int64,
            "granted": pl.Float64,
            "absorbed": pl.Float64,
            "expired": pl.Float64,
            "hits": pl.Int64,
        },
    )


def _barriers_for_hero(pool: pl.DataFrame, hero: int, hurt: set[int]) -> list[dict]:
    ticks = pool["tick"].to_list()
    values = pool["barrier"].to_list()

    out: list[dict] = []
    standing: list[dict] = []
    for i in range(1, len(ticks)):
        delta = values[i] - values[i - 1]
        if delta < -0.5 and standing:
            _draw_down(standing, -delta, absorbed=_took_damage(ticks[i], hurt))
            standing = [b for b in standing if b["_left"] > 0.5]
        elif delta > _RISE:
            barrier = {
                "tick": ticks[i],
                "hero_id": hero,
                "granted": delta,
                "absorbed": 0.0,
                "expired": 0.0,
                "hits": 0,
                "_left": delta,
            }
            out.append(barrier)
            standing.append(barrier)

    for barrier in standing:  # never came back down before the match ended
        barrier["expired"] += barrier["_left"]
    for barrier in out:
        barrier.pop("_left")
    return out


def _draw_down(standing: list[dict], amount: float, *, absorbed: bool) -> None:
    total = sum(b["_left"] for b in standing)
    if total <= 0:
        return
    for barrier in standing:
        share = min(amount * (barrier["_left"] / total), barrier["_left"])
        barrier["_left"] -= share
        if absorbed:
            barrier["absorbed"] += share
            barrier["hits"] += 1
        else:
            barrier["expired"] += share


def _took_damage(tick: int, hurt: set[int]) -> bool:
    """Damage rows and pool samples can land a tick apart, so allow +/-1."""
    return tick in hurt or (tick - 1) in hurt or (tick + 1) in hurt
