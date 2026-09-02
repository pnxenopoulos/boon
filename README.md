<div align="center">

# Boon

<p>
  <a href="https://discord.gg/WmjZHxWrCD"><img src="https://img.shields.io/discord/1466262096479129673?color=5865F2&logo=discord&logoColor=white&style=for-the-badge" alt="Discord"></a>
  <a href="https://boon.readthedocs.io"><img src="https://readthedocs.org/projects/boon/badge/?version=latest&style=for-the-badge" alt="Docs"></a>
  <a href="https://github.com/pnxenopoulos/boon/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pnxenopoulos/boon/ci.yml?style=for-the-badge" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge" alt="License: MIT"></a>
</p>

<p>
  <a href="https://pypi.org/project/boon-deadlock/"><img src="https://img.shields.io/pypi/v/boon-deadlock.svg?style=for-the-badge" alt="PyPI"></a>
  <a href="https://pepy.tech/project/boon-deadlock"><img src="https://img.shields.io/pepy/dt/boon-deadlock?style=for-the-badge" alt="Downloads"></a>
  <a href="https://www.python.org/downloads/"><img src="https://img.shields.io/pypi/pyversions/boon-deadlock?style=for-the-badge" alt="Python 3.11+"></a>
</p>

<!-- <p>
  <a href="https://crates.io/crates/boon-deadlock"><img src="https://flat.badgen.net/crates/v/boon-deadlock?color=orange" alt="crates.io"></a>
  <a href="https://crates.io/crates/boon-deadlock"><img src="https://flat.badgen.net/crates/d/boon-deadlock" alt="crates.io Downloads"></a>
</p>

<p>
  <a href="https://github.com/pnxenopoulos/boon/releases"><img src="https://img.shields.io/github/v/release/pnxenopoulos/boon?style=for-the-badge" alt="GitHub Release"></a>
  <a href="https://github.com/pnxenopoulos/boon/releases"><img src="https://img.shields.io/github/downloads/pnxenopoulos/boon/total?style=for-the-badge" alt="CLI Downloads"></a>
</p> -->

</div>

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo parser. The Rust core has native Python bindings. Boon reads Source 2 `.dem` files and returns [Polars](https://pola.rs) DataFrames.

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

Deadlock demos contain player positions, kills, damage, item builds, objective state, and other match data. The Source 2 demo format is complex and undocumented. Boon handles the format so that you can analyze structured data.

- ⚡ **Fast.** The core parser is written in Rust. Parsing a full match takes seconds, not minutes.
- 📊 **Structured output.** Each dataset is a Polars DataFrame. You can filter, group, join, and display the data.
- 🎯 **Parse only what you need.** Boon loads each dataset on demand. Use `load()` to parse multiple datasets in one pass.
- 🗂️ **Comprehensive.** Player state, combat, economy, objectives, map props, Sinner's Sacrifice, derived stats, buffs/debuffs, urn and Rift tracking, and street brawl scoring.
- 💻 **CLI included.** `pip install boon-deadlock` ships a `boon` command for quick inspection without writing any code.

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

Requires Python 3.11–3.14.

### CLI

`pip install boon-deadlock` or `uv add boon-deadlock` adds the `boon` command to your PATH. See the [CLI documentation](https://boon.readthedocs.io/en/latest/cli.html). The repository also contains the low-level `boon-dev` debug tool. Build it with `cargo build --release -p boon-dev`.

### Rust library

```toml
[dependencies]
boon-deadlock = "0.8"
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

# Derived stats (boon.stats), also exposed as Demo methods
demo.kill_participation()    # (kills + assists) / team kills, per player
```

### CLI

Bundled with the Python package (`pip install boon-deadlock`):

```bash
# Match metadata
boon info match.dem

# Player roster
boon players match.dem

# Any dataset as a table (add --json for machine-readable output)
boon show match.dem kills --limit 20

# Post-match summary
boon summary match.dem

# All available commands
boon --help
```

The `boon-dev` tool adds low-level commands such as `entities`, `events`, and `send-tables`. Build it with `cargo build --release -p boon-dev`. See the [CLI reference](https://boon.readthedocs.io/en/latest/cli.html).

## Available Datasets

Each dataset is a `Demo` property that returns a [Polars](https://pola.rs) DataFrame. Boon loads a dataset when you first access it. Use `load()` to parse multiple datasets in one pass. Call `Demo.available_datasets()` to get the full list.

| Dataset | Description |
|---------|-------------|
| `player_ticks` | Per-player state every tick (position, health, souls, net worth, kills, deaths, assists, 40+ fields) |
| `world_ticks` | World state every tick (pause state, next mid boss spawn) |
| `kills` | Hero kill events with attacker, victim, and assisters |
| `damage` | Damage events with mitigation, hitgroups, source metadata, and light/heavy/other melee classification |
| `item_purchases` | Item shop transactions (purchased, upgraded, sold, swapped, failed) |
| `ability_upgrades` | Hero ability point spending (tier 1-3) |
| `ability_ticks` | Ability cooldown, charge, and slot state changes |
| `abilities` | Important ability usage events |
| `flex_slots` | Flex slot unlock events per team |
| `chat` | In-game chat messages (all chat and team chat) |
| `objectives` | Objective health state changes (walkers, barracks, shrines, patron, mid boss) with position and phase tracking |
| `mid_boss` | Mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire) |
| `troopers` | Per-tick alive lane trooper state with position *(opt-in, large)* |
| `neutrals` | Neutral creep state changes with change detection *(opt-in)* |
| `breakables` | Breakable map-prop destruction events with resolved subclass and last-known position *(opt-in)* |
| `sinners_sacrifice` | Sinner's Sacrifice machine lifecycle and exact hit attribution *(opt-in)* |
| `stat_modifier_events` | Permanent stat bonus change events from pickups *(opt-in)* |
| `active_modifiers` | Active buff/debuff modifier events *(opt-in)* |
| `urn` | Urn lifecycle and delivery point events *(opt-in)* |
| `rift` | Rift (Koth) lifecycle, capture/expiry, winner, lane, and position *(opt-in)* |
| `street_brawl_ticks` | Per-tick street brawl state *(street brawl only)* |
| `street_brawl_rounds` | Street brawl round scoring events *(street brawl only)* |

## Project Structure

| Crate | Description |
|-------|-------------|
| [`boon`](crates/boon) | Core parser library (published as `boon-deadlock` on crates.io) |
| [`boon-proto`](crates/boon-proto) | Auto-generated Deadlock protobuf definitions |
| [`boon-dev`](crates/boon-dev) | Low-level developer / debugging CLI (in-repo only, not published) |
| [`boon-python`](crates/boon-python) | Python bindings that use PyO3 (published as `boon-deadlock` on PyPI) |

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
- [deadlock.nyc](https://deadlock.nyc) — an online demo parser powered by Boon

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, coding standards, and submission instructions.

## License

MIT — see [LICENSE](LICENSE) for details.
