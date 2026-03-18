# API Reference

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

#### `load()`

```python
demo.load("kills", "player_ticks", "world_ticks")
```

Load one or more datasets from the demo file in a single pass.

Valid dataset names: `"player_ticks"`, `"world_ticks"`, `"kills"`, `"damage"`,
`"flex_slots"`, `"respawns"`, `"purchases"`, `"abilities"`, `"ability_upgrades"`,
`"shop_events"`, `"chat"`, `"objectives"`, `"boss_kills"`, `"mid_boss"`,
`"troopers"`, `"neutrals"`.

Already-loaded datasets are skipped. Multiple datasets requested together share
a single parse pass over the file for efficiency.

**Parameters:**

- **\*datasets** (`str`) -- One or more dataset names to load.

**Raises:**

- `ValueError` -- If an unknown dataset name is provided.

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

The total duration of the demo in seconds.

---

#### `total_clock_time`

```python
demo.total_clock_time  # str
```

The total duration of the demo as a formatted string (e.g., `"12:34"`).

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

#### `winning_team`

```python
demo.winning_team  # str | None
```

The name of the winning team (e.g., `"Archmother"`), or `None` if no game-over
event was found.

---

#### `banned_hero_ids`

```python
demo.banned_hero_ids  # list[int]
```

List of banned hero IDs. Returns an empty list if no banned heroes event was found.
Scans for the `k_EUserMsg_BannedHeroes` event on first access.

---

#### `banned_heroes`

```python
demo.banned_heroes  # list[str]
```

List of banned hero names. Returns an empty list if no banned heroes event was found.

### DataFrame Properties

#### `teams`

```python
demo.teams  # polars.DataFrame
```

Team number to team name mapping.

| Column | Type | Description |
|--------|------|-------------|
| `team_num` | `int` | Raw team number (1=Spectator, 2=Hidden King, 3=Archmother) |
| `team_name` | `str` | The team name |

---

#### `players`

```python
demo.players  # polars.DataFrame
```

Player information. Computed from the final tick.

| Column | Type | Description |
|--------|------|-------------|
| `player_name` | `str` | The player's display name |
| `steam_id` | `int` | The player's Steam ID |
| `hero` | `str` | The player's hero name |
| `hero_id` | `int` | The player's hero ID |
| `team` | `str` | `"Archmother"`, `"Hidden King"`, or `"Spectator"` |
| `team_num` | `int` | Raw team number |
| `start_lane` | `int` | Original lane (1=left, 4=center, 6=right) |

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
| `x` | `float` | Player X position |
| `y` | `float` | Player Y position |
| `z` | `float` | Player Z position |
| `pitch` | `float` | Camera pitch angle |
| `yaw` | `float` | Camera yaw angle |
| `roll` | `float` | Camera roll angle |
| `in_regen_zone` | `bool` | In a regeneration zone |
| `death_time` | `float` | Time of death |
| `last_spawn_time` | `float` | Time of last spawn |
| `respawn_time` | `float` | Time until respawn |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `lifestate` | `int` | Life state value |
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
| `hitgroup_id` | `int` | The hitgroup that was hit |
| `crit_damage` | `float` | Critical damage amount |
| `attacker_class` | `int` | The attacker's entity class ID |
| `victim_class` | `int` | The victim's entity class ID |

---

#### `purchases`

```python
demo.purchases  # polars.DataFrame
```

Item purchase events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the purchase occurred |
| `hero_id` | `int` | The hero ID of the purchasing player |
| `ability_id` | `int` | The raw ability/item hash ID |
| `ability` | `str` | The ability/item name purchased |
| `sell` | `bool` | Whether this was a sell event |
| `quickbuy` | `bool` | Whether this was a quickbuy purchase |

---

#### `respawns`

```python
demo.respawns  # polars.DataFrame
```

Player respawn events. Auto-loads on first access if not already loaded via `load()`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the player respawned |
| `hero_id` | `int` | The hero ID of the respawned player |

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
| `ability_id` | `int` | The raw MurmurHash2 ability ID |
| `ability` | `str` | The ability name |
| `upgrade_bits` | `int` | Cumulative upgrade bitmask (1=T1, 3=T1+T2, 7=T1+T2+T3, 15=T1+T2+T3+T4) |

---

#### `shop_events`

```python
demo.shop_events  # polars.DataFrame
```

Item shop transactions. Includes purchases, upgrades, sells, swaps, and failures.
Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the transaction occurred |
| `hero_id` | `int` | The hero ID of the player |
| `ability_id` | `int` | The raw MurmurHash2 item/ability ID |
| `ability` | `str` | The item name |
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

Per-tick objective entity health. Tracks walkers, titans, barracks, and mid boss.
Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `objective_type` | `str` | `"walker"`, `"titan"`, `"barracks"`, or `"mid_boss"` |
| `team_num` | `int` | The team that owns the objective |
| `lane` | `int` | Lane assignment (1, 4, or 6) |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |

---

#### `boss_kills`

```python
demo.boss_kills  # polars.DataFrame
```

Objective destruction events. Auto-loads on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the objective was destroyed |
| `objective_team` | `int` | The team that owned the destroyed objective |
| `objective_id` | `int` | Objective mask change ID |
| `entity_class` | `str` | `"walker"`, `"mid_boss"`, `"titan_shield_generator"`, `"barracks"`, `"titan"`, `"core"` |
| `gametime` | `float` | The game time when the objective was destroyed |
| `bosses_remaining` | `int` | Number of bosses remaining for the team |

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
| `hero_id` | `int` | The hero involved (0 for spawn/kill events) |
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
| `x` | `float` | X position |
| `y` | `float` | Y position |
| `z` | `float` | Z position |

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
| `neutral_type` | `str` | `"neutral"` or `"neutral_node_mover"` |
| `team_num` | `int` | The neutral's team |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `x` | `float` | X position |
| `y` | `float` | Y position |
| `z` | `float` | Z position |

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
