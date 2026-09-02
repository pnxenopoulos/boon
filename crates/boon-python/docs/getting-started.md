# 🚀 Getting Started

## Requirements

- Python 3.11+
- Rust toolchain (for building from source)

## Installation

We recommend using [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

If you do not use uv, use pip:

```bash
pip install boon-deadlock
```

Boon is a Rust library. Its Python bindings use [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs).

## Quick Start

The `Demo` class is the entry point for parsing. Give it the path to a `.dem`
file. Then, access the required datasets as properties. Boon parses a property
on first access. Use `load()` to parse multiple datasets in one pass. Call
`Demo.available_datasets()` to get all dataset names.

Most properties return [Polars](https://pola.rs) DataFrames. Use the Polars API
to filter, group, and analyze the data.

```python
from boon import Demo

demo = Demo("match.dem")

# Read metadata.
print(demo.map_name)         # "dl_midtown"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.match_id)         # 28309863

# Get a dataset as a Polars DataFrame.
players = demo.players
print(players)
# shape: (12, 6)
# ┌─────────────┬───────────────┬─────────┬──────────┬────────────┬──────┐
# │ player_name ┆ steam_id      ┆ hero_id ┆ team_num ┆ start_lane ┆ rank │
# │ ---         ┆ ---           ┆ ---     ┆ ---      ┆ ---        ┆ ---  │
# │ str         ┆ u64           ┆ i64     ┆ i64      ┆ i64        ┆ i64  │
# ╞═════════════╪═══════════════╪═════════╪══════════╪════════════╪══════╡
# │ Player1     ┆ 7656119...    ┆ 13      ┆ 2        ┆ 1          ┆ 61   │
# │ ...         ┆ ...           ┆ ...     ┆ ...      ┆ ...        ┆ ...  │
# └─────────────┴───────────────┴─────────┴──────────┴────────────┴──────┘

# Load multiple datasets in one pass.
demo.load("kills", "damage", "item_purchases", "ability_upgrades")

# Boon uses the cached data.
print(f"Kills: {len(demo.kills)}")
print(f"Damage events: {len(demo.damage)}")
```

## Working with Tick Data

```python
# World state per tick
world = demo.world_ticks
print(world.columns)  # ['tick', 'is_paused', 'next_midboss']

# Player state per tick (one row per player per tick)
player_ticks = demo.player_ticks
print(player_ticks.shape)    # (648000, 50) — 12 players × 54000 ticks
print(player_ticks.columns)  # ['tick', 'hero_id', 'x', 'y', 'z', ...]
```

## Events and Economy

```python
# Kill events
kills = demo.kills

# Damage events
damage = demo.damage

# Item shop transactions
item_purchases = demo.item_purchases

# Ability point spending
ability_upgrades = demo.ability_upgrades

# Chat messages
chat = demo.chat
```

## Objectives and Map State

```python
# Objective health state changes (walkers, barracks, shrines, patron, mid boss)
objectives = demo.objectives

# Mid boss lifecycle (spawn, kill, rejuv buffs)
mid_boss = demo.mid_boss

# Get one row per Rift, with the winner and lane.
rift = demo.rift

# Lane troopers and guardians (opt-in, large dataset)
troopers = demo.troopers
# "trooper" is a lane creep. "trooper_boss" is a lane guardian.
```

## Filtering with Polars

Boon returns [Polars](https://pola.rs) DataFrames. Use the Polars API to filter,
group, and analyze the data:

```python
import polars as pl

# Select one player's data.
haze = player_ticks.filter(pl.col("hero_id") == 13)

# Health over time
haze.select("tick", "health", "max_health")

# Net worth at end of game
final_tick = player_ticks.filter(pl.col("tick") == player_ticks["tick"].max())
final_tick.select("hero_id", "gold_net_worth", "ap_net_worth", "kills", "deaths", "assists")
```

## Error Handling

Boon raises specific exceptions for invalid demo files. See {ref}`Exceptions <exceptions>` for the full list.
