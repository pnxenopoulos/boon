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
  <a href="https://www.python.org/downloads/"><img src="https://img.shields.io/pypi/pyversions/boon-deadlock?style=for-the-badge" alt="Python 3.11–3.15"></a>
</p>

</div>

```python
from boon import Demo

demo = Demo("match.dem", preload=False)
demo.load("kills", "damage")
print(demo.players)
print(demo.kills.head())
demo.kills.write_parquet("kills.parquet")
```

Boon parses [Valve Deadlock](https://store.steampowered.com/app/1422450/Deadlock/)
Source 2 `.dem` files with a Rust parser and [PyO3](https://pyo3.rs) Python bindings.
Python results use [Polars](https://pola.rs) DataFrames for filters, groups, joins,
and Apache Parquet output. Optional plots use [matplotlib](https://matplotlib.org).
Install `boon-deadlock`; import `boon` in Python or Rust.

**Demo compatibility:** Use Boon **0.10.0 or earlier** for demos recorded before
the **City Never Sleeps** update (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos recorded with that update or later.

## Table of Contents

- [Why Boon?](#why-boon)
- [Installation](#install-boon-for-python-or-rust)
- [Quick Start](#read-deadlock-replay-data-with-boon)
- [Available Datasets](#boon-replay-datasets)
- [Project Structure](#boon-repository-structure)
- [Documentation](#boon-documentation)
- [Useful Links](#useful-links)
- [Contributing](#contributing)
- [License](#license)

## Why Boon?

Boon reads player state, combat events, items, and objectives from Deadlock demos.

- **Rust core:** Read Source 2 demo files with native Python bindings.
- **DataFrames:** Filter, group, and join results with Polars.
- **Dataset selection:** Load compatible datasets in one parser pass.
- **Game data:** Use boon-data catalogs for names and calculated stats. Download optional plotting images.
- **Plots:** Draw match scoreboards, team totals, and player positions with `boon.plot`.
- **CLI:** Inspect demos with the `boon` command.

<a id="installation"></a>

## Install Boon for Python or Rust

Use Boon as a Python library, Rust crate, or CLI.

### Python

Install with [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

Or use pip:

```bash
pip install boon-deadlock
```

Use Python 3.11–3.15.

### CLI

`pip install boon-deadlock` or `uv add boon-deadlock` adds the `boon` command to your PATH. See the [CLI documentation](https://boon.readthedocs.io/en/latest/cli.html). The repository also contains the low-level `boon-dev` debug tool. Build it with `cargo build --release -p boon-dev`.

Download catalogs and optional images from a boon-data release:

```bash
boon get --images all
# Or select groups: --images map --images heroes --images items --images icons
```

### Plotting

Install `uv add 'boon-deadlock[plot]'` or `pip install 'boon-deadlock[plot]'`.

```python
from boon import Demo, plot

demo = Demo("match.dem", preload=False)
fig, ax = plot.summary(demo, version="6795", builds=True)
fig.savefig("summary.png")
fig, ax = plot.timeline(demo, "net_worth")
fig.savefig("souls.png")
fig, ax = plot.positions(demo, tick=10000, version="6795", ziplines=True)
fig.savefig("positions.png")
```

Use a boon-data version with images and map bounds for position plots.
See the [plotting guide](https://boon.readthedocs.io/en/latest/plot.html).

### Rust library

```toml
[dependencies]
boon-deadlock = "0.19"
```

Rust callers can load stat and state catalogs from JSON bytes with
`StatCatalog::from_json_bytes` and `StateCatalog::from_json_bytes`.
These constructors validate the catalogs without filesystem or network access.
Use `Parser::calculate_stats` with a `StatBatch` to query hero stats, ability
stats and imbues in one replay pass. Each query keeps its own selections.
See [the Rust example](crates/boon/examples/stat_batch.rs).

Use `boon::replay_inputs::UpgradeReader` for current upgrade tokens and
`EnemyMinimap` for recorded enemy icons. Call `EnemyMinimap::update(ctx, emit)`
at every completed tick. Set `emit=false` between selected ticks to keep
last-seen history.

Boon uses pbdems2 0.4.1. Resolve field keys from the current schema; do not
store numeric field keys across parser upgrades.

<a id="quick-start"></a>

## Read Deadlock replay data with Boon

### Python

```python
from boon import Demo

demo = Demo("match.dem", preload=False)
print(demo.match_id, demo.map_name, demo.total_clock_time)
print(demo.winning_team_num)

players = demo.snapshots("players", ticks=[10000, 20000])
print(players.select("tick", "steam_id", "hero_id", "health"))
print(demo.kill_participation())
```

Use `calculate_hero_stats()` for hero values, `calculate_ability_stats()` for
ability bonuses, `imbues()` for item selections, and `player_states()` for state
names. Select a boon-data version from `boon versions`.
Use `get_item_purchases(data_version=...)` for catalog-specific item upgrade links.
See the [feature examples](crates/boon-python/docs/examples.md#query-boon-stats-states-and-ammo),
[hero stat names](crates/boon-python/docs/hero-stats.md), and
[ability stat names](crates/boon-python/docs/ability-stats.md#boon-ability-bonus-names-and-v1-rules).
The guides list accepted strings and enum members.
See [enemy minimap icons](crates/boon-python/docs/examples.md#read-boon-enemy-minimap-icons)
for team visibility and last-seen positions. Player snapshots also include
current `upgrade_ids` and an optional recorded `boon_count`.

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

<a id="available-datasets"></a>

## Boon replay datasets

Each dataset is a `Demo` property that returns a [Polars](https://pola.rs) DataFrame. `Demo(path)` preloads kills, damage, and abilities together. Other datasets load on first access. Use `Demo(path, preload=False)` for metadata-only construction, then `load()` to request several datasets together. Call `Demo.available_datasets()` to get the full list.

| Dataset | Description |
|---------|-------------|
| `player_ticks` | State for each player at each tick (position, health, souls, net worth, kills, deaths, assists, 40+ fields) |
| `world_ticks` | World state every tick (pause state, next mid boss spawn) |
| `kills` | Hero kill events with attacker, victim, and assisters |
| `damage` | Damage events with mitigation, hitgroups, source metadata, and light/heavy/other melee classification |
| `item_purchases` | Recorded item changes and matched upgrade components |
| `ability_upgrades` | Hero ability point spending (tier 1-3) |
| `ability_ticks` | Ability cooldown, charge, and slot state changes |
| `abilities` | Important ability usage events |
| `flex_slots` | Flex slot unlock events for each team |
| `chat` | In-game chat messages (all chat and team chat) |
| `objectives` | Objective health state changes (walkers, barracks, shrines, patron, mid boss) with position and phase tracking |
| `mid_boss` | Mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire) |
| `zipline_nodes` | Initial zipline geometry and later revisions, with full entity handles |
| `zipline_changes` | Recorded zipline ownership and visibility changes |
| `troopers` | Alive lane trooper state for each tick with position *(opt-in, large)* |
| `neutrals` | Neutral creep state changes with change detection *(opt-in)* |
| `breakables` | Breakable map-prop destruction events with resolved subclass and last-known position *(opt-in)* |
| `sinners_sacrifice` | Sinner's Sacrifice machine lifecycle and exact hit attribution *(opt-in)* |
| `stat_modifier_events` | Permanent stat bonus change events from pickups *(opt-in)* |
| `active_modifiers` | Active buff/debuff modifier events *(opt-in)* |
| `urn` | Urn lifecycle and delivery point events *(opt-in)* |
| `rift` | Rift (Koth) lifecycle, capture/expiry, winner, lane, and position *(opt-in)* |
| `street_brawl_ticks` | Street Brawl state for each tick *(street brawl only)* |
| `street_brawl_rounds` | Street brawl round scoring events *(street brawl only)* |

<a id="project-structure"></a>

## Boon repository structure

| Crate | Description |
|-------|-------------|
| [`boon`](crates/boon) | Core parser library (published as `boon-deadlock` on crates.io) |
| [`boon-proto`](crates/boon-proto) | Auto-generated Deadlock protobuf definitions |
| [`boon-dev`](crates/boon-dev) | Low-level developer / debugging CLI (in-repo only, not published) |
| [`boon-python`](crates/boon-python) | Python bindings that use PyO3 (published as `boon-deadlock` on PyPI) |

<a id="documentation"></a>

## Boon documentation

Recorded zipline routes and ownership are available through `demo.zipline_nodes`,
`demo.zipline_changes`, and `demo.snapshots("ziplines", ticks=...)`.
See the [zipline guide](https://boon.readthedocs.io/en/latest/ziplines.html).

See [Visual Design](DESIGN.md) for the plot style guide.

The [llms.txt index](https://boon.readthedocs.io/llms.txt) links to plain-text guides and API contracts.

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
