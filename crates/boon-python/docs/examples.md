# Examples

Practical examples showing how to use boon for common Deadlock replay analysis tasks.

## Resolving IDs to names

Boon DataFrames use raw integer IDs for heroes, teams, abilities, and modifiers rather than strings. This keeps the data compact and fast to filter, group, and join on — you never pay for string comparisons in hot loops. It also means the data is stable: IDs don't change if Valve renames a hero or ability.

When you need human-readable names, boon provides module-level mapping functions that return `dict[int, str]`:

```python
from boon import hero_names, team_names, ability_names, modifier_names, game_mode_names

hero_names()       # {1: "Infernus", 2: "Seven", 3: "Vindicta", ...}
team_names()       # {1: "Spectator", 2: "Hidden King", 3: "Archmother"}
ability_names()    # {123456: "Spectral Wall", ...}  (MurmurHash2 IDs)
modifier_names()   # {789012: "modifier_tentacle_debuff", ...}  (MurmurHash2 IDs)
game_mode_names()  # {1: "Unranked", 2: "Ranked", 4: "StreetBrawl", ...}
```

Use these with Polars `replace_strict` to add name columns, or with `dict.get` when iterating rows:

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("match.dem")
heroes = hero_names()

# Add a hero name column to any DataFrame with a hero_id column
players = demo.players.with_columns(
    pl.col("hero_id").replace_strict(heroes, default="Unknown").alias("hero")
)

# Or resolve when iterating
for row in demo.players.iter_rows(named=True):
    print(heroes.get(row["hero_id"], "Unknown"))
```

## Match summary

Print a quick overview of a match: duration, winner, and per-player KDA.

```python
from boon import Demo, hero_names, team_names

demo = Demo("match.dem")

heroes = hero_names()
teams = team_names()

print(f"Match {demo.match_id}")
print(f"Duration: {demo.total_clock_time}")
print(f"Winner: {teams.get(demo.winning_team_num, 'Unknown')}")
print()

players = demo.players
for row in players.iter_rows(named=True):
    name = heroes.get(row["hero_id"], "Unknown")
    team = teams.get(row["team_num"], "Unknown")
    print(f"  {name:<16} ({team})")
```

## Kill feed

Build a kill feed with hero names and timestamps.

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("match.dem")
heroes = hero_names()

kills = demo.kills.sort("tick")
for row in kills.iter_rows(named=True):
    time = demo.tick_to_clock_time(row["tick"])
    attacker = heroes.get(row["attacker_hero_id"], "Unknown")
    victim = heroes.get(row["victim_hero_id"], "Unknown")
    assisters = [heroes.get(a, "?") for a in row["assister_hero_ids"]]
    assist_str = f" (assists: {', '.join(assisters)})" if assisters else ""
    print(f"[{time}] {attacker} killed {victim}{assist_str}")
```

## Net worth over time

Extract per-player net worth at regular intervals.

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("match.dem")
heroes = hero_names()

pt = demo.player_ticks

# Sample every 60 seconds (tick_rate * 60)
interval = demo.tick_rate * 60
sampled = pt.filter(pl.col("tick") % interval == 0)

# Pivot to wide format: one column per hero
nw = (
    sampled
    .select("tick", "hero_id", "gold_net_worth")
    .with_columns(
        pl.col("hero_id").replace_strict(heroes, default="Unknown").alias("hero")
    )
    .pivot(on="hero", index="tick", values="gold_net_worth")
    .sort("tick")
)
print(nw)
```

## Damage breakdown

Summarize total damage dealt by each hero, split by attacker class.

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("match.dem")
heroes = hero_names()

damage = demo.damage

summary = (
    damage
    .group_by("attacker_hero_id", "attacker_class")
    .agg(pl.col("damage").sum().alias("total_damage"))
    .with_columns(
        pl.col("attacker_hero_id")
        .replace_strict(heroes, default="Unknown")
        .alias("hero")
    )
    .sort("total_damage", descending=True)
)
print(summary)
```

## Item build order

Show each player's item purchase order with timestamps.

```python
import polars as pl
from boon import Demo, hero_names, ability_names

demo = Demo("match.dem")
heroes = hero_names()
items = ability_names()

purchases = (
    demo.item_purchases
    .filter(pl.col("change") == "purchased")
    .sort("tick")
)

for row in purchases.iter_rows(named=True):
    time = demo.tick_to_clock_time(row["tick"])
    hero = heroes.get(row["hero_id"], "Unknown")
    item = items.get(row["ability_id"], "Unknown")
    print(f"[{time}] {hero:<16} bought {item}")
```

## Objective timeline

Track when objectives are destroyed and by which team.

```python
from boon import Demo, team_names

demo = Demo("match.dem")
teams = team_names()

for row in demo.boss_kills.sort("tick").iter_rows(named=True):
    time = demo.tick_to_clock_time(row["tick"])
    team = teams.get(row["objective_team"], "Unknown")
    print(f"[{time}] {team} destroyed {row['entity_class']} (obj {row['objective_id']})")
```

## Heatmap data

Extract player positions for a specific hero, suitable for plotting.

```python
import polars as pl
from boon import Demo

demo = Demo("match.dem")

# Filter to a single hero's alive ticks
hero_id = 13  # Haze
alive = demo.player_ticks.filter(
    (pl.col("hero_id") == hero_id) & (pl.col("is_alive") == True)
)

# x/y coordinates ready for matplotlib, seaborn, etc.
positions = alive.select("x", "y").to_numpy()
print(f"{len(positions)} position samples for hero {hero_id}")
# positions[:, 0] = x, positions[:, 1] = y
```

## Active modifiers (buffs/debuffs)

Track when specific abilities are applied to players.

```python
import polars as pl
from boon import Demo, hero_names, ability_names

demo = Demo("match.dem")
heroes = hero_names()
abilities = ability_names()

# Load the opt-in dataset
demo.load("active_modifiers")
mods = demo.active_modifiers

# Filter to "applied" events and resolve names
applied = (
    mods
    .filter(pl.col("event") == "applied")
    .with_columns([
        pl.col("hero_id").replace_strict(heroes, default="Unknown").alias("hero"),
        pl.col("ability_id").replace_strict(abilities, default="Unknown").alias("ability"),
    ])
)

# Top 10 most frequent abilities
top = (
    applied
    .group_by("hero", "ability")
    .len()
    .sort("len", descending=True)
    .head(10)
)
print(top)
```

## Street brawl scores

Street brawl is a separate game mode with its own round-based scoring system. Boon exposes two street-brawl-specific datasets: `street_brawl_ticks` (per-tick state) and `street_brawl_rounds` (round scoring events). These properties only exist on street brawl demos (`game_mode == 4`) — accessing them on a standard match will raise `NotStreetBrawlError`.

```python
from boon import Demo

demo = Demo("street_brawl_match.dem")

rounds = demo.street_brawl_rounds
for row in rounds.iter_rows(named=True):
    print(
        f"Round {row['round']}: "
        f"Amber {row['amber_score']} - Sapphire {row['sapphire_score']} "
        f"(scored by team {row['scoring_team']})"
    )
```
