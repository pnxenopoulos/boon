# 📚 API Reference

(demo)=
## `Demo`

```python
from boon import Demo

demo = Demo("match.dem")
```

A Deadlock demo file. The constructor reads the file header, file information,
and first tick for match metadata, then preloads kills, damage, and abilities in
one shared pass. Other datasets load on first access. Combat parsing errors are
reported during construction when preloading is enabled.

Use `Demo("match.dem", preload=False)` for lightweight construction. This
keyword-only option disables combat preloading; datasets remain available on
first access or through `demo.load(...)`. CLI commands use this mode to load
only the datasets they request.

**Raises:**

- `FileNotFoundError` -- If the file does not exist.
- `InvalidDemoError` -- If the file is not a valid demo.
- `DemoHeaderError` -- If required fields (build number, map name) are missing from the file header.
- `DemoInfoError` -- If required fields (playback ticks, playback time) are missing from the file info.

**Parameters:**

- **path** (`str`) -- Path to the `.dem` file.

### Methods

#### `verify()`

```python
demo.verify()  # -> bool
```

Verify that the file is a valid demo. The method returns `True` for a valid file.

The constructor runs this check. An existing `Demo` instance always returns `True`.

---

#### `available_datasets()`

```python
Demo.available_datasets()  # -> list[str]
```

Return all dataset names. You can pass these names to `load()` or access them as properties.

---

#### `load()`

```python
demo.load("kills", "player_ticks", "world_ticks")
```

Load one or more datasets from the demo. Use `available_datasets()` to get valid names.

Boon skips datasets that are already loaded. Event and entity datasets share
one filtered pass. Player, world, and trooper snapshots share a parallel pass.
A request that includes both groups uses both passes.

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

Convert a tick number to elapsed seconds. The result excludes paused time.
The method loads `world_ticks` on the first call to find pauses.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `float` -- The elapsed time in seconds, excluding pauses.

---

#### `tick_to_clock_time()`

```python
demo.tick_to_clock_time(11400)  # -> "3:10"
```

Convert a tick number to a clock time string, such as `"3:14"` or `"12:34"`.
The result excludes paused time. The method loads `world_ticks` on the first
call to find pauses.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `str` -- A formatted clock time string.

---

#### `tick_to_match_seconds()`

```python
demo.tick_to_match_seconds(11400)  # -> 160.0  (on-screen match clock, not elapsed)
```

Convert a tick to on-screen match-clock seconds. `tick_to_seconds` counts from the
demo's tick 0, which is the pre-game lobby, so it leads the on-screen match clock by
`pregame_seconds`; this subtracts that offset. The result is `0.0` at `game_start_tick`
and negative during the pre-game (matching the spectator clock's countdown), and it
excludes paused time.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `float | None` -- Match-clock seconds, or `None` when `pregame_seconds`
is unavailable.

---

#### `tick_to_match_clock()`

```python
demo.tick_to_match_clock(11400)  # -> "2:40"   (on-screen clock)
demo.tick_to_match_clock(500)    # -> "-0:22"  (pre-game countdown)
```

The formatted counterpart to `tick_to_match_seconds`. Pre-game ticks read as a
negative clock.

**Parameters:**

- **tick** (`int`) -- The game tick to convert.

**Returns:** `str | None` -- A formatted match-clock string, or `None` when
`pregame_seconds` is unavailable.

#### `snapshots()`

```python
demo.snapshots(every=64)                          # ~1 row/sec of ticks
demo.snapshots(ticks=[29000, 30000])              # specific ticks
demo.snapshots(start_tick=29000, end_tick=30000)  # a contiguous window
demo.snapshots("troopers", events="kills")        # troopers at kill ticks
demo.snapshots(["player_ticks", "world_ticks"], seconds=1.0)  # -> dict
```

Sample per-tick state at *selected* ticks in one parallel pass. Boon decodes
the demo once. It processes full-packet keyframe segments in parallel. Boon
creates rows only for the selected ticks. Therefore, `snapshots(every=64)`
uses less memory and time than a full `player_ticks` frame that you filter
in Python.

- **`datasets`** -- which snapshot dataset(s): `"player_ticks"` (default),
  `"world_ticks"`, `"troopers"`, or a list.
- **`ticks`** -- a specific tick or list of ticks.
- **`every`** / **`seconds`** -- a periodic stride (mutually exclusive).
- **`events`** -- sample at the ticks of event datasets, such as `"kills"`.
- **`start_tick`** / **`end_tick`** -- restrict to a contiguous window.

Return one DataFrame for one dataset. Return a dictionary for multiple datasets.
A window without another selector returns each tick in the window. A request
without a selector raises `ValueError`.


#### `summary()`

```python
summary = demo.summary()
summary["snapshots"]     # Player totals and state at each recorded snapshot
summary["gold_sources"]  # Souls, kills, and damage by source at each snapshot
summary["last_hits"]     # Final last-hit totals
summary["objectives"]    # Recorded objective results
summary["damage"]        # Source-to-target matrix: interval amounts and totals
summary["healing"]       # Healing and regeneration from that matrix
```

Boon decodes the demo's `PostMatchDetails` message. These tables contain recorded
post-match statistics. They do not estimate healing from changes in health.
The first call builds and caches all six tables.

- **`snapshots`** has one row per player and `snapshot_time_s`. It includes
  `player_slot`, `hero_id`, `kills`, `deaths`, `assists`, `net_worth`, `denies`,
  `level`, `lane`, `creep_kills`, and `neutral_kills`.
  Recorded damage totals are `player_damage`, `creep_damage`, `neutral_damage`,
  `boss_damage`, `self_damage`, and `player_damage_taken`.
  Healing totals are `player_healing`, `teammate_healing`, and `self_healing`.
  Other recorded counters are `damage_mitigated`, `damage_absorbed`,
  `absorption_provided`, `heal_prevented`, and `heal_lost`.
  The added counters are null when the message omits them.
  Existing gold and orb columns remain available. Their prefixes are `player_*`,
  `lane_creep_*`, `neutral_creep*`, `boss_*`, `treasure_*`, `denies_*`,
  `team_bonus_*`, `breakable_*`, `assassinate_*`, `trophy_collector_*`,
  `cultist_sacrifice_*`, `assists_*`, and `unknown_*`.
  The `unknown_*` columns refer to the Goose Egg source.
- **`gold_sources`** has one row per recorded player, snapshot, and soul source.
  Columns are `snapshot_time_s`, `player_slot`, `hero_id`, `source_id`,
  `source_name`, `gold`, `gold_orbs`, `kills`, and `damage`.
  The counters are cumulative at that snapshot. `gold` and `gold_orbs` preserve
  the separate counters from the message. Use `snapshots.net_worth` for net worth.
  Source names are protobuf names, such as `k_ePlayers`, `k_eLaneCreeps`, and
  `k_eAssists`. Unknown IDs remain available as `unknown_<id>`.
  Absent source IDs, names, or counters are null. This table preserves sources
  that have no column in `snapshots`.
- **`last_hits`** contains `hero_id` and the final scoreboard `last_hits` total.
- **`objectives`** contains `team_objective_id`, `team`, `destroyed_time_s`,
  `first_damage_time_s`, `creep_damage`, `player_damage`, and
  `player_spirit_damage`. Absent times are null.
- **`damage`** contains the full recorded matrix. Each row identifies a
  `dealer_player_slot`, `target_player_slot`, `source_name`, `stat_type`, and
  `sample_time_s`. `dealer_hero_id` and `target_hero_id` identify roster heroes;
  non-player slots have null hero IDs.
  `damage` is the amount for the interval from `interval_start_s` to
  `sample_time_s`. **`total` is the recorded cumulative amount at `sample_time_s`.**
  `stat_type` is `damage`, `healing`, `heal_prevented`, `mitigated`, `lethal`, or
  `regen`. Unknown values use `unknown_<id>`.
  `is_category=True` marks broad source categories such as `Bullet`, `Ability`,
  `Melee`, `Misc`, and `UnknownAbility`. These rows duplicate specific sources.
  Select categories or specific sources before aggregation. Do not add them together.
- **`healing`** selects `healing` and `regen` rows from the matrix and excludes
  category duplicates. Columns are `interval_start_s`, `interval_end_s`,
  `healer_player_slot`, `healer_hero_id`, `target_player_slot`, `target_hero_id`,
  `source_name`, `stat_type`, `amount`, and `total`.
  `amount` is the interval amount. **`total` is the recorded cumulative amount
  at `interval_end_s`.** Periods with no increase remain in the table.
  `stat_type` separates healing from regeneration. `source_name` preserves the
  recorded item, ability, modifier, or other source label.

Times use match-clock seconds. Player snapshots and matrix samples can use
**different reporting periods**. Use their recorded times; do not assume a fixed
interval or join them by row number. Sparse matrix histories start at later
samples. Boon does not add rows for unrecorded periods.
Hero IDs come from the match roster. Use player slots to identify players across
hero changes.

To get a total at one reporting period, select that time and sum `total` across
sources. **Do not sum `total` across reporting periods.** To combine periods,
sum the interval column (`damage` or `amount`).

```python
import polars as pl

summary = demo.summary()
healing = summary["healing"]
matrix = summary["damage"]
period = matrix["sample_time_s"].max()  # Select a recorded reporting period

# Healing and regeneration totals by player and type at each period.
healing_totals = healing.group_by(
    "interval_end_s", "healer_player_slot", "stat_type"
).agg(pl.col("total").sum())

# Healing or regeneration by source for one player at one period.
healing_sources = healing.filter(
    (pl.col("interval_end_s") == period) & (pl.col("healer_player_slot") == 2)
).group_by("stat_type", "source_name").agg(pl.col("total").sum())

# Souls by source. This table uses the player snapshot schedule.
souls = summary["gold_sources"].filter(pl.col("player_slot") == 2)

# Damage dealt by type at one period: use only broad categories.
damage_types = matrix.filter(
    (pl.col("sample_time_s") == period)
    & (pl.col("stat_type") == "damage")
    & pl.col("is_category")
).group_by("dealer_player_slot", "source_name").agg(pl.col("total").sum())

# Damage to/from players: use only specific sources to prevent duplicates.
player_damage = matrix.filter(
    (pl.col("sample_time_s") == period)
    & (pl.col("stat_type") == "damage")
    & ~pl.col("is_category")
    & pl.col("dealer_hero_id").is_not_null()
    & pl.col("target_hero_id").is_not_null()
).group_by("dealer_player_slot", "target_player_slot").agg(pl.col("total").sum())

# Optional square matrix: rows deal damage, columns receive damage.
damage_matrix = player_damage.pivot(
    on="target_player_slot", index="dealer_player_slot", values="total"
)
```

**Returns:** `dict[str, polars.DataFrame]` with the six tables above.

**Raises:** `DemoMessageError` if the post-match message is absent or invalid.

(kill-participation)=
#### `kill_participation()`

```python
demo.kill_participation()                          # whole match
demo.kill_participation(start_tick=0, end_tick=18000)  # windowed
```

Each player's kill participation is `(kills + assists) / team_kills`.
A kill credits a player as either the killer or an assister, never both.
The ratio is in `[0, 1]`, or null if the team has no kills.
This method calls [`boon.stats.kill_participation()`](#stats).

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

(time-dead)=
#### `time_dead()`

```python
demo.time_dead()
```

Time each player spent dead during regulation. A player is dead when
`is_alive == False`. The function counts only unpaused ticks up to game over.
The totals use the same time limits as `regulation_ticks` and
`regulation_seconds`. This method calls [`boon.stats.time_dead()`](#stats).

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
and thus this metric, is undefined).

(in-combat)=

#### `in_combat()`

```python
demo.in_combat()
```

Whether each player is in combat, per tick. Each hit updates the pawn's combat
window to `last_damage_time + delay`. The observed delay is approximately
0.5 seconds for trooper or denizen damage and 3 seconds for hero damage.
The function compares game time with `player_ticks.in_combat_end_time`.
This method calls [`boon.stats.in_combat()`](#stats).

**Returns:** `polars.DataFrame` — one row per `(tick, hero_id)`, so it joins
directly onto `player_ticks`, sorted by `tick` then `hero_id`:

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `hero_id` | `int` | The player's hero ID |
| `in_combat` | `bool` | Whether the player is in combat on that tick |

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

The total duration of the demo as a formatted string (for example, `"12:34"`), covering
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
demo.match_id  # int | None
```

The match ID for this demo, or `None` if the demo does not carry one (for example a
partial capture or sandbox / custom content).

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
Uses the cached `k_EUserMsg_GameOver` event, or scans for it if necessary.

---

#### `game_over_tick`

```python
demo.game_over_tick  # int | None
```

The tick when the game ended, or `None` if no game-over event was found.
Uses the cached `k_EUserMsg_GameOver` event, or scans for it if necessary.

---

#### `game_start_tick`

```python
demo.game_start_tick  # int | None
```

The tick at which the on-screen match clock reaches `0:00`, when the barrier drops
and the game begins, ending the pre-game lobby. The counterpart to `game_over_tick`.
`None` if `pregame_seconds` is unavailable.

---

#### `pregame_seconds`

```python
demo.pregame_seconds  # float | None
```

The pregame duration in seconds, usually about 30 seconds.
Boon reads the replicated match clock at game over to calculate the duration.
It does not assume a fixed duration.

`match clock = tick_to_seconds - pregame_seconds`.
The result is `None` if the game-over event or match clock is absent.
It is also `None` if recording starts after pregame.
See `tick_to_match_seconds` and `tick_to_match_clock`.

---

#### `regulation_ticks`

```python
demo.regulation_ticks  # int | None
```

The number of match-clock ticks at the game-over event. Boon uses the
replicated HUD match clock, so this value excludes pregame, pauses, and
post-game time. Old demos that omit the clock or set it to zero use active demo
ticks as a fallback. The fallback excludes pauses and post-game time, but it can
include pregame recording time. Therefore, it might not match the HUD clock
exactly. `None` if no game-over event was found.

---

#### `regulation_seconds`

```python
demo.regulation_seconds  # float | None
```

The replicated HUD match-clock value at the game-over event, in seconds.
`regulation_ticks` is this value converted to ticks. `None` if no game-over
event was found.

---

#### `regulation_clock_time`

```python
demo.regulation_clock_time  # str | None
```

The match-clock value at game over as a formatted string (for example,
`"32:45"`). The format drops fractional seconds. `None` if no game-over event
was found.

### DataFrame Properties

#### `players`

```python
demo.players  # polars.DataFrame
```

Player information. Computed from a snapshot at game over (or the final tick
when no game-over event is available).

| Column | Type | Description |
|--------|------|-------------|
| `player_name` | `str` | The player's display name |
| `steam_id` | `int` | The player's Steam ID |
| `hero_id` | `int` | The player's hero ID (use `hero_names()` to resolve) |
| `team_num` | `int` | Raw team number (use `team_names()` to resolve) |
| `start_lane` | `int` | Original lane color (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the `CMsgLaneColor` proto enum) |
| `rank` | `int` | Packed competitive display rank; `0` means unranked, calibrating, or unavailable |

---

#### `banned_heroes`

```python
demo.banned_heroes  # polars.DataFrame
```

Heroes that the `BannedHeroes` user message identifies. The server can send
this message once before the match starts. GOTV recordings do not always
contain the message.

| Column | Type | Description |
|--------|------|-------------|
| `hero_id` | `int` | The banned hero's ID (joins to `players.hero_id`) |
| `hero_name` | `str` | Resolved hero name, or `"HERO_NOT_FOUND"` for an ID absent from the selected boon-data catalog |

The message contains only hero IDs. It does not contain the team, banning
player, or draft order. Boon can list unavailable heroes, but it cannot build
the draft order.

An empty DataFrame means that the demo contains no ban data. It does not prove
that the match had no bans. The demo cannot distinguish a match without bans
from a server build that did not send the message.

```python
demo.banned_heroes
# shape: (2, 2)
# ┌─────────┬─────────────┐
# │ hero_id ┆ hero_name   │
# │ ---     ┆ ---         │
# │ i64     ┆ str         │
# ╞═════════╪═════════════╡
# │ 69      ┆ The Doorman │
# │ 63      ┆ Mina        │
# └─────────┴─────────────┘
```

---

#### `player_ticks`

```python
demo.player_ticks  # polars.DataFrame
```

Per-tick, per-player state. Returns one row per player per tick.
Rows where the pawn is not found or `hero_id == 0` are skipped.
Boon loads this dataset on first access.

The `stat_modifier_*` columns are signed sums of known entries in the
controller's `m_vecStatViewerModifierValues` vector. They do not include base
hero stats or all temporary effects. Do not use them as final or effective stats.

**Player fields** (from the player pawn and controller):

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
| `in_item_shop` | `bool` | In an item shop zone |
| `death_time` | `float` | Time of death |
| `last_spawn_time` | `float` | Time of last spawn |
| `respawn_time` | `float` | Time until respawn |
| `health` | `int` | Current health |
| `max_health` | `int` | Maximum health |
| `barrier` | `float` | Current barrier remaining; `0.0` when no tracker is present |
| `stat_modifier_health` | `float` | Observed health modifier total |
| `stat_modifier_spirit_power` | `float` | Observed spirit-power modifier total |
| `stat_modifier_fire_rate` | `float` | Observed fire-rate modifier total |
| `stat_modifier_weapon_damage` | `float` | Observed weapon-damage modifier total |
| `stat_modifier_cooldown_reduction` | `float` | Observed cooldown-reduction modifier total |
| `stat_modifier_ammo` | `float` | Observed ammo modifier total |
| `stat_modifier_bullet_resist` | `float` | Observed bullet-resistance modifier total |
| `stat_modifier_spirit_resist` | `float` | Observed spirit-resistance modifier total |
| `stat_modifier_values_available` | `bool` | The demo serializer contains the stat-viewer vector |
| `unknown_stat_modifier_count` | `int` | Vector entries with an unknown nonzero value type |
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
Boon loads this dataset on first access.

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

Hero kill events. Preloaded during construction unless `preload=False`.

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

Damage events. Preloaded during construction unless `preload=False`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The enclosing demo command tick |
| `damage` | `int` | The damage dealt |
| `pre_damage` | `float` | The damage before mitigation |
| `damage_absorbed` | `float` or null | Recorded absorption; legacy integer fallback when the float field is absent |
| `victim_shield_new` | `int` or null | Remaining shield after the hit |
| `victim_shield_max` | `int` or null | Shield capacity |
| `server_tick` | `int` or null | Server tick recorded in the damage message |
| `victim_hero_id` | `int` | The hero ID of the victim (0 if not a hero) |
| `attacker_hero_id` | `int` | The hero ID of the attacker (0 if not a hero) |
| `victim_health_new` | `int` | The victim's health after damage |
| `hitgroup_id` | `int` | The hitgroup that was hit (use `hitgroup_names()` to resolve) |
| `crit_damage` | `float` | Critical damage amount |
| `attacker_class` | `int` | The attacker's entity class ID |
| `victim_class` | `int` | The victim's entity class ID |
| `ability_id` | `int` | Raw ability/weapon ID, or `0` when absent; resolve with `ability_names()` |
| `damage_type` | `int` | Raw Source `type` damage bitfield |
| `citadel_type` | `int` | Deadlock damage category; `3` is melee-typed damage |
| `damage_flags` | `int` | Raw Valve damage flags used for detailed classification |
| `is_melee` | `bool` | Whether this is melee-typed damage (`citadel_type == 3`) |
| `melee_type` | `str` or null | `"light"`, `"heavy"`, or `"other"` for melee-typed damage; null otherwise |

`tick` comes from the demo command that contains the damage message. Use this
value with other Boon datasets and the tick-to-clock methods. `server_tick`
comes from the damage message itself. These counters can differ. In
`106996573.dem`, `server_tick - tick` is either 1,705 or 1,706. Thus, one fixed
offset does not give an exact conversion for all rows. A null `server_tick`
means the message does not contain this field.

The shield fields are copied from the damage message. Observed hit sequences
suggest that `victim_shield_new` is the shield remaining after the hit and
`victim_shield_max` is the capacity of that shield pool. For example, in
`103129247.dem`, five hits reduce the reported shield from 95 to 0 while its
capacity stays at 127. The final hit absorbs about 3.30 damage and deals 11
damage to health. The shield fields are integers; `damage_absorbed` can have
a fractional value. These fields do not give a complete history of shield
gains and expiry. They can be absent even when absorption is positive. Null
means absent, not zero.

`is_melee` contains Valve's melee damage category. The `DFLAG_LIGHT_MELEE`
and `DFLAG_HEAVY_MELEE` bits identify light and heavy hits.
`melee_type="other"` identifies melee abilities, NPC attacks, and unclear
flag combinations. Boon does not use the ability name or damage value to
classify melee damage.

---

#### `flex_slots`

```python
demo.flex_slots  # polars.DataFrame
```

Flex slot unlock events. Boon loads this dataset on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the flex slot was unlocked |
| `team_num` | `int` | The team number that unlocked the flex slot |

---

#### `abilities`

```python
demo.abilities  # polars.DataFrame
```

Important ability usage events. Preloaded during construction unless `preload=False`.

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
player upgrades one of their abilities. Boon loads this dataset on first access.

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
Boon loads this dataset on first access.

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

In-game chat messages. Boon loads this dataset on first access.

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
Boon loads this dataset on first access.

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
Boon loads this dataset on first access.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick |
| `team_num` | `int` | The team involved |
| `event` | `str` | `"spawned"`, `"killed"`, `"picked_up"`, `"used"`, `"expired"` |

---

#### `rift`

```python
demo.rift  # polars.DataFrame
```

Rift lifecycle — one row per Rift. Boon loads this dataset on first access.

The Rift is a periodic king-of-the-hill objective (`Koth` in the game files).
After the announcement, teams can capture it until it expires.
A successful capture grants enhanced troopers to the winning team in the
Rift's lane.

Exactly one of `capture_tick` / `expire_tick` is set per row. Only completed
Rifts appear: one still live when the demo ends is omitted.

| Column | Type | Description |
|--------|------|-------------|
| `rift_num` | `int` | 1-based Rift index in the match. Entity indices are recycled between Rifts, so this is the stable identifier |
| `announce_tick` | `int \| None` | Tick the spawner appeared, ahead of the Rift becoming contestable; `None` if not observed |
| `active_tick` | `int` | Tick the Rift became contestable |
| `capture_tick` | `int \| None` | Tick a team captured it, or `None` if it expired |
| `expire_tick` | `int \| None` | Tick it expired uncaptured, or `None` if it was captured |
| `winning_team` | `int \| None` | Team that captured it; `None` if it expired |
| `lane` | `int` | Lane the Rift spawned in (`1`/`6` observed), or `0` when the location is not a known Rift site |
| `x` | `float` | X position of the cash-in in world (Hammer) units |
| `y` | `float` | Y position of the cash-in in world (Hammer) units |
| `z` | `float` | Z position of the cash-in in world (Hammer) units |

Boon reads the winner from the game rules' scoring team.
The Rift entity's `m_iTeamNum` records the last team to make capture progress.
That team can differ from the winner.

```python
demo.rift.select(["rift_num", "capture_tick", "winning_team", "lane"])
# ┌──────────┬──────────────┬──────────────┬──────┐
# │ rift_num ┆ capture_tick ┆ winning_team ┆ lane │
# ╞══════════╪══════════════╪══════════════╪══════╡
# │ 1        ┆ 48871        ┆ 3            ┆ 1    │
# │ 2        ┆ 79884        ┆ 3            ┆ 6    │
# └──────────┴──────────────┴──────────────┴──────┘
```

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

#### `breakables`

```python
demo.breakables  # polars.DataFrame
```

Breakable map-prop destruction events. Boon tracks
`CCitadel_BreakableProp` entities. A broken prop leaves the PVS without a
health-zero update or a permanent delete. Boon keeps the leave as a candidate
until parsing is complete. It emits the event only when the same
`(entity_id, entity_serial)` identity does not become active again. Boon
ignores full-packet delete and create replacements.

The server does not report health zero or a dead lifestate before the leave.
Therefore, the dataset does not contain these columns. Position and team use
the last values before the final leave.

Boon does not load this dataset by default. Access the property or call `load("breakables")`.
Subclass names come from the newest verified local boon-data `misc.json`,
automatically downloading latest if no installation exists. The recorded
`subclass_id` is preserved even when no catalog entry matches it. To resolve
those IDs against a different version, use `breakable_names(version="6698")`.
Catalog acquisition failures raise `boon.data.DataError` before parsing begins.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | Tick when the prop was broken |
| `event` | `str` | Always `"broken"` |
| `entity_id` | `int` | Entity slot index |
| `entity_serial` | `int` | Serial distinguishing reuse of the slot |
| `subclass_id` | `int` | Raw unsigned `m_nSubclassID` hash |
| `subclass_name` | `str` | Resolved internal subclass name, or `"BREAKABLE_NOT_FOUND"` |
| `team_num` | `int` | Last-known team |
| `x` | `float` | Last-known X position in world (Hammer) units |
| `y` | `float` | Last-known Y position in world (Hammer) units |
| `z` | `float` | Last-known Z position in world (Hammer) units |

---

#### `sinners_sacrifice`

```python
demo.sinners_sacrifice  # polars.DataFrame
```

Sinner's Sacrifice machine lifecycle and hit events. Boon tracks
`CNPC_Neutral_SinnersSacrifice` and
`CNPC_Neutral_SinnersSacrifice_Hideout`. Entity state supplies the identity,
health, and position. Each Damage message supplies the victim, incoming damage,
and attacker hero. Boon keeps a health decrease that has no matching message.
For this event, `attacker_hero_id` is `0`.

`health` is the machine state at the end of the tick. Multiple hits in one
tick can have the same health value. A completed machine stays alive at one
health. An inactive machine can omit health fields. Therefore, the dataset does
not add a false death or lifestate event.

Boon does not load this dataset by default. Access the property or call
`load("sinners_sacrifice")`.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | Tick when the event occurred |
| `event` | `str` | `"spawned"`, `"hit"`, or `"reset"` |
| `entity_id` | `int` | Entity slot index |
| `entity_serial` | `int` | Serial distinguishing reuse of the slot |
| `attacker_hero_id` | `int` | Attacker hero ID, or `0` when unresolved/not applicable |
| `damage` | `int` | Incoming damage, or `0` for spawned/reset events |
| `health` | `int` | Machine health at the end of this tick |
| `max_health` | `int` | Maximum machine health |
| `team_num` | `int` | Machine team |
| `x` | `float` | X position in world (Hammer) units |
| `y` | `float` | Y position in world (Hammer) units |
| `z` | `float` | Z position in world (Hammer) units |

---

#### `stat_modifier_events`

```python
demo.stat_modifier_events  # polars.DataFrame
```

Permanent stat bonus changes from urn and breakable pickups. Boon emits a row when a stat total changes.

Not loaded by default. Access this property or call `load("stat_modifier_events")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the stat changed |
| `hero_id` | `int` | The player's hero ID |
| `stat_type` | `str` | `"health"`, `"spirit_power"`, `"fire_rate"`, `"weapon_damage"`, `"cooldown_reduction"`, `"ammo"`, `"bullet_resist"`, or `"spirit_resist"` |
| `amount` | `float` | The signed change from this event |

---

#### `active_modifiers`

```python
demo.active_modifiers  # polars.DataFrame
```

Raw active buff and debuff modifiers on players. Boon tracks `applied`,
`changed`, and `removed` events for each Source 2 modifier serial. A
`changed` event records a change to stacks, duration, or application time.

One ability can create multiple modifier instances. The number of rows is not
the stack count. Use `stacks`. A `serial` identifies an active entry. The
game can reuse the serial after removal. Use the apply, change, and remove
events to identify each cycle. Do not use the serial as a globally unique
lifecycle ID.

Not loaded by default. Access this property or call `load("active_modifiers")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick when the modifier event occurred |
| `hero_id` | `int` | The affected player's hero ID |
| `event` | `str` | `"applied"`, `"changed"` (live state changed), or `"removed"` |
| `serial` | `int` | Source 2 runtime identifier; unique while live, reusable after removal |
| `modifier_id` | `int` | Raw modifier subclass hash ID (use `modifier_names()` to resolve) |
| `ability_id` | `int` | Raw ability subclass hash ID (use `ability_names()` to resolve) |
| `duration` | `float` | Current modifier duration in seconds (`-1` when indefinite) |
| `caster_hero_id` | `int` | Hero ID of the caster |
| `stacks` | `int` | Number of stacks |

---

#### `ability_ticks`

```python
demo.ability_ticks  # polars.DataFrame
```

Ability cooldown and charge state over time. The dataset is **change-only**.
Boon emits a row only when the cooldown or charge state changes. One entity
exists for each ability that a player owns. These entities include movement
abilities such as jump, dash, and slide. Use `slot` to remove these abilities
from the result.

Not loaded by default. Access this property or call `load("ability_ticks")` explicitly.

| Column | Type | Description |
|--------|------|-------------|
| `tick` | `int` | The game tick of the state change |
| `hero_id` | `int` | The owning player's hero ID |
| `ability_id` | `int` | Ability subclass hash (use `ability_names()` to resolve) |
| `slot` | `int` | Ability slot (`EAbilitySlots_t`); signature abilities use small values |
| `cooldown_start` | `float` | Game time the cooldown started |
| `cooldown_end` | `float` | Game time the ability is available again |
| `remaining_charges` | `int` | Charges currently available |
| `charge_recharge_start` | `float` | Game time the regenerating charge started |
| `charge_recharge_end` | `float` | Game time the regenerating charge completes |

---

#### `urn`

```python
demo.urn  # polars.DataFrame
```

Urn lifecycle events. Boon finds urn pickup, drop, and return events in the
`ActiveModifiers` string table. It finds delivery point activation and
deactivation in `CCitadelIdolReturnTrigger` entities.

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
Boon loads this dataset on first access.

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
Boon loads this dataset on first access.

**Raises:** `NotStreetBrawlError` if the demo is not a street brawl game.

| Column | Type | Description |
|--------|------|-------------|
| `round` | `int` | Sequential round number (1-indexed) |
| `tick` | `int` | The game tick when the round ended |
| `scoring_team` | `int` | The team that scored |
| `amber_score` | `int` | The Hidden King (old name: Amber Hand) cumulative score |
| `sapphire_score` | `int` | The Archmother (old name: Sapphire Flame) cumulative score |

## Name Lookup Functions

Module-level functions resolve IDs to names without parsing a demo.
`hero_names`, `ability_names`, `ability_display_names`, `modifier_names`, and `breakable_names`
read boon-data catalogs. Each accepts an optional `version` client-version string.
Without it, Boon selects the newest verified local installation. If none exists,
it downloads the latest release automatically. Explicit missing versions are
also downloaded. Importing Boon and parsing raw IDs do not download data.

Installed data works offline. Failed acquisition or invalid catalog contents
raise `boon.data.DataError`. Use `boon versions` and `boon get VERSION` to manage
installations; use `boon get VERSION --force` to repair corrupt files.
See {doc}`data` for cache configuration and verification.

```python
from boon import ability_names, hero_names

hero_names(version="6698")
ability_names(version="6698")
```

### `hero_names()`

```python
from boon import hero_names

hero_names()  # -> dict[int, str]
```

Return hero IDs mapped to localized names. Unlocalized heroes use their internal names.

**Returns:** `dict[int, str]` -- Hero ID to hero name mapping (for example, `{1: "Infernus", 2: "Seven", ...}`).

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

The returned values are stable internal VData names such as
`upgrade_quick_silver`, not localized in-game labels.

**Returns:** `dict[int, str]` -- Ability hash to internal-name mapping.

---

### `ability_display_names()`

```python
from boon import ability_display_names

ability_display_names()  # -> dict[str, str]
```

Return exact current internal ability/item names mapped to their English
in-game display names. This includes `ability_*`, `upgrade_*`, and
`citadel_ability_*` naming schemes. For example,
`upgrade_quick_silver` maps to `"Quicksilver Reload"`, while
`ability_unicorn_luminousstrike` maps to `"Radiant Daggers"`.
`citadel_ability_hook` maps to `"Grapple Arm"`.

Hidden, test, retired, or otherwise unlocalized internal entries are omitted;
Boon never invents a display name by stripping prefixes or title-casing.

**Returns:** `dict[str, str]` -- Internal ability/item name to English display
name mapping.

---

### `breakable_names()`

```python
from boon import breakable_names

breakable_names().get(3986897915)  # -> "citadel_breakable_prop_wooden_crate"
breakable_names(version="6698")  # explicitly select a client version
```

Returns `{subclass_id: internal_name}` from `misc.json`, restricted to records
whose `definition._class` is `citadel_breakable_prop`. Other misc definitions
such as pickups are excluded. IDs absent from the catalog are not in the map;
`demo.breakables.subclass_name` uses `"BREAKABLE_NOT_FOUND"` for them.

### `game_mode_names()`

```python
from boon import game_mode_names

game_mode_names()  # -> dict[int, str]
```

Return a mapping of game mode ID to game mode name.

**Returns:** `dict[int, str]` -- Game mode ID to name mapping (for example, `{1: "6v6", 4: "street_brawl"}`).

---

### `modifier_names()`

```python
from boon import modifier_names

modifier_names()  # -> dict[int, str]
```

Return unqualified and owner-qualified modifier IDs mapped to their corresponding
names. For example, an owner-qualified token resolves to
`ability_afterburn/modifier_afterburn_dot`. Repeated records with the same ID and
name share one entry. Conflicting names raise `DataError`; use the raw boon-data
record indexes to get all candidate definitions and their effects.

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

**Returns:** `dict[int, str]` -- Patron phase ID to name mapping (for example, `{0: "normal", 1: "final", 2: "transforming"}`).

---

### `hitgroup_names()`

```python
from boon import hitgroup_names

hitgroup_names()  # -> dict[int, str]
```

Return a mapping of hit group ID to hit group name, for resolving the
`hitgroup_id` column on the `damage` frame. Values come from Source 2's `HitGroup_t` enum.
The map includes `0=generic`, `1=head`, `2=chest`, and `3=stomach`.
Limb values are `4=left_arm`, `5=right_arm`, `6=left_leg`, and `7=right_leg`.
Other values include `8=neck`, `10=gear`, `11=special`, `19=head_no_resist`,
and `-1=invalid`. Values `12`–`18` identify tier-2 and drone boss weakpoints. The `HITGROUP_COUNT` sentinel is omitted.

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

These functions calculate metrics from parsed demo data. Each function takes
a [`Demo`](#demo) and returns a Polars DataFrame. Most results use `hero_id`
for joins with other datasets. `teamfights()` returns one row per fight.
Each function also has a `Demo` method. For example,
`demo.kill_participation()` calls `boon.stats.kill_participation(demo)`.

### `kill_participation()`

```python
from boon import stats

stats.kill_participation(demo)                              # whole match
stats.kill_participation(demo, start_tick=0, end_tick=18000)  # windowed
demo.kill_participation()                                   # equivalent method form
```

Each player's `(kills + assists) / team_kills`.
A kill credits a player as either the killer or an assister, never both.
The ratio is in `[0, 1]`, or null if the team has no kills.
Use `start_tick` and `end_tick` to select a window.
The denominator counts team kills in that same window.

**Returns:** `polars.DataFrame` with columns `hero_id`, `team_num`, `kills`,
`assists`, `team_kills`, `kill_participation` (see the
[`Demo.kill_participation()`](#kill-participation) table), one row per player,
sorted by `team_num` then `hero_id`.

### `time_dead()`

```python
from boon import stats

stats.time_dead(demo)   # equivalently: demo.time_dead()
```

Time each player spent dead during regulation. A player is dead when
`is_alive == False`. The function counts only unpaused ticks up to game over.
The totals use the same time limits as `demo.regulation_ticks` and
`demo.regulation_seconds`.

**Returns:** `polars.DataFrame` with columns `hero_id`, `team_num`,
`ticks_dead`, `seconds_dead`, `pct_regulation_dead` (see the
[`Demo.time_dead()`](#time-dead) table), one row per player, sorted by
`team_num` then `hero_id`.

**Raises:** `ValueError` — if the demo has no game-over event.

### `in_combat()`

```python
from boon import stats

stats.in_combat(demo)   # equivalently: demo.in_combat()
```

Reports the combat state of each player for each tick. Boon uses the pawn's
`in_combat_end_time` value in `player_ticks`. Boon calculates the current game
time from non-paused ticks and a constant offset. Damage events set the offset.

**Returns:** `polars.DataFrame` with columns `tick`, `hero_id`, `in_combat` (see
the [`Demo.in_combat()`](#in-combat) table), one row per `(tick, hero_id)`,
sorted by `tick` then `hero_id`.

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

Raised when a requested protobuf message is absent or cannot be decoded, such
as post-match details or a malformed kill/damage event.

---

### `NotStreetBrawlError`

```python
from boon import NotStreetBrawlError
```

Raised when accessing street brawl datasets (`street_brawl_ticks`, `street_brawl_rounds`) on a demo that is not a street brawl game (game_mode != 4).
