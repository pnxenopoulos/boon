# Boon

[![Discord](https://img.shields.io/discord/868146581419999232?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/tWCwmHDy2u)
[![Docs](https://readthedocs.org/projects/boon/badge/?version=latest)](https://boon.readthedocs.io)
[![Project](https://img.shields.io/badge/project-board-24292e.svg?logo=github)](https://github.com/users/pnxenopoulos/projects/6)
[![CI](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml/badge.svg)](https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[![Python 3.11+](https://img.shields.io/badge/python-3.11+-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)
[![PyPI](https://img.shields.io/pypi/v/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)
[![Downloads](https://img.shields.io/pypi/dm/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)

[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo / replay parser. It is written in Rust and ships with Python bindings, a CLI tool, and a standalone Rust library.

## Features

- Parse Deadlock `.dem` demo files at native speed
- Python library returning [Polars](https://pola.rs) DataFrames for analysis
- CLI for quick inspection of demo files
- Access to match metadata, player info, entity state, game events, and post-match summaries

## Available Data

The following data can be extracted from demo files:

- **Player ticks** -- per-player state every tick (position, health, souls, net worth, kills, deaths, assists, and 40+ more fields)
- **World ticks** -- world state every tick (pause state, next mid boss spawn time)
- **Kills** -- hero kill events with attacker, victim, and assisters
- **Damage** -- damage events with pre/post mitigation, hitgroups, and crit damage
- **Purchases** -- item purchase/sell notifications
- **Shop events** -- full item shop transactions (purchased, upgraded, sold, swapped, failure)
- **Ability upgrades** -- hero ability point spending (skill tier upgrades T1-T4)
- **Abilities** -- important ability usage events
- **Respawns** -- player respawn events
- **Flex slots** -- flex slot unlock events per team
- **Chat** -- in-game chat messages (all chat and team chat)
- **Objectives** -- per-tick objective entity health (walkers, titans, barracks, mid boss)
- **Boss kills** -- objective destruction events (walkers, titans, barracks, mid boss, core)
- **Mid boss** -- mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire)
- **Troopers** -- per-tick alive lane trooper state (position, health, lane) *(opt-in, large dataset)*
- **Neutrals** -- neutral creep state changes with change detection *(opt-in)*
- **Players** -- player info (name, Steam ID, hero, team, starting lane)
- **Match metadata** -- match ID, map name, build number, tick rate, total ticks/time
- **Game result** -- winning team, game over tick, banned heroes

## Installation

### Python

```bash
uv add boon-deadlock

# or

pip install boon-deadlock
```

Requires Python 3.11+. Boon depends on [Polars](https://pola.rs) for DataFrames.

### CLI

Install a prebuilt binary via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) (no compilation needed):

```bash
cargo binstall boon-cli
```

Or download a binary from the [GitHub Releases](https://github.com/pnxenopoulos/boon/releases) page.

Or build from source (requires Rust):

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
boon-deadlock = "0.1"
```

## Quick Start

### Python

```python
from boon import Demo

demo = Demo("match.dem")

# Match metadata
print(demo.match_id)         # 28309863
print(demo.map_name)         # "dl_midtown"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.winner)           # "Team1"

# Player info
print(demo.players)
# shape: (12, 7)
# ┌─────────────┬──────────────┬──────────┬─────────┬─────────────┬──────────┬────────────┐
# │ player_name ┆ steam_id     ┆ hero     ┆ hero_id ┆ team        ┆ team_num ┆ start_lane │
# ...

# Datasets (Polars DataFrames — all lazy-loaded on first access)
player_ticks     = demo.player_ticks      # per-player state every tick
world_ticks      = demo.world_ticks       # world state every tick
kills            = demo.kills             # kill events
damage           = demo.damage            # damage events
purchases        = demo.purchases         # item purchase notifications
shop_events      = demo.shop_events       # full shop transactions
ability_upgrades = demo.ability_upgrades  # skill point spending
abilities        = demo.abilities         # ability usage events
respawns         = demo.respawns          # respawn events
flex_slots       = demo.flex_slots        # flex slot unlocks
chat             = demo.chat              # chat messages
objectives       = demo.objectives        # objective health per tick
boss_kills       = demo.boss_kills        # objective destruction events
mid_boss         = demo.mid_boss          # mid boss lifecycle events

# Large datasets (opt-in, not loaded by default)
troopers         = demo.troopers          # lane trooper state per tick
neutrals         = demo.neutrals          # neutral creep state changes
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

Full documentation is available at the [Boon docs site](https://boon.readthedocs.io/en/latest/), including:

- [Getting Started](crates/boon-python/docs/getting-started.md)
- [Python API Reference](crates/boon-python/docs/api.md)
- [CLI Reference](crates/boon-python/docs/cli.md)
- [Changelog](crates/boon-python/docs/changelog.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and how to submit changes.

## License

MIT &mdash; see [LICENSE](LICENSE) for details.
