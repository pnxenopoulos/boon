# 🚀 Getting Started

## Requirements

- Python 3.11–3.14
- Rust toolchain (for building from source)

## Installation

Install with [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

Or use pip:

```bash
pip install boon-deadlock
```

Boon is a Rust library. Its Python bindings use [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs).

## Quick Start

Give `Demo` the path to a `.dem` file. It loads kills, damage, and abilities.
Other datasets load on first access. Use `preload=False` to defer all dataset loads.
Use `load()` to read compatible datasets in one pass.
Most results are [Polars](https://pola.rs) DataFrames.

```python
from boon import Demo

demo = Demo("match.dem")

# Read metadata.
print(demo.map_name)
print(demo.total_ticks)
print(demo.total_clock_time)
print(demo.match_id)

# Get a dataset as a Polars DataFrame.
players = demo.players
print(players)
# List dataset names.
print(Demo.available_datasets())

# Load additional datasets together; existing frames are cached.
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
print(player_ticks.shape)    # Row counts depend on the recorded pawns and ticks.
print(player_ticks.columns)  # ['tick', 'steam_id', 'ammo_fraction', 'hero_id', ...]
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

Select players by Steam ID. Include `tick` when you join sampled player rows:

```python
import polars as pl

# Select one account, across hero changes.
steam_id = demo.players["steam_id"][0]
player = player_ticks.filter(pl.col("steam_id") == steam_id)

# Health over time
player.select("tick", "hero_id", "health", "max_health")

# Net worth at end of game
final_tick = player_ticks.filter(pl.col("tick") == player_ticks["tick"].max())
final_tick.select("hero_id", "gold_net_worth", "ap_net_worth", "kills", "deaths", "assists")
```

## Stats, states, and imbues

Use `calculate_hero_stats()` for supported hero values and `calculate_ability_stats()` for ability bonus percentages.
Use `imbues()` for item selections and `player_states()` for recorded state names. Each method uses a selected boon-data client
version. List versions with `boon versions` and install one with `boon get VERSION`.
A query downloads a missing version.

Start with the [feature examples](examples.md#stats-states-and-ammo).
The [hero stat table](hero-stats.md) and [ability stat table](ability-stats.md#percentage-rules)
list all accepted strings and enum members. Join player rows with `steam_id`;
include `tick` when you join sampled results.

## Item upgrade links

`demo.item_purchases` uses the newest installed catalog, or downloads latest if none is installed.
Use `demo.get_item_purchases(data_version="6712")` to select an exact version.
The `upgraded_from_ability_ids` column lists matched component sales. Recorded `change` values do not change.
See [item upgrades](examples.md#item-upgrades) for a complete query.

## Error Handling

Boon raises specific exceptions for invalid demo files. See {ref}`Exceptions <exceptions>` for the full list.
