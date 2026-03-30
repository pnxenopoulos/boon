<div align="center">

# Boon

[![Discord](https://img.shields.io/discord/1466262096479129673?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/WmjZHxWrCD)
[![Docs](https://readthedocs.org/projects/boon/badge/?version=latest)](https://boon.readthedocs.io)
[![CI](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml/badge.svg)](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Python** &nbsp;
[![PyPI](https://img.shields.io/pypi/v/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)
[![Downloads](https://static.pepy.tech/personalized-badge/boon-deadlock?period=total&units=international_system&left_color=grey&right_color=blue&left_text=PyPI%20Downloads)](https://pepy.tech/project/boon-deadlock)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)

**Rust** &nbsp;
[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)

**CLI** &nbsp;
[![GitHub Release](https://img.shields.io/github/v/release/pnxenopoulos/boon?label=CLI)](https://github.com/pnxenopoulos/boon/releases)
[![CLI Downloads](https://img.shields.io/github/downloads/pnxenopoulos/boon/total?label=CLI%20Downloads)](https://github.com/pnxenopoulos/boon/releases)

</div>

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo / replay parser written in Rust with native Python bindings. It parses Source 2 demo files (`.dem`) and returns [Polars](https://pola.rs) DataFrames, giving you structured access to match data without dealing with the binary format yourself.

## Table of Contents

- [Why Boon?](#why-boon)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Available Datasets](#available-datasets)
- [Project Structure](#project-structure)
- [Documentation](#documentation)
- [Useful Links](#useful-links)
- [Contributing](#contributing)
- [License](#license)

## Why Boon?

Deadlock demo files contain a wealth of match data — player positions, kills, damage, item builds, objective state, and more — but the Source 2 demo format is complex and undocumented. Boon handles the low-level parsing so you can focus on analysis.

- ⚡ **Fast.** The core parser is written in Rust. Parsing a full match takes seconds, not minutes.
- 📊 **Structured output.** Every dataset is a Polars DataFrame, ready for filtering, grouping, joins, and visualization.
- 🎯 **Parse only what you need.** Each dataset is loaded on demand. Request one property and Boon skips everything else. Batch multiple datasets with `load()` to share a single parse pass.
- 🗂️ **Comprehensive.** Player state, kills, damage, item purchases, ability upgrades, objectives, chat, lane troopers, neutral creeps, buffs/debuffs, urn tracking, and street brawl scoring.
- 💻 **CLI included.** A standalone command-line tool for quick inspection without writing any code.

## Installation

Boon can be used as a Python library, a Rust crate, or a standalone CLI tool.

### Python

We recommend using [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

You can also use pip:

```bash
pip install boon-deadlock
```

Requires Python 3.11+.

### CLI

Download a prebuilt binary from the [GitHub Releases](https://github.com/pnxenopoulos/boon/releases) page.

### Rust library

```toml
[dependencies]
boon-deadlock = "0.1"
```

## Quick Start

### Python

```python
from boon import Demo

demo = Demo("match.dem")

# Match metadata
print(demo.match_id)         # 70555151
print(demo.map_name)         # "start"
print(demo.total_clock_time) # "37:38"
print(demo.winning_team_num) # 3

# Datasets are Polars DataFrames, lazy-loaded on first access
kills = demo.kills
damage = demo.damage
player_ticks = demo.player_ticks

# Batch-load multiple datasets in a single parse pass
demo.load("kills", "damage", "player_ticks", "objectives")

# See what datasets are available
Demo.available_datasets()
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

## Available Datasets

Each dataset is a property on the `Demo` class that returns a [Polars](https://pola.rs) DataFrame. Datasets are lazy-loaded on first access — boon only parses what you request. If you need multiple datasets, `load()` parses them in a single pass for efficiency. Call `Demo.available_datasets()` to see the full list programmatically.

| Dataset | Description |
|---------|-------------|
| `player_ticks` | Per-player state every tick (position, health, souls, net worth, kills, deaths, assists, 40+ fields) |
| `world_ticks` | World state every tick (pause state, next mid boss spawn) |
| `kills` | Hero kill events with attacker, victim, and assisters |
| `damage` | Damage events with pre/post mitigation, hitgroups, and crit damage |
| `item_purchases` | Item shop transactions (purchased, upgraded, sold, swapped) |
| `ability_upgrades` | Hero ability point spending (tier 1-3) |
| `abilities` | Important ability usage events |
| `flex_slots` | Flex slot unlock events per team |
| `chat` | In-game chat messages (all chat and team chat) |
| `objectives` | Objective health state changes (walkers, barracks, shrines, patron, mid boss) with position and phase tracking |
| `mid_boss` | Mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire) |
| `troopers` | Per-tick alive lane trooper state with position *(opt-in, large)* |
| `neutrals` | Neutral creep state changes with change detection *(opt-in)* |
| `stat_modifier_events` | Permanent stat bonus change events from pickups *(opt-in)* |
| `active_modifiers` | Active buff/debuff modifier events *(opt-in)* |
| `urn` | Urn (idol) lifecycle and delivery point events *(opt-in)* |
| `street_brawl_ticks` | Per-tick street brawl state *(street brawl only)* |
| `street_brawl_rounds` | Street brawl round scoring events *(street brawl only)* |

## Project Structure

| Crate | Description |
|-------|-------------|
| [`boon`](crates/boon) | Core parser library (published as `boon-deadlock` on crates.io) |
| [`boon-proto`](crates/boon-proto) | Auto-generated Deadlock protobuf definitions |
| [`boon-cli`](crates/boon-cli) | Command-line interface |
| [`boon-python`](crates/boon-python) | Python bindings via PyO3 (published as `boon-deadlock` on PyPI) |

## Documentation

Full documentation is available at [boon.readthedocs.io](https://boon.readthedocs.io), including:

- [Getting Started](https://boon.readthedocs.io/en/latest/getting-started.html)
- [Examples](https://boon.readthedocs.io/en/latest/examples.html)
- [API Reference](https://boon.readthedocs.io/en/latest/api.html)
- [CLI Reference](https://boon.readthedocs.io/en/latest/cli.html)
- [FAQ](https://boon.readthedocs.io/en/latest/faq.html)
- [Known Issues](https://boon.readthedocs.io/en/latest/known-issues.html)
- [Changelog](https://boon.readthedocs.io/en/latest/changelog.html)

## Useful Links

- [Deadlock](https://www.playdeadlock.com/) — official home page
- [Steam store page](https://store.steampowered.com/app/1422450/Deadlock/)
- [Deadlock Wiki](https://deadlock.wiki/)
- [r/DeadlockTheGame](https://www.reddit.com/r/DeadlockTheGame/) — Reddit community

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and how to submit changes.

## License

MIT — see [LICENSE](LICENSE) for details.
