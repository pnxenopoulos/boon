# Boon

[![CI](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml/badge.svg)](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)
[![PyPI](https://img.shields.io/pypi/v/boon.svg)](https://pypi.org/project/boon/)
[![Downloads](https://img.shields.io/pypi/dm/boon.svg)](https://pypi.org/project/boon/)
[![Discord](https://img.shields.io/discord/868146581419999232?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/W34XjsSs2H)

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo / replay parser. It is written in Rust and ships with Python bindings, a CLI tool, and a standalone Rust library.

## Features

- Parse Deadlock `.dem` demo files at native speed
- Python library returning [Polars](https://pola.rs) DataFrames for analysis
- CLI for quick inspection of demo files
- Seven built-in datasets: player ticks, world ticks, kills, damage, flex slots, respawns, and purchases
- Access to match metadata, player info, entity state, game events, and post-match summaries

## Installation

### Python

```bash
pip install boon

# or

uv add boon
```

Requires Python 3.11+. Boon depends on [Polars](https://pola.rs) for DataFrames.

### CLI

Build the CLI from source (requires Rust):

```bash
git clone https://github.com/pnxenopoulos/boon.git
cd boon
cargo build --release -p boon-cli
# Binary is at target/release/boon
```

### Rust library

Add to your `Cargo.toml`:

```toml
[dependencies]
boon = { package = "boon-deadlock", git = "https://github.com/pnxenopoulos/boon.git" }
```

## Quick Start

### Python

```python
from boon import Demo

demo = Demo("match.dem")

# Match metadata
print(demo.match_id)         # 28309863
print(demo.map_name)         # "street_test"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.winner)           # "Team1"

# Player info
print(demo.players)
# shape: (12, 7)
# ┌─────────────┬──────────────┬──────────┬─────────┬─────────────┬──────────┬────────────┐
# │ player_name ┆ steam_id     ┆ hero     ┆ hero_id ┆ team        ┆ team_num ┆ start_lane │
# ...

# Datasets (Polars DataFrames)
player_ticks = demo.player_ticks   # per-player state every tick
world_ticks  = demo.world_ticks    # world state every tick
kills        = demo.kills          # kill events
damage       = demo.damage         # damage events
purchases    = demo.purchases      # item purchases
respawns     = demo.respawns       # respawn events
flex_slots   = demo.flex_slots     # flex slot unlocks
```

### CLI

```bash
# Match metadata
boon info match.dem

# Post-match summary (players, objectives, gold breakdowns)
boon summary match.dem

# Game events
boon events match.dem --summary

# Entity state at a specific tick
boon entities match.dem --tick 10000 --filter CCitadelPlayerController

# All available commands
boon --help
```

## Project Structure

| Crate | Description |
|-------|-------------|
| `boon` | Core parser library (Rust) |
| `boon-proto` | Auto-generated Deadlock protobuf definitions |
| `boon-cli` | Command-line interface |
| `boon-python` | Python bindings via PyO3 |

## Documentation

Full documentation is available at the [Boon docs site](https://github.com/pnxenopoulos/boon/tree/main/crates/boon-python/docs), including:

- [Getting Started](crates/boon-python/docs/getting-started.md)
- [Python API Reference](crates/boon-python/docs/api.md)
- [CLI Reference](crates/boon-python/docs/cli.md)
- [Changelog](crates/boon-python/docs/changelog.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and how to submit changes.

## License

MIT &mdash; see [LICENSE](LICENSE) for details.
