<div align="center">

# Boon

[![Discord](https://img.shields.io/discord/1466262096479129673?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/WmjZHxWrCD)
[![Docs](https://readthedocs.org/projects/boon/badge/?version=latest)](https://boon.readthedocs.io)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

**Python** &nbsp;
[![PyPI](https://img.shields.io/pypi/v/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/boon-deadlock?period=total&units=international_system&left_color=grey&right_color=blue&left_text=PyPI%20Downloads)](https://pepy.tech/project/boon-deadlock)
[![Python 3.11–3.14](https://img.shields.io/badge/python-3.11%E2%80%933.14-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)

**Rust** &nbsp;
[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)

</div>

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo parser. Its Rust core has native Python bindings. Boon returns [Polars](https://pola.rs) DataFrames.

**Demo compatibility:** Use Boon **0.10.0 or earlier** for demos recorded before
the **City Never Sleeps** update (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos recorded with that update or later.

## Installation

Install with [uv](https://docs.astral.sh/uv/):

```bash
uv add boon-deadlock
```

You can also use pip:

```bash
pip install boon-deadlock
```

Use Python 3.11–3.14.

## Quick Start

```python
from boon import Demo

demo = Demo("match.dem")  # preloads kills, damage, and abilities
# Use Demo("match.dem", preload=False) for metadata-only construction.

# Match metadata
print(demo.match_id)
print(demo.map_name)
print(demo.total_ticks)
print(demo.total_clock_time)
print(demo.winning_team_num)

# Name lookups; no Demo instance is necessary.
from boon import (
    ability_display_names, ability_names, hero_names, modifier_names, team_names,
)

print(hero_names(version="6712"))
print(team_names())      # {1: "Spectator", 2: "Hidden King", 3: "Archmother"}
print(ability_names(version="6712"))
print(modifier_names(version="6712"))
print(ability_display_names(version="6712"))

# Player info
print(demo.players)
# shape: (12, 6)
# ┌─────────────┬──────────────┬─────────┬──────────┬────────────┬──────┐
# │ player_name ┆ steam_id     ┆ hero_id ┆ team_num ┆ start_lane ┆ rank │
# ...

# Datasets (combat is preloaded; other frames load on first access)
player_ticks     = demo.player_ticks      # per-player state every tick
world_ticks      = demo.world_ticks       # world state every tick
kills            = demo.kills             # kill events
damage           = demo.damage            # damage events, including melee classification
item_purchases   = demo.item_purchases    # item shop transactions
ability_upgrades = demo.ability_upgrades  # skill point spending
ability_ticks    = demo.ability_ticks     # cooldown, charge, and slot changes
abilities        = demo.abilities         # ability usage events
flex_slots       = demo.flex_slots        # flex slot unlocks
chat             = demo.chat              # chat messages
objectives       = demo.objectives        # objective health per tick
mid_boss         = demo.mid_boss          # mid boss lifecycle events
troopers         = demo.troopers          # lane trooper state per tick
neutrals         = demo.neutrals          # neutral creep state changes
breakables       = demo.breakables        # breakable map-prop destruction events
sinners_sacrifice = demo.sinners_sacrifice  # Sinner's Sacrifice machine hits
stat_modifier_events = demo.stat_modifier_events  # permanent stat bonus change events
active_modifiers = demo.active_modifiers  # buff/debuff modifier events
urn              = demo.urn               # urn lifecycle and delivery events
rift             = demo.rift              # rift (koth) lifecycle, one row per rift
```

## CLI

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

## Features

- Parse Deadlock `.dem` files at native speed with Rust
- 22 built-in datasets covering players, combat, economy, objectives, and map state
- Access to match metadata, player info, entity state, game events, and post-match summaries
- All data returned as [Polars](https://pola.rs) DataFrames
- Bundled `boon` command-line tool for quick demo inspection

Use `calculate_hero_stats()`, `calculate_ability_stats()`, `player_states()`, and `imbues()` with a matching boon-data client version.
Use `get_item_purchases(data_version=...)` for item upgrade links.
See the [examples](https://boon.readthedocs.io/en/latest/examples.html) for complete queries.

## Documentation

Full documentation is available at [boon.readthedocs.io](https://boon.readthedocs.io).

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
