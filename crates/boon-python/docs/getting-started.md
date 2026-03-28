# 🚀 Getting Started

## Requirements

- Python 3.11+
- Rust toolchain (for building from source)

## Installation

We recommend using [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

If you don't use uv, pip works too:

```bash
pip install boon-deadlock
```

Boon is a Rust library with Python bindings built using [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs).

## Quick Start

The `Demo` class is the entrypoint for all parsing. Pass it a path to a `.dem` file, then access the datasets you need as properties. Each property is parsed on first access, so you only pay for what you use. If you need several datasets at once, `load()` parses them in a single pass. Call `Demo.available_datasets()` to see all dataset names.

Most properties return [Polars](https://pola.rs) DataFrames, so you get the full Polars API for filtering, grouping, and analysis out of the box.

```python
from boon import Demo

demo = Demo("match.dem")

# Metadata properties (not DataFrames)
print(demo.map_name)         # "dl_midtown"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.match_id)         # 28309863

# Dataset properties return Polars DataFrames
players = demo.players
print(players)
# shape: (12, 5)
# ┌─────────────┬───────────────┬─────────┬──────────┬────────────┐
# │ player_name ┆ steam_id      ┆ hero_id ┆ team_num ┆ start_lane │
# │ ---         ┆ ---           ┆ ---     ┆ ---      ┆ ---        │
# │ str         ┆ u64           ┆ i64     ┆ i64      ┆ i64        │
# ╞═════════════╪═══════════════╪═════════╪══════════╪════════════╡
# │ Player1     ┆ 7656119...    ┆ 13      ┆ 2        ┆ 1          │
# │ ...         ┆ ...           ┆ ...     ┆ ...      ┆ ...        │
# └─────────────┴───────────────┴─────────┴──────────┴────────────┘

# Batch-load multiple datasets in a single parse pass
demo.load("kills", "damage", "item_purchases", "ability_upgrades")

# Cached — no additional parsing needed
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
# Objective health state changes (walkers, titans, barracks, mid boss)
objectives = demo.objectives

# Objective destruction events
boss_kills = demo.boss_kills

# Mid boss lifecycle (spawn, kill, rejuv buffs)
mid_boss = demo.mid_boss

# Lane troopers and guardians (opt-in, large dataset)
troopers = demo.troopers
# trooper_type is "trooper" (lane creeps) or "trooper_boss" (lane guardian)
```

## Filtering with Polars

Boon returns [Polars](https://pola.rs) DataFrames, so you can use the full Polars
API for filtering, grouping, and analysis:

```python
import polars as pl

# Get a single player's data
haze = player_ticks.filter(pl.col("hero_id") == 13)

# Health over time
haze.select("tick", "health", "max_health")

# Net worth at end of game
final_tick = player_ticks.filter(pl.col("tick") == player_ticks["tick"].max())
final_tick.select("hero_id", "gold_net_worth", "ap_net_worth", "kills", "deaths", "assists")
```

## Error Handling

Boon raises its own exceptions for invalid or malformed demo files. See the {ref}`Exceptions <exceptions>` section of the API reference for the full list.
