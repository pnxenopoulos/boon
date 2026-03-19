# Getting Started

## Requirements

- Python 3.11+
- Rust toolchain (for building from source)

## Installation

Boon is a Rust library with Python bindings built using [PyO3](https://pyo3.rs) and
[maturin](https://www.maturin.rs). Install it using [uv](https://docs.astral.sh/uv/guides/install-python/) or pip:

```bash
uv add boon-deadlock

# or

pip install boon-deadlock
```

## Quick Start

```python
from boon import Demo

# Open a demo file
demo = Demo("match.dem")

# Inspect metadata
print(demo.map_name)         # "dl_midtown"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.match_id)         # 28309863

# Get player info
players = demo.players
print(players)
# shape: (12, 7)
# ┌─────────────┬───────────────┬───────────┬─────────┬──────────────┬──────────┬────────────┐
# │ player_name ┆ steam_id      ┆ hero      ┆ hero_id ┆ team         ┆ team_num ┆ start_lane │
# │ ---         ┆ ---           ┆ ---       ┆ ---     ┆ ---          ┆ ---      ┆ ---        │
# │ str         ┆ u64           ┆ str       ┆ i64     ┆ str          ┆ i64      ┆ i64        │
# ╞═════════════╪═══════════════╪═══════════╪═════════╪══════════════╪══════════╪════════════╡
# │ Player1     ┆ 7656119...    ┆ Haze      ┆ 13      ┆ Hidden King  ┆ 2        ┆ 1          │
# │ ...         ┆ ...           ┆ ...       ┆ ...     ┆ ...          ┆ ...      ┆ ...        │
# └─────────────┴───────────────┴───────────┴─────────┴──────────────┴──────────┴────────────┘
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
shop_events = demo.shop_events

# Ability point spending
ability_upgrades = demo.ability_upgrades

# Chat messages
chat = demo.chat
```

## Objectives and Map State

```python
# Objective health per tick (walkers, titans, barracks, mid boss)
objectives = demo.objectives

# Objective destruction events
boss_kills = demo.boss_kills

# Mid boss lifecycle (spawn, kill, rejuv buffs)
mid_boss = demo.mid_boss
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

```python
from boon import Demo, InvalidDemoError, DemoHeaderError, DemoInfoError, DemoMessageError

try:
    demo = Demo("match.dem")
except FileNotFoundError:
    print("File does not exist")
except InvalidDemoError:
    print("Not a valid demo file")
except DemoHeaderError:
    print("Demo header is missing required fields")
except DemoInfoError:
    print("Demo file info is missing required fields")
except DemoMessageError:
    print("Could not resolve match data from demo")
```
