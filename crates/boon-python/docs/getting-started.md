# Getting Started

## Requirements

- Python 3.11+
- Rust toolchain (for building from source)

## Installation

Boon is a Rust library with Python bindings built using [PyO3](https://pyo3.rs) and
[maturin](https://www.maturin.rs). Install it from source:

```bash
# Clone the repository
git clone https://github.com/yourusername/boon.git
cd boon/crates/boon-python

# Install in development mode (editable)
pip install maturin
maturin develop --release

# Or install directly
pip install .
```

If you use [uv](https://docs.astral.sh/uv/):

```bash
cd boon/crates/boon-python
uv sync
uv run maturin develop --release
```

## Quick Start

```python
from boon import Demo

# Open a demo file
demo = Demo("match.dem")

# Inspect metadata
print(demo.map_name)        # "street_test"
print(demo.total_ticks)     # 54000
print(demo.total_clock_time) # "30:00"
print(demo.match_id)        # 28309863

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
