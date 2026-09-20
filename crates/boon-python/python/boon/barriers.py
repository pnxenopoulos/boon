"""Recorded barrier absorption from damage messages."""

from __future__ import annotations

from typing import TYPE_CHECKING

import polars as pl

if TYPE_CHECKING:
    from boon import Demo

__all__ = ["barriers"]


def barriers(demo: Demo) -> pl.DataFrame:
    """Return one row per player-targeted damage message with barrier absorption.

    ``absorbed`` is the recorded amount, including fully blocked hits with zero
    health damage. ``remaining`` and ``capacity`` are the message's shield values;
    they are null when absent. ``ability_id`` identifies the attacking source,
    not the source that granted the barrier. ``server_tick`` is distinct from
    the demo ``tick`` and can be null in older recordings.

    Secondary-stat messages remain visible with ``is_secondary_stat`` unchanged.
    Row counts describe messages, not individual bullets or barrier grants.
    No grant, expiration, or per-grant hit count is inferred from pool changes.
    Only the damage dataset is loaded.
    """
    demo.load("damage")
    return (
        demo.damage.filter(
            (pl.col("victim_hero_id") > 0)
            & pl.col("damage_absorbed").is_finite()
            & (pl.col("damage_absorbed") > 0)
        )
        .select(
            "tick",
            "server_tick",
            pl.col("victim_hero_id").alias("hero_id"),
            "victim_entity_id",
            "attacker_hero_id",
            "ability_id",
            pl.col("damage_absorbed").alias("absorbed"),
            pl.col("victim_shield_new").alias("remaining"),
            pl.col("victim_shield_max").alias("capacity"),
            "is_secondary_stat",
        )
        .sort("tick", maintain_order=True)
    )
