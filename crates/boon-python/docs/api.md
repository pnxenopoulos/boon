# 📚 API Reference

## `Demo`

```python
from boon import Demo

demo = Demo("match.dem")
```

A Deadlock demo file. Construction parses the file header, file info, and first tick
to extract metadata.

**Raises:**

- `FileNotFoundError` -- If the file does not exist.
- `InvalidDemoError` -- If the file is not a valid demo.
- `DemoHeaderError` -- If required fields (build number, map name) are missing from the file header.
- `DemoInfoError` -- If required fields (playback ticks, playback time) are missing from the file info.
- `DemoMessageError` -- If the match ID could not be resolved from game entities.

**Parameters:**

- **path** (`str`) -- Path to the `.dem` file.

### Methods

#### `verify()`

```python
demo.verify()  # -> bool
```

Verify that the file is a valid demo file. Returns `True` if valid.

This is already called during construction, so it will always return `True`
for an existing `Demo` instance.

---

#### `available_datasets()`

```python
Demo.available_datasets()  # -> list[str]
```

Static method. Returns the list of dataset names that can be passed to `load()` or accessed as properties.

---

#### `load()`

```python
demo.load("kills", "player_ticks", "world_ticks")
```

Load one or more datasets from the demo file in a single pass. See `available_datasets()` for valid names.

Already-loaded datasets are skipped. Multiple datasets requested together share
a single parse pass over the file for efficiency.

**Parameters:**

- **\*datasets** (`str`) -- One or more dataset names to load.

**Raises:**

- `ValueError` -- If an unknown dataset name is provided.
- `NotStreetBrawlError` -- If a street brawl dataset is requested on a non-street-brawl demo.

---

#### `tick_to_seconds()`

```python
demo.tick_to_seconds(11400)  # -> 190.0
```

Convert a tick number to seconds elapsed, excluding paused time.
Automatically loads `world_ticks` on first call to determine pauses.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `float` -- The elapsed time in seconds, excluding pauses.

---

#### `tick_to_clock_time()`

```python
demo.tick_to_clock_time(11400)  # -> "3:10"
```

Convert a tick number to a clock time string (e.g., `"3:14"` or `"12:34"`),
excluding paused time. Automatically loads `world_ticks` on first call to
determine pauses.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `str` -- A formatted clock time string.

#### `summary()`

```python
summary = demo.summary()
summary.keys()                 # dict_keys(['snapshots', 'last_hits', 'objectives', 'damage'])
summary["snapshots"]           # pl.DataFrame -- one row per (snapshot, player)
summary["last_hits"]           # pl.DataFrame -- hero_id, last_hits
summary["objectives"]          # pl.DataFrame -- post-match objective records
summary["damage"]              # pl.DataFrame -- damage matrix (long form)
```

Parse the post-match summary from the demo's `PostMatchDetails` event. Returns a
dict with four top-level keys:

- **`snapshots`** -- a Polars DataFrame with one row per (snapshot, player).
  Snapshots are taken at intervals through the match (not every minute);
  `snapshot_time_s` marks each one. Columns hold that player's running totals at
  that time: `hero_id`, `kills`, `deaths`, `assists`, `net_worth`, `denies`,
  `level`, `lane`, `creep_kills`, `neutral_kills`, `player_damage`, and the
  per-source gold/orbs breakdown (`player_*`, `lane_creep_*`, `neutral_creep*`,
  `boss_*`, `treasure_*`, `denies_*`, `team_bonus_*`, `breakable_*`,
  `assassinate_*`, `trophy_collector_*`, `cultist_sacrifice_*`, `assists_*`, and
  `unknown_*`).
- **`last_hits`** -- a Polars DataFrame of `hero_id` and `last_hits`: the final
  scoreboard last-hit (souls secured) total. This is only recorded per match,
  not per snapshot, which is why it is a separate frame rather than a snapshot
  column.
- **`objectives`** -- a Polars DataFrame of post-match objective records:
  `team_objective_id`, `team`, `destroyed_time_s`, `first_damage_time_s`,
  `creep_damage`, `player_damage`, `player_spirit_damage`. `destroyed_time_s` and
  `first_damage_time_s` are null when the objective was never destroyed/damaged.
- **`damage`** -- a Polars DataFrame of the damage matrix in long form: one row
  per (`dealer_player_slot`, `target_player_slot`, `source_name`,
  `sample_time_s`). Dealer/target are also resolved to `dealer_hero_id` and
  `target_hero_id` (null for non-player slots like `0`), so the frame joins to
  `snapshots`/`last_hits` on `hero_id`. `damage` is the **per-interval, additive**
  amount for that
  `stat_type` dealt during the interval ending at that sample -- `sum` it for
  totals, `cumsum` over `sample_time_s` for the running total. `stat_type` is a
  string (`damage`, `healing`, `heal_prevented`, `mitigated`, `lethal`,
  `regen`). `is_category` (bool) flags Valve's coarse damage-type buckets
  (`Bullet`/`Ability`/`Melee`/`Misc`/`UnknownAbility`), which duplicate the
  specific-source rows for `damage`; filter to `is_category == False` for the
  complete, non-overlapping per-source breakdown across all stat types.

  ```python
  import polars as pl
  dmg = demo.summary()["damage"]
  # player-vs-player damage matrix (totals) -- the obvious query just works:
  (dmg.filter((pl.col("stat_type") == "damage") & ~pl.col("is_category"))
      .group_by("dealer_player_slot", "target_player_slot")
      .agg(pl.col("damage").sum()))
  ```

**Returns:** `dict` -- The post-match summary (Polars DataFrames keyed by name).

**Raises:** `DemoMessageError` -- If the demo contains no post-match details
(for example, an incomplete recording).

#### `kill_participation()`

```python
demo.kill_participation()                          # whole match
demo.kill_participation(start_tick=0, end_tick=18000)  # windowed
```

Each player's kill participation: `(kills + assists) / team_kills`. A player is
credited on a team kill as either the killer or an assister (never both), so the
value is a fraction in `[0, 1]` — the share of their team's kills they were
involved in. Convenience method that delegates to
[`boon.stats.kill_participation()`](#stats); see that section for details.

Optional `start_tick` / `end_tick` restrict the count to kills within that tick
window (the denominator is the team's kills in the same window).

**Returns:** `polars.DataFrame` — one row per player, sorted by `team_num` then
`hero_id`:

| Column | Type | Description |
|--------|------|-------------|
| `hero_id` | `int` | The player's hero ID |
| `team_num` | `int` | The player's team number |
| `kills` | `int` | Kills credited to the player (in the window) |
| `assists` | `int` | Assists credited to the player (in the window) |
| `team_kills` | `int` | Total kills by the player's team (in the window) |
| `kill_participation` | `float` | `(kills + assists) / team_kills`, or null if the team had zero kills |

#### `time_dead()`

```python
demo.time_dead()
```

Time each player spent dead during regulation. A player is dead on any tick
where they are not alive (`is_alive == False`); only non-paused ticks up to the
game-over event are counted, so the totals align with `regulation_ticks` /
`regulation_seconds`. Convenience method that delegates to
[`boon.stats.time_dead()`](#stats).

**Returns:** `polars.DataFrame` — one row per player, sorted by `team_num` then
`hero_id`:

| Column | Type | Description |
|--------|------|-------------|
| `hero_id` | `int` | The player's hero ID |
| `team_num` | `int` | The player's team number |
| `ticks_dead` | `int` | Non-paused regulation ticks spent dead |
| `seconds_dead` | `float` | `ticks_dead / tick_rate` |
| `pct_regulation_dead` | `float` | `ticks_dead / regulation_ticks` as a percentage in `[0, 100]` |

**Raises:** `ValueError` — If the demo has no game-over event (regulation time,
and therefore this metric, is undefined).

### Metadata Properties

#### `path`

```python
demo.path  # pathlib.Path
```

The path to the demo file.

---

#### `total_ticks`

```python
demo.total_ticks  # int
```

The total number of ticks in the demo.

---

#### `total_seconds`

```python
demo.total_seconds  # float
```

The total duration of the demo in seconds, covering the **entire recording**
(including pre-game and post-match time). For the duration of actual gameplay,
see `regulation_seconds`.

---

#### `total_clock_time`

```python
demo.total_clock_time  # str
```

The total duration of the demo as a formatted string (e.g., `"12:34"`), covering
the **entire recording**. For gameplay duration, see `regulation_clock_time`.

---

#### `build`

```python
demo.build  # int
```

The build number of the game that recorded the demo.

---

#### `map_name`

```python
demo.map_name  # str
```

The name of the map the demo was recorded on.

---

#### `match_id`

```python
demo.match_id  # int
```

The match ID for this demo.

---

#### `game_mode`

```python
demo.game_mode  # int
```

The game mode ID for this demo (use `game_mode_names()` to resolve).

---

#### `tick_rate`

```python
demo.tick_rate  # int
```

The tick rate of the demo (ticks per second).

---

#### `winning_team_num`

```python
demo.winning_team_num  # int | None
```

The team number of the winning team, or `None` if no game-over event was found.
Scans for the `k_EUserMsg_GameOver` event on first access.

---

#### `game_over_tick`

```python
demo.game_over_tick  # int | None
```

The tick when the game ended, or `None` if no game-over event was found.
Scans for the `k_EUserMsg_GameOver` event on first access.

---

#### `regulation_ticks`

```python
demo.regulation_ticks  # int | None
```

The number of active (non-paused) ticks of regulation play, counted from the
start of the recording up to the game-over event. Reflects how much of the game
was actually played, unlike `total_ticks` (the full recording). `None` if no
game-over event was found. Scans for `k_EUserMsg_GameOver` and loads
`world_ticks` on first access.

---

#### `regulation_seconds`

```python
demo.regulation_seconds  # float | None
```

The active gameplay duration in seconds, up to the game-over event. Equal to
`regulation_ticks / tick_rate`. The regulation counterpart to `total_seconds`.
`None` if no game-over event was found.

---

#### `regulation_clock_time`

```python
demo.regulation_clock_time  # str | None
```

The regulation play duration as a formatted string (e.g., `"32:45"`). The
counterpart to `total_clock_time`. `None` if no game-over event was found.

### DataFrame Properties

#### `players`

```python
demo.players  # polars.DataFrame
```

Player information. Computed from the final tick.

| Column | Type | Description |
|--------|------|-------------|
| `player_name` | `str` | The player's display name |
| `steam_id` | `int` | The player's Steam ID |
| `hero_id` | `int` | The player's hero ID (use `hero_names()` to resolve) |
| `team_num` | `int` | Raw team number (use `team_names()` to resolve) |
| `start_lane` | `int` | Original lane color (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the `CMsgLaneColor` proto enum) |

---

#### `player_ticks`

```python
demo.player_ticks  # polars.DataFrame
```

Per-tick, per-player state. Returns one row per player per tick.
Rows where the pawn is not found or `hero_id == 0` are skipped.
Auto-loads on first access if not already loaded via `load()`.

**Pawn fields** (from `CCitadelPlayerPawn`):

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `hero_id` | `int` | Hero ID |
| `x` | `float` | Player X position in world (Hammer) units |
| `y` | `float` | Player Y position in world (Hammer) units |
| `z` | `float` | Player Z position in world (Hammer) units |
| `pitch` | `float` | Camera pitch angle |
| `yaw` | `float` | Camera yaw angle |
| `roll` | `float` | Camera roll angle |
| `in_regen_zone` | `bool` | In a regeneration zone |
| `death_time` | `float` | Time of death |
| `last_spawn_time` | `float` | Time of last spawn |
| `respawn_time` | `float` | Time until respawn |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `lifestate` | `int` | Life state value (use `lifestate_names()` to resolve) |
| `souls` | `int` | Current souls (currency) |
| `spent_souls` | `int` | Total spent souls |
| `in_combat_end_time` | `float` | In-combat timer end |
| `in_combat_last_damage_time` | `float` | In-combat last damage time |
| `in_combat_start_time` | `float` | In-combat timer start |
| `player_damage_dealt_end_time` | `float` | Damage dealt timer end |
| `player_damage_dealt_last_damage_time` | `float` | Damage dealt last damage time |
| `player_damage_dealt_start_time` | `float` | Damage dealt timer start |
| `player_damage_taken_end_time` | `float` | Damage taken timer end |
| `player_damage_taken_last_damage_time` | `float` | Damage taken last damage time |
| `player_damage_taken_start_time` | `float` | Damage taken timer start |
| `time_revealed_by_npc` | `float` | Time revealed on minimap by NPC |
| `build_id` | `int` | Hero build ID |

**Controller fields** (from `CCitadelPlayerController`):

| Column | Type | Description |
|--------|------|-------------|
| `is_alive` | `bool` | Whether the player is alive |
| `has_rebirth` | `bool` | Has rebirth |
| `has_rejuvenator` | `bool` | Has rejuvenator |
| `has_ultimate_trained` | `bool` | Ultimate is trained |
| `health_regen` | `float` | Health regeneration rate |
| `ultimate_cooldown_start` | `float` | Ultimate cooldown start time |
| `ultimate_cooldown_end` | `float` | Ultimate cooldown end time |
| `ap_net_worth` | `int` | Ability power net worth |
| `gold_net_worth` | `int` | Gold net worth |
| `denies` | `int` | Total denies |
| `hero_damage` | `int` | Total hero damage dealt |
| `hero_healing` | `int` | Total hero healing |
| `objective_damage` | `int` | Total objective damage |
| `self_healing` | `int` | Total self healing |
| `kill_streak` | `int` | Current kill streak |
| `last_hits` | `int` | Total last hits |
| `level` | `int` | Player level |
| `kills` | `int` | Total kills |
| `deaths` | `int` | Total deaths |
| `assists` | `int` | Total assists |

---

#### `world_ticks`

```python
demo.world_ticks  # polars.DataFrame
```

World state at every tick. Returns one row per tick.
Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `is_paused` | `bool` | Whether the game is paused |
| `next_midboss` | `float` | Time until next midboss spawn |

---

#### `kills`

```python
demo.kills  # polars.DataFrame
```

Hero kill events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the kill occurred |
| `victim_hero_id` | `int` | The hero ID of the killed player |
| `attacker_hero_id` | `int` | The hero ID of the attacker |
| `assister_hero_ids` | `list[int]` | List of hero IDs of players who assisted |

---

#### `damage`

```python
demo.damage  # polars.DataFrame
```

Damage events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the damage occurred |
| `damage` | `int` | The damage dealt |
| `pre_damage` | `float` | The damage before mitigation |
| `victim_hero_id` | `int` | The hero ID of the victim (0 if not a hero) |
| `attacker_hero_id` | `int` | The hero ID of the attacker (0 if not a hero) |
| `victim_health_new` | `int` | The victim's health after damage |
| `hitgroup_id` | `int` | The hitgroup that was hit (use `hitgroup_names()` to resolve) |
| `crit_damage` | `float` | Critical damage amount |
| `attacker_class` | `int` | The attacker's entity class ID |
| `victim_class` | `int` | The victim's entity class ID |

---

#### `flex_slots`

```python
demo.flex_slots  # polars.DataFrame
```

Flex slot unlock events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the flex slot was unlocked |
| `team_num` | `int` | The team number that unlocked the flex slot |

---

#### `abilities`

```python
demo.abilities  # polars.DataFrame
```

Important ability usage events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the ability was used |
| `hero_id` | `int` | The hero ID of the player |
| `ability` | `str` | The ability name |

---

#### `ability_upgrades`

```python
demo.ability_upgrades  # polars.DataFrame
```

Hero ability point spending events (skill tier upgrades). Emits a row each time a
player upgrades one of their abilities. Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the upgrade occurred |
| `hero_id` | `int` | The hero ID of the player |
| `ability_id` | `int` | The raw MurmurHash2 ability ID (use `ability_names()` to resolve) |
| `tier` | `int` | Upgrade tier (1, 2, or 3) |

---

#### `item_purchases`

```python
demo.item_purchases  # polars.DataFrame
```

Item shop transactions. Includes purchases, upgrades, sells, swaps, and failures.
Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the transaction occurred |
| `hero_id` | `int` | The hero ID of the player |
| `ability_id` | `int` | The raw MurmurHash2 item/ability ID (use `ability_names()` to resolve) |
| `change` | `str` | Transaction type: `"purchased"`, `"upgraded"`, `"sold"`, `"swapped"`, `"failure"` |

---

#### `chat`

```python
demo.chat  # polars.DataFrame
```

In-game chat messages. Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the message was sent |
| `hero_id` | `int` | The hero ID of the sender |
| `text` | `str` | The message text |
| `chat_type` | `str` | `"all"` or `"team"` |

---

#### `objectives`

```python
demo.objectives  # polars.DataFrame
```

Objective health state changes. Tracks walkers, barracks, shrines, patrons, and mid boss. Emits a row when an objective's health or max_health changes.
Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `objective_type` | `str` | `"walker"`, `"barracks"`, `"shrine"`, `"patron"`, or `"mid_boss"` |
| `team_num` | `int` | The team that owns the objective |
| `lane` | `int` | Lane color (1=yellow, 3=green, 4=blue, 6=purple; 0 for patron/shrine/mid_boss) |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `phase` | `int` | Patron phase — resolve with `patron_phase_names()` (0=normal, 1=final, 2=transforming; 0 for non-patron) |
| `x` | `float` | X position in world (Hammer) units |
| `y` | `float` | Y position in world (Hammer) units |
| `z` | `float` | Z position in world (Hammer) units |
| `entity_id` | `int` | Entity index (stable per structure across ticks) |

---

#### `mid_boss`

```python
demo.mid_boss  # polars.DataFrame
```

Mid boss lifecycle events including spawn, kill, and rejuvenator buff tracking.
Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `team_num` | `int` | The team involved |
| `event` | `str` | `"spawned"`, `"killed"`, `"picked_up"`, `"used"`, `"expired"` |

---

#### `troopers`

```python
demo.troopers  # polars.DataFrame
```

Per-tick alive lane trooper state. Tracks `CNPC_Trooper` and `CNPC_TrooperBoss` entities.
Emits a row for every alive trooper at every tick.

**Warning:** This is a large dataset (~5M+ rows). Not loaded by default.
Access this property or call `load("troopers")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `trooper_type` | `str` | `"trooper"` or `"trooper_boss"` |
| `team_num` | `int` | The trooper's team |
| `lane` | `int` | Lane assignment (1, 4, or 6) |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `x` | `float` | X position in world (Hammer) units |
| `y` | `float` | Y position in world (Hammer) units |
| `z` | `float` | Z position in world (Hammer) units |
| `entity_id` | `int` | Entity index (stable per trooper across ticks) |

---

#### `neutrals`

```python
demo.neutrals  # polars.DataFrame
```

Neutral creep state changes. Tracks `CNPC_TrooperNeutral` and `CNPC_TrooperNeutralNodeMover`.
Only emits a row when an alive neutral's state changes (health, position), significantly
reducing data volume compared to per-tick tracking.

Not loaded by default. Access this property or call `load("neutrals")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the state changed |
| `team_num` | `int` | The neutral's team |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `x` | `float` | X position in world (Hammer) units |
| `y` | `float` | Y position in world (Hammer) units |
| `z` | `float` | Z position in world (Hammer) units |
| `entity_id` | `int` | Entity index (stable per neutral across ticks) |

---

#### `stat_modifier_events`

```python
demo.stat_modifier_events  # polars.DataFrame
```

Permanent stat bonus change events from urn and breakable pickups. Emits a row whenever a stat total changes.

Not loaded by default. Access this property or call `load("stat_modifier_events")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the stat changed |
| `hero_id` | `int` | The player's hero ID |
| `stat_type` | `str` | `"health"`, `"spirit_power"`, `"fire_rate"`, `"weapon_damage"`, `"cooldown_reduction"`, or `"ammo"` |
| `amount` | `float` | The increase from this event |

---

#### `active_modifiers`

```python
demo.active_modifiers  # polars.DataFrame
```

Active buff/debuff modifiers on players. Tracks applied and removed events
for each modifier.

Not loaded by default. Access this property or call `load("active_modifiers")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the modifier event occurred |
| `hero_id` | `int` | The affected player's hero ID |
| `event` | `str` | `"applied"` or `"removed"` |
| `modifier_id` | `int` | Raw modifier subclass hash ID (use `modifier_names()` to resolve) |
| `ability_id` | `int` | Raw ability subclass hash ID (use `ability_names()` to resolve) |
| `duration` | `float` | Modifier duration |
| `caster_hero_id` | `int` | Hero ID of the caster |
| `stacks` | `int` | Number of stacks |

---

#### `urn`

```python
demo.urn  # polars.DataFrame
```

Urn lifecycle events. Tracks when the urn is picked up, dropped, or returned
by filtering urn-related modifiers from the `ActiveModifiers` string table.
Also tracks delivery point activation/deactivation via `CCitadelIdolReturnTrigger` entities.

Not loaded by default. Access this property or call `load("urn")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the event occurred |
| `event` | `str` | `"picked_up"`, `"dropped"`, `"returned"`, `"delivery_active"`, or `"delivery_inactive"` |
| `hero_id` | `int` | The hero involved (0 for delivery events) |
| `team_num` | `int` | Team of the delivery point (0 for modifier events) |
| `x` | `float` | Delivery point or pawn X position in world (Hammer) units |
| `y` | `float` | Delivery point or pawn Y position in world (Hammer) units |
| `z` | `float` | Delivery point or pawn Z position in world (Hammer) units |

---

#### `street_brawl_ticks`

```python
demo.street_brawl_ticks  # polars.DataFrame
```

Per-tick street brawl state. Only available for street brawl demos (game_mode=4).
Auto-loads on first access if not already loaded via `load()`.

**Raises:** `NotStreetBrawlError` if the demo is not a street brawl game.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `round` | `int` | Current round number |
| `state` | `int` | Street brawl state enum value |
| `amber_score` | `int` | The Hidden King (old name: Amber Hand) score |
| `sapphire_score` | `int` | The Archmother (old name: Sapphire Flame) score |
| `buy_countdown` | `int` | Last buy phase countdown value |
| `next_state_time` | `float` | Time of next state transition |
| `state_start_time` | `float` | Time the current state started |
| `non_combat_time` | `float` | Total non-combat time elapsed |

---

#### `street_brawl_rounds`

```python
demo.street_brawl_rounds  # polars.DataFrame
```

Street brawl round scoring events. Only available for street brawl demos (game_mode=4).
Auto-loads on first access if not already loaded via `load()`.

**Raises:** `NotStreetBrawlError` if the demo is not a street brawl game.

| Column | Type | Description |
|--------|------|-------------|
| `round` | `int` | Sequential round number (1-indexed) |
| `tick` | `int` | The game tick when the round ended |
| `scoring_team` | `int` | The team that scored |
| `amber_score` | `int` | The Hidden King (old name: Amber Hand) cumulative score |
| `sapphire_score` | `int` | The Archmother (old name: Sapphire Flame) cumulative score |

## Name Lookup Functions

Module-level functions for resolving IDs to human-readable names. These do not
require a parsed demo.

### `hero_names()`

```python
from boon import hero_names

hero_names()  # -> dict[int, str]
```

Return a mapping of hero ID to hero name.

**Returns:** `dict[int, str]` -- Hero ID to hero name mapping (e.g., `{1: "Infernus", 2: "Seven", ...}`).

---

### `team_names()`

```python
from boon import team_names

team_names()  # -> dict[int, str]
```

Return a mapping of team number to team name.

**Returns:** `dict[int, str]` -- `{1: "Spectator", 2: "Hidden King", 3: "Archmother"}`.

---

### `ability_names()`

```python
from boon import ability_names

ability_names()  # -> dict[int, str]
```

Return a mapping of MurmurHash2 ability ID to ability name.

**Returns:** `dict[int, str]` -- Ability hash to name mapping.

---

### `game_mode_names()`

```python
from boon import game_mode_names

game_mode_names()  # -> dict[int, str]
```

Return a mapping of game mode ID to game mode name.

**Returns:** `dict[int, str]` -- Game mode ID to name mapping (e.g., `{1: "6v6", 4: "street_brawl"}`).

---

### `modifier_names()`

```python
from boon import modifier_names

modifier_names()  # -> dict[int, str]
```

Return a mapping of MurmurHash2 modifier ID to modifier name.

**Returns:** `dict[int, str]` -- Modifier hash to name mapping.

---

### `patron_phase_names()`

```python
from boon import patron_phase_names

patron_phase_names()  # -> dict[int, str]
```

Return a mapping of patron phase ID to phase name. Phases are the values of
`CNPC_Boss_Tier3.m_ePhase`: `0=normal` (shielded), `1=final` (killable),
`2=transforming` (vulnerable). Non-patron objectives report `0` by default.

**Returns:** `dict[int, str]` -- Patron phase ID to name mapping (e.g., `{0: "normal", 1: "final", 2: "transforming"}`).

---

### `hitgroup_names()`

```python
from boon import hitgroup_names

hitgroup_names()  # -> dict[int, str]
```

Return a mapping of hit group ID to hit group name, for resolving the
`hitgroup_id` column on the `damage` frame. Values are Source 2's `HitGroup_t`
enum: `0=generic`, `1=head`, `2=chest`, `3=stomach`, the limbs (`4=left_arm`,
`5=right_arm`, `6=left_leg`, `7=right_leg`), `8=neck`, `10=gear`, `11=special`,
the tier-2 / drone boss weakpoints (`12`–`18`), `19=head_no_resist`, and
`-1=invalid`. The `HITGROUP_COUNT` sentinel is omitted.

**Returns:** `dict[int, str]` -- Hit group ID to name mapping.

---

### `lifestate_names()`

```python
from boon import lifestate_names

lifestate_names()  # -> dict[int, str]
```

Return a mapping of life state ID to life state name, for resolving the
`lifestate` column on `player_ticks`. Values are Source 2's `LifeState_t` enum.

**Returns:** `dict[int, str]` -- Life state ID to name mapping (`{0: "alive", 1: "dying", 2: "dead", 3: "respawnable", 4: "respawning"}`).

---

(stats)=
## Stats (`boon.stats`)

An analysis layer of derived metrics computed from parsed demo data. Each
function takes a [`Demo`](#demo) and returns a Polars DataFrame, keyed on
`hero_id` so results join cleanly to the parser's other frames (`players`,
`kills`, `player_ticks`, the `summary()` outputs, ...). Every metric is also
surfaced as a convenience method on `Demo` (e.g. `demo.kill_participation()`
delegates to `boon.stats.kill_participation(demo)` — same computation).

### `kill_participation()`

```python
from boon import stats

stats.kill_participation(demo)                              # whole match
stats.kill_participation(demo, start_tick=0, end_tick=18000)  # windowed
demo.kill_participation()                                   # equivalent method form
```

Each player's `(kills + assists) / team_kills`. A player is credited on a team
kill as either the killer or an assister (never both on the same kill), so the
value is a fraction in `[0, 1]` — the share of their team's kills they were
involved in. Pass `start_tick` / `end_tick` to count only kills within that tick
window (the denominator is the team's kills in the same window).

**Returns:** `polars.DataFrame` with columns `hero_id`, `team_num`, `kills`,
`assists`, `team_kills`, `kill_participation` (see the
[`Demo.kill_participation()`](#kill-participation) table), one row per player,
sorted by `team_num` then `hero_id`.

### `time_dead()`

```python
from boon import stats

stats.time_dead(demo)   # equivalently: demo.time_dead()
```

Time each player spent dead during regulation. A player is dead on any tick
where `is_alive == False`; only non-paused ticks up to the game-over event are
counted, so the totals align with `demo.regulation_ticks` /
`demo.regulation_seconds`.

**Returns:** `polars.DataFrame` with columns `hero_id`, `team_num`,
`ticks_dead`, `seconds_dead`, `pct_regulation_dead` (see the
[`Demo.time_dead()`](#time-dead) table), one row per player, sorted by
`team_num` then `hero_id`.

**Raises:** `ValueError` — if the demo has no game-over event.

---

(exceptions)=
## Exceptions

### `InvalidDemoError`

```python
from boon import InvalidDemoError
```

Raised when a demo file is invalid or cannot be parsed (bad magic bytes, corrupted data).

---

### `DemoHeaderError`

```python
from boon import DemoHeaderError
```

Raised when required fields are missing from the demo file header (build number, map name).

---

### `DemoInfoError`

```python
from boon import DemoInfoError
```

Raised when required fields are missing from the demo file info (playback ticks, playback time).

---

### `DemoMessageError`

```python
from boon import DemoMessageError
```

Raised when required data could not be resolved from demo messages (e.g., match ID from `CCitadelGameRulesProxy`).

---

### `NotStreetBrawlError`

```python
from boon import NotStreetBrawlError
```

Raised when accessing street brawl datasets (`street_brawl_ticks`, `street_brawl_rounds`) on a demo that is not a street brawl game (game_mode != 4).
