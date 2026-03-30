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
| `lane` | `int` | Lane assignment (1, 4, or 6; 0 for patron/shrine/mid_boss) |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `phase` | `int` | Patron phase (0=normal, 2=shields down, 1=final phase; 0 for non-patron) |
| `x` | `float` | X position |
| `y` | `float` | Y position |
| `z` | `float` | Z position |
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
| `x` | `float` | X position |
| `y` | `float` | Y position |
| `z` | `float` | Z position |
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
| `x` | `float` | X position |
| `y` | `float` | Y position |
| `z` | `float` | Z position |
| `entity_id` | `int` | Entity index (stable per neutral across ticks) |

---

#### `stat_modifier_events`

```python
demo.stat_modifier_events  # polars.DataFrame
```

Permanent stat bonus change events from idol and breakable pickups. Emits a row whenever a stat total changes.

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

Urn (idol) lifecycle events. Tracks when the urn is picked up, dropped, or returned
by filtering idol-related modifiers from the `ActiveModifiers` string table.
Also tracks delivery point activation/deactivation via `CCitadelIdolReturnTrigger` entities.

Not loaded by default. Access this property or call `load("urn")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the event occurred |
| `event` | `str` | `"picked_up"`, `"dropped"`, `"returned"`, `"delivery_active"`, or `"delivery_inactive"` |
| `hero_id` | `int` | The hero involved (0 for delivery events) |
| `team_num` | `int` | Team of the delivery point (0 for modifier events) |
| `x` | `float` | Delivery point X position (0.0 for modifier events) |
| `y` | `float` | Delivery point Y position (0.0 for modifier events) |
| `z` | `float` | Delivery point Z position (0.0 for modifier events) |

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
