# 📖 Examples

These examples show common Deadlock replay analysis tasks.

## Resolving IDs to names

Boon DataFrames use integer IDs for heroes, teams, abilities, and modifiers.
The IDs keep data compact and make operations fast. A change to a display
name does not change its ID. Changes to internal names can change hashed IDs.

Use the mapping functions to resolve IDs. `ability_names()` and `modifier_names()`
return internal names. `ability_display_names()` returns localized ability labels.
ID lookups return `dict[int, str]`. `ability_display_names()` returns
`dict[str, str]`, keyed by internal ability name:

```python
from boon import hero_names, team_names, ability_names, modifier_names, game_mode_names

hero_names()       # {1: "Infernus", 2: "Seven", 3: "Vindicta", ...}
team_names()       # {1: "Spectator", 2: "Hidden King", 3: "Archmother"}
ability_names().get(2521299219)   # "ability_golden_idol"
modifier_names().get(3388847715)  # "modifier_citadel_idol_return"
game_mode_names()                # {1: "6v6", 4: "street_brawl"}
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

## Stats, states, and ammo

These examples use `108575009.dem` at demo tick `187554`. Replace the filename,
tick, and client version for another replay. List versions with `boon versions`.
Install the matching version with `boon get VERSION`. A query downloads a missing
version automatically; it does not choose a version from the demo header.

Run this setup, then the example you need:

```python
import polars as pl
from boon import AbilityStat, Demo, HeroStat, StatMode

version = "6712"
tick = 187554
demo = Demo("108575009.dem", preload=False)
roster = demo.players.select("steam_id", "player_name", "team_num")
steam_id = roster["steam_id"][0]
```

### All hero stats for every player

```python
report = demo.calculate_hero_stats(
    ticks=tick,                  # A list, such as [187554, 187600], also works.
    data_version=version,
    stats=list(HeroStat),
    mode=StatMode.CURRENT,
    explain=True,
    strict=False,
)
values = report.values.join(roster, on="steam_id", how="left", validate="m:1")
print(values.sort("tick", "player_name", "stat"))
print(values.filter(pl.col("status") != "calculated"))
print(report.contributions)
print(report.metadata)
```

Omit `steam_ids` to query all players. Add `steam_ids=[steam_id]` to select one
account. `heroes=[hero_id]` selects the hero at each tick; an account can change
heroes. `list(HeroStat)` includes each stat once, despite the `AMMO` alias.

Use enum members or matching strings, such as
`stats=["clip_size", "fire_rate", "bullet_resist"]`. See the
[complete hero stat table](hero-stats.md) for all names and units.
`fire_rate` and `weapon_damage` return bonus percentages. `clip_size` returns
finite capacity. `sprint_speed` is the additional sprint component; add it to
`move_speed` for nominal full sprint speed.

`strict=False` preserves unresolved rows with null values. A partial value is a
known subtotal, not a verified final value. Always retain `status` and
`diagnostic` when you export a report. Do not sum contribution rows: they can
include intermediate inputs, such as spirit power.

### Compare baseline and current values

```python
query = dict(
    ticks=tick,
    data_version=version,
    steam_ids=[steam_id],
    stats=[HeroStat.CLIP_SIZE, HeroStat.FIRE_RATE],
    strict=False,
)
current = demo.calculate_hero_stats(**query, mode="current").values
baseline = demo.calculate_hero_stats(**query, mode="baseline").values
keys = ["tick", "steam_id", "stat"]
comparison = current.join(
    baseline.select(
        *keys,
        pl.col("value").alias("baseline_value"),
        pl.col("status").alias("baseline_status"),
        pl.col("diagnostic").alias("baseline_diagnostic"),
    ),
    on=keys,
    how="left",
    validate="1:1",
)
print(comparison)
```

The [mode table](hero-stats.md#select-baseline-or-current-effects) lists both
strings and enum members. Both modes use the selected tick. Each calculation
applies its equation to the selected inputs. Current movement values still omit
crouching, sprint acceleration, and bullet-hit slows.

### Join recorded states to stat rows

```python
states = demo.player_states(ticks=tick, data_version=version)
with_states = values.filter(pl.col("steam_id").is_not_null()).join(
    states.filter(pl.col("steam_id").is_not_null()).select(
        "tick", "steam_id", "states", "unknown_states"
    ),
    on=["tick", "steam_id"],
    how="left",
    validate="m:1",
)
print(with_states.select("player_name", "stat", "value", "states"))
print(states.filter(pl.col("states").list.contains("IN_COMBAT")))
```

Run the hero-stat example first to create `values`. Use `steam_id` for roster
joins, and `tick` plus `steam_id` for sampled results. Keep null IDs separate;
never match them as one account. Use each row's `hero_id` for its hero at that tick.
The roster's hero ID describes the final hero.

State names come from the catalog, not a fixed enum. See
[available state names](player-states.md#available-state-names) to list them.
An empty known-state list does not prove that no states are set: inspect
`unknown_states`. A null mask means unavailable data.

### Remaining rounds and unlimited ammo

```python
ammo = demo.snapshots(ticks=tick, data_version=version)
print(ammo.select(
    "tick", "steam_id", "hero_id", "ammo_fraction", "ammo", "max_ammo",
    "unlimited_ammo", "ammo_status", "ammo_diagnostic",
))
```

Without `data_version`, snapshots include the recorded `ammo_fraction` but do
not calculate rounds or capacity. With it, `ammo` is `ammo_fraction * max_ammo`,
rounded to the nearest integer; halves round up. `max_ammo` stays finite when
`unlimited_ammo` is true. Missing or duplicate Steam IDs leave calculated ammo
fields null. Partial capacity also makes the round count partial.

### Ability bonuses and imbues

```python
imbues = demo.imbues(ticks=tick, steam_ids=[steam_id], data_version=version)
print(imbues.bindings)  # Source item and selected ability.
print(imbues.effects)   # Individual effects and targeting filters.

ability_report = demo.calculate_ability_stats(
    ticks=tick,
    data_version=version,
    steam_ids=[steam_id],
    stats=list(AbilityStat),
    mode="current",
    include_items=True,
    explain=True,
    strict=False,
)
print(ability_report.values)
print(ability_report.contributions.select(
    "ability_id", "stat", "value", "source", "scope", "state", "included",
    "diagnostic",
))

if not imbues.bindings.is_empty():
    selected_id = imbues.bindings["ability_id"][0]
    selected = demo.calculate_ability_stats(
        ticks=tick,
        data_version=version,
        steam_ids=[steam_id],
        abilities=[selected_id],
        stats=["cooldown_reduction", "duration_bonus", "range_bonus"],
        strict=False,
    )
    print(selected.values)
```

The default query selects signature abilities. `include_items=True` also selects
owned items. `abilities` accepts owned catalog IDs, not names or signature-slot
numbers. Read recorded IDs from `imbues.bindings` or `demo.ability_ticks`.
See the [ability stat table](ability-stats.md#percentage-rules) for all strings,
enum members, and rules.

Ability results are percentages, not final seconds, metres, or running timers.
`item_cooldown_reduction` is separate from hero `cooldown_reduction`.
`not_applicable` means that a rule does not apply, such as cooldown scaling on a
disabled item. Imbue effects are individual catalog values; they are not combined
bonuses. `included=False` contribution rows explain skipped effects.

### Select a ruleset

```python
from boon import rulesets

result = demo.calculate_hero_stats(
    ticks=tick,
    data_version=version,
    stats=["clip_size", "fire_rate"],
    rulesets={
        "clip_size": rulesets.clip_size.v1,
        "fire_rate": rulesets.fire_rate.v1,
    },
    strict=False,
)
print(result.metadata)
```

Omit `rulesets` to use the supported V1 rules. A mapping must contain one rule
for each requested stat. Rule objects have `stat`, `id`, `version`, and
`documented_on`. Rule versions identify equations; `data_version` selects game
data. Custom equations and other rule versions are not accepted.

## Healing and barrier absorption in the summary

```python
summary = demo.summary()
final = (
    summary["snapshots"]
    .filter(pl.col("steam_id").is_not_null())
    .sort("snapshot_time_s")
    .group_by("steam_id")
    .last()
    .join(roster, on="steam_id", how="left", validate="1:1")
)
print(final.select(
    "player_name", "hero_id", "snapshot_time_s", "player_healing",
    "barrier_absorption", "damage_absorbed",
))
```

Run the setup above first. `player_healing` is recorded healing.
`barrier_absorption` is damage stopped by barriers that the player provided.
`damage_absorbed` is damage stopped by barriers on that player. Some viewer
screens combine healing and provided barrier absorption; these columns keep
them separate. Do not add cumulative counters from different reporting periods.

Use `summary["healing"]` for healing and regeneration by source and period,
`summary["gold_sources"]` for soul sources, and `summary["damage"]` for the
recorded matrix. See {ref}`summary <summary>` for fields and aggregation rules.
`demo.healing` and `demo.barriers()` are not available.

## Match summary

Print the match duration, winner, and player roster.

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

The `damage` dataset includes all damage events in the game — hero vs hero, hero vs objectives, troopers, neutrals, and more. Filtering by `attacker_hero_id` or `victim_hero_id` is usually a good idea to focus on what you care about.

This example summarizes total damage dealt by each hero, split by attacker class.

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

Track when objectives are destroyed by filtering for health reaching zero.

```python
import polars as pl
from boon import Demo, team_names

demo = Demo("match.dem")
teams = team_names()

destroyed = demo.objectives.filter(pl.col("health") == 0).sort("tick")
for row in destroyed.iter_rows(named=True):
    time = demo.tick_to_clock_time(row["tick"])
    team = teams.get(row["team_num"], "Unknown")
    print(f"[{time}] {team} lost {row['objective_type']}")
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
positions = alive.select("x", "y")
print(f"{positions.height} position samples for hero {hero_id}")
# positions["x"], positions["y"] pass straight to plt.scatter(...)
```

## Active modifiers (buffs/debuffs)

Track the effective lifetime of abilities on players. An `applied` event starts
an effective lifetime. A `removed` event ends it because of an explicit state
change, an aura exit, slot reuse, or a finite-duration deadline. Boon does not
apply one death rule to all modifiers because some modifiers persist after
death.

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
