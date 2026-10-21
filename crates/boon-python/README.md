<div align="center">

# Boon

[![Discord](https://img.shields.io/discord/1466262096479129673?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/WmjZHxWrCD)
[![Docs](https://readthedocs.org/projects/boon/badge/?version=latest)](https://boon.readthedocs.io)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

**Python** &nbsp;
[![PyPI](https://img.shields.io/pypi/v/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/boon-deadlock?period=total&units=international_system&left_color=grey&right_color=blue&left_text=PyPI%20Downloads)](https://pepy.tech/project/boon-deadlock)
[![Python 3.11–3.15](https://img.shields.io/badge/python-3.11%E2%80%933.15-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)

**Rust** &nbsp;
[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)

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

<a id="installation"></a>

## Install Boon for Python

Install with [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

You can also use pip:

```bash
pip install boon-deadlock
```

Use Python 3.11–3.15.

<a id="quick-start"></a>

## Read Deadlock player state with Boon

```python
from boon import Demo, hero_names

demo = Demo("match.dem", preload=False)
print(demo.match_id, demo.map_name, demo.total_clock_time)
print(hero_names(version="6712"))

players = demo.snapshots("players", ticks=[10000, 20000])
print(players.select("tick", "steam_id", "hero_id", "health"))
print(Demo.available_datasets())
```

Player snapshots include current `upgrade_ids` and an optional recorded
`boon_count`. Objectives include `entity_serial` to distinguish reuse of an
entity index. See the [API reference](https://boon.readthedocs.io/en/latest/api.html)
and [minimap example](https://boon.readthedocs.io/en/latest/examples.html#read-boon-enemy-minimap-icons).

<a id="cli"></a>

## Inspect Deadlock demos with the Boon CLI

The package adds the `boon` command to your PATH. Use it to inspect a demo
without Python code:

```bash
boon info match.dem                    # match metadata
boon players match.dem                 # roster, with hero names resolved
boon show match.dem kills --limit 20   # any dataset as a table (or --json)
boon summary match.dem                 # post-match summary
boon stats match.dem -m kill-participation
```

Run `boon --help` for all commands. See the [CLI documentation](https://boon.readthedocs.io/en/latest/cli.html).

<a id="plotting"></a>

## Draw Deadlock match plots with Boon

Install `uv add 'boon-deadlock[plot]'` or `pip install 'boon-deadlock[plot]'`.

```python
from boon import Demo, plot

demo = Demo("match.dem", preload=False)
fig, ax = plot.summary(demo, version="6795", builds=True)
fig.savefig("summary.png")
fig, ax = plot.timeline(demo, "net_worth")
fig.savefig("souls.png")
fig, ax = plot.positions(demo, tick=187554, version="6795", ziplines=True)
fig.savefig("positions.png")
```

Image plots use downloaded boon-data assets. Position plots also need map bounds
in that release. See the [plotting guide](https://boon.readthedocs.io/en/latest/plot.html).

<a id="features"></a>

## Boon Python features

- Parse Deadlock `.dem` files at native speed with Rust
- 24 built-in datasets covering players, combat, economy, objectives, and map state
- Access to match metadata, player info, entity state, game events, and post-match summaries
- Dataset results use [Polars](https://pola.rs) DataFrames
- Bundled `boon` command-line tool for quick demo inspection

Use `calculate_hero_stats()`, `calculate_ability_stats()`, `player_states()`, and `imbues()` with a matching boon-data client version.
Use `get_item_purchases(data_version=...)` for item upgrade links.
See the [examples](https://boon.readthedocs.io/en/latest/examples.html) for complete queries.

<a id="documentation"></a>

## Boon Python documentation

The [llms.txt index](https://boon.readthedocs.io/llms.txt) links to plain-text guides and API contracts.

Full documentation is available at [boon.readthedocs.io](https://boon.readthedocs.io).

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
