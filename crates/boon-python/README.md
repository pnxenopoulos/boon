# Boon

[![Discord](https://img.shields.io/discord/868146581419999232?color=5865F2&logo=discord&logoColor=white)](https://discord.gg/tWCwmHDy2u)
[![Docs](https://readthedocs.org/projects/boon/badge/?version=latest)](https://boon.readthedocs.io)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/downloads/)
[![Downloads](https://img.shields.io/pypi/dm/boon-deadlock.svg)](https://pypi.org/project/boon-deadlock/)

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo / replay parser written in Rust with native Python bindings. It returns [Polars](https://pola.rs) DataFrames for easy analysis.

## Installation

```bash
uv add boon-deadlock

# or

pip install boon-deadlock
```

Requires Python 3.11+.

## Quick Start

```python
from boon import Demo

demo = Demo("match.dem")

# Match metadata
print(demo.match_id)         # 28309863
print(demo.map_name)         # "dl_midtown"
print(demo.total_ticks)      # 54000
print(demo.total_clock_time) # "30:00"
print(demo.winning_team_num) # 2

# Name lookups (module-level — no demo required)
from boon import hero_names, team_names, ability_names, modifier_names

print(hero_names())      # {0: "Base", 1: "Infernus", ...}
print(team_names())      # {1: "Spectator", 2: "Hidden King", 3: "Archmother"}
print(ability_names())   # {46922526: "inherent_base", ...}
print(modifier_names())  # {2059539911: "timer", ...}

# Player info
print(demo.players)
# shape: (12, 5)
# ┌─────────────┬──────────────┬─────────┬──────────┬────────────┐
# │ player_name ┆ steam_id     ┆ hero_id ┆ team_num ┆ start_lane │
# ...

# Datasets (Polars DataFrames — all lazy-loaded on first access)
player_ticks     = demo.player_ticks      # per-player state every tick
world_ticks      = demo.world_ticks       # world state every tick
kills            = demo.kills             # kill events
damage           = demo.damage            # damage events
item_purchases   = demo.item_purchases    # item shop transactions
ability_upgrades = demo.ability_upgrades  # skill point spending
abilities        = demo.abilities         # ability usage events
respawns         = demo.respawns          # respawn events
flex_slots       = demo.flex_slots        # flex slot unlocks
chat             = demo.chat              # chat messages
objectives       = demo.objectives        # objective health per tick
boss_kills       = demo.boss_kills        # objective destruction events
mid_boss         = demo.mid_boss          # mid boss lifecycle events
troopers         = demo.troopers          # lane trooper state per tick
neutrals         = demo.neutrals          # neutral creep state changes
stat_modifier_events = demo.stat_modifier_events  # permanent stat bonus change events
active_modifiers = demo.active_modifiers  # buff/debuff modifier events
urn              = demo.urn               # urn lifecycle and delivery events
```

## Features

- Parse Deadlock `.dem` demo files at native speed via Rust
- 18 built-in datasets covering players, combat, economy, objectives, and map state
- Access to match metadata, player info, entity state, game events, and post-match summaries
- All data returned as [Polars](https://pola.rs) DataFrames

## Documentation

Full documentation is available at [boon.readthedocs.io](https://boon.readthedocs.io).

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
