# 📝 Changelog

## 0.4.0

### boon-python

- New `boon.stats` module — derived metrics computed from parsed demo data, keyed on `hero_id`. Each is also a thin `Demo` method. Initial metrics:
  - `kill_participation(demo)` (`Demo.kill_participation()`) — `(kills + assists) / team_kills` per player (a `[0, 1]` fraction), with an optional `start_tick` / `end_tick` window.
  - `time_dead(demo)` (`Demo.time_dead()`) — per-player `ticks_dead`, `seconds_dead`, and `pct_regulation_dead`, counting only non-paused ticks up to game-over so totals align with `regulation_ticks` / `regulation_seconds`.
- `hitgroup_names()` returning `dict[int, str]` for the `hitgroup_id` column on `damage`. Values are Source 2's `HitGroup_t` enum (`-1=invalid`, `0=generic`, `1=head`, … `19=head_no_resist`; the `HITGROUP_COUNT` sentinel is omitted).
- `lifestate_names()` returning `dict[int, str]` for the `lifestate` column on `player_ticks`. Values are Source 2's `LifeState_t` enum (`0=alive`, `1=dying`, `2=dead`, `3=respawnable`, `4=respawning`).
- **Changed:** `patron_phase_names()` renames patron phase `2` from `shields_down` to `transforming`. The raw `phase` integer is unchanged (still `2`); update any code comparing the resolved string against `"shields_down"`.

### boon-cli

- No functional changes; the version is bumped in step with the workspace.

### boon

- New `hitgroups` and `lifestates` lookup tables: `hitgroup_name(id)` / `all_hitgroups()` and `lifestate_name(id)` / `all_lifestates()`, mapping Source 2's `HitGroup_t` and `LifeState_t` enums to names and re-exported at the crate root. These back the Python `hitgroup_names()` / `lifestate_names()`.
- **Changed:** `patron_phase_name(2)` now returns `transforming` (was `shields_down`).

## 0.3.0

### boon-python

- **Faster:** all parsing is a few percent quicker — packet messages the parser doesn't consume (sounds, temp entities, etc.) are now skipped in place instead of copied out of the bitstream first.
- **Fixed & faster:** `active_modifiers` (and the idol events in `urn`) emitted duplicate `applied`/`removed` rows on nearly every tick. Source 2 keeps a modifier's original `ActiveModifiers` entry and adds a separate `entry_type=2` removal entry (the table never shrinks), so the old per-tick full-table rescan re-applied and re-removed it indefinitely — ~99% of rows were per-tick duplicates (e.g. 3.16M rows where ~41k were real). The scan now processes only the entries each string-table delta touches, reporting each modifier once applied and once removed. Much faster too: the ActiveModifiers decode dropped from ~7.3s to ~0.06s (full dataset load ~18s → ~5s). `urn` output is unchanged apart from dropping the duplicate idol events.
- **Fixed:** `modifier_names()` resolved only ~87 modifiers — the generic ones defined as top-level keys in `modifiers.vdata`; most gameplay modifiers are nested `subclass:` blocks the generator never scanned. It now unions every top-level key in `modifiers.vdata`, every nested `_my_subclass_name` there, and the `_my_subclass_name` of each modifier subclass in `abilities.vdata` (those whose `_class` starts with `modifier_`) — ~917 entries. This is the right field because a demo identifies a modifier by the `modifier_subclass` token on `CModifierTableEntry`, the `CUtlStringToken` (`MurmurHash2`) of its `_my_subclass_name`. Many modifiers live only in engine/C++ code and appear in no vdata file, so a share of `modifier_id` values stay `MODIFIER_NOT_FOUND` (name-list-bound, not a hashing limitation).
- `ability_names()`, `modifier_names()`, and `hero_names()` reflect the latest Deadlock build (`6557`). The hero table gains Raven (`hero_operative`, id 62) and a test hero (id 83), and renames id 82 (`hero_opera`) from "Raven" to "Opera" — Valve moved "Raven" onto the new slot. The ability table also tracked a few upstream removals.
- **Fixed:** the `max_health` column on `player_ticks` reported the pawn's `m_iMaxHealth`, a stale base value that current health exceeds on over half of all ticks (e.g. `817` vs a reported max of `780`). It now reads the controller's `m_PlayerDataGlobal.m_iHealthMax` — the live effective max (level growth, items, buffs) — falling back to the pawn value only before the controller is populated. The `health` column is unchanged.

### boon-cli

- **Fixed & faster:** the `active-modifiers` command had the same per-tick flicker — re-emitting an `applied`/`removed` pair for stale entries on nearly every tick (e.g. 435,795 events where ~5,763 were real). It now processes only the entries each string-table delta touches, reporting each modifier once applied and once removed, and runs much faster.

### boon-proto

- Synced protobuf definitions to the latest Deadlock build (`6536` → `6557`); `boon-proto` is now `0.2.10717574+6557`. The notable change is a new `CMsgServerSignoutData_DetailedStats.UrnCapture` message (per-urn post-match stats) added as a repeated `urn_captures` field. An earlier build also dropped a redundant `[default = 0]` from two `usermessages.proto` `uint32` fields — a no-op, since `0` is already the implicit default.

## 0.2.0

### boon-python

- `Demo.summary()` method returning the post-match summary as a dict with `snapshots`, `last_hits`, `objectives`, and `damage` keys. `snapshots` is a Polars DataFrame with one row per (snapshot, player) — a `snapshot_time_s` column plus per-player running totals (kills/deaths/assists, net worth, denies, level, lane, creep/neutral kills, player damage, and the per-source gold/orbs breakdown). `last_hits` is a Polars DataFrame of `hero_id` and `last_hits` (the final scoreboard last-hit total, only recorded per match). `objectives` is a Polars DataFrame of post-match objective records (destruction time and damage taken). `damage` is a Polars DataFrame of the damage matrix in long form — one row per (dealer, target, source, sample) with dealer/target as both `*_player_slot` and resolved `*_hero_id` (null for non-player slots, joinable to the other frames on `hero_id`), the per-interval (additive) `damage` per `stat_type` (a readable string), an `is_category` flag distinguishing coarse damage-type buckets from specific sources, and `sample_time_s`. Filter to `is_category == False` and `sum` for totals, or `cumsum` over `sample_time_s` for the running total.
- `Demo.regulation_ticks`, `Demo.regulation_seconds`, `Demo.regulation_clock_time` properties for the duration of actual gameplay (active, paused-time-excluded ticks up to the game-over event), distinct from the full-recording `total_ticks`/`total_seconds`/`total_clock_time`. Return `None` when no game-over event is present.
- `patron_phase_names()` module-level function returning `dict[int, str]` of patron phase ID to name (`0=normal`, `1=final`, `2=shields_down`) for the `phase` column on patron objective rows.
- **Fixed:** the `start_lane` / `lane` column docs previously claimed `1=left, 4=center, 6=right`, which is wrong — the values are `CMsgLaneColor` color IDs and `3=green` was also missing. Docs now correctly read `1=yellow, 3=green, 4=blue, 6=purple, 0=none`.
- **Fixed:** `player_ticks` dropped most players, often leaving only one hero. Player controllers link to their pawn through a `CHandle` whose entity index is the low 14 bits; the index was masked with `0x7FFF` (15 bits) instead of `0x3FFF`, so any handle with an odd serial resolved to the wrong entity and that player was silently skipped. The mask is now `0x3FFF`, and `player_ticks` again covers every player on the roster.
- **Fixed:** the `x` / `y` / `z` columns on `player_ticks`, `objectives`, `troopers`, `neutrals`, and `urn` previously emitted only the in-cell offset half of Source 2's split position storage — values bounded to `[0, 512)` that reset to `0` every time the entity crossed a cell boundary, producing a sawtooth instead of a trajectory. They now emit full world (Hammer-unit) positions, combining the networked `m_cellX/Y/Z` cell index with the `m_vecOrigin.m_vec{X,Y,Z}` offset via the new `boon::position::cell_to_world` helper. No display-side scaling is applied; downstream plotters supply their own map projection.

### boon-cli

- `summary` command for post-match details: a match overview, a timing section (total ticks/time and tick rate from the recording, the game-over tick, and the regulation/gameplay duration), each player's final snapshot (with the scoreboard last-hit total), and an objectives table mirroring the Python `summary()` `objectives` frame. `--json` dumps the full decoded metadata.
- Name resolution reflects the refreshed ability and modifier name tables from the latest Deadlock build.
- **Fixed:** the `neutrals` and `troopers` commands' `x` / `y` / `z` columns now report full world coordinates (Hammer units) instead of just the in-cell offset half of Source 2's split position storage — see the matching Python fix above.

### boon-proto

- Synced protobuf definitions to the latest Deadlock build and regenerated the ability and modifier name lookup tables (surfaced via `ability_names()` and `modifier_names()`).
- Versioned independently from the rest of the workspace to track the game build: `MAJOR.MINOR.<SourceRevision>+<GameBuild>` (e.g. `0.2.10691905+6536`). The monotonic `SourceRevision` is the patch, so each proto sync yields a higher, publishable version while staying compatible within the `0.2` line.

## 0.1.0

### boon-python (breaking changes from pre-release)

- **Breaking:** Removed `hero` and `team` string columns from `players` DataFrame. Use `hero_names()` and `team_names()` to resolve IDs to names.
- **Breaking:** Removed `teams` DataFrame property. Use `team_names()` module-level function instead.
- **Breaking:** Removed `winning_team` property. Use `winning_team_num` with `team_names()`.
- **Breaking:** Removed `banned_heroes` property. The `k_EUserMsg_BannedHeroes` event is no longer reliably present in GOTV demo recordings (see Known Limitations).
- **Breaking:** Moved `Demo.hero_names()` and `Demo.team_names()` from static methods to module-level functions `hero_names()` and `team_names()`. Import directly from `boon`.
- **Breaking:** `purchases` and `shop_events` datasets merged into `item_purchases`. Columns: `tick`, `hero_id`, `ability_id`, `change`.
- **Breaking:** `ability` column removed from `ability_upgrades`. Use `ability_names()` to resolve `ability_id`.
- **Breaking:** `modifier` and `ability` columns removed from `active_modifiers`. Use `modifier_names()` and `ability_names()` to resolve IDs.

### boon-python

- `Demo` class with metadata properties: `path`, `total_ticks`, `total_seconds`, `total_clock_time`, `build`, `map_name`, `match_id`, `tick_rate`, `game_mode`.
- `Demo.players` property returning a Polars DataFrame of player info.
- `Demo.player_ticks` property returning per-tick, per-player state (48 columns).
- `Demo.world_ticks` property returning per-tick world state.
- `Demo.kills` property for hero kill events with attacker, victim, and assisters.
- `Demo.damage` property for damage events with pre/post mitigation, hitgroups, and crit damage.
- `Demo.flex_slots` property for flex slot unlock events.
- `Demo.abilities` property for important ability usage events.
- `Demo.ability_upgrades` property for hero ability point spending events.
- `Demo.item_purchases` property for item shop transactions.
- `Demo.chat` property for in-game chat messages.
- `Demo.objectives` property for objective health state changes.
- `Demo.mid_boss` property for mid boss lifecycle events.
- `Demo.troopers` property for per-tick alive lane trooper state (opt-in, large dataset).
- `Demo.neutrals` property for neutral creep state changes with change detection (opt-in).
- `Demo.stat_modifier_events` property for permanent stat bonus change events (opt-in).
- `Demo.active_modifiers` property for active buff/debuff modifier events (opt-in).
- `Demo.urn` property for urn (idol) lifecycle events (picked up, dropped, returned) and delivery point tracking (active, inactive with position and team).
- `Demo.street_brawl_ticks` property for per-tick street brawl state (round, scores, state transitions).
- `Demo.street_brawl_rounds` property for street brawl round scoring events.
- `NotStreetBrawlError` exception raised when accessing street brawl datasets on non-street-brawl demos.
- `Demo.winning_team_num`, `Demo.game_over_tick` properties for game-over state (lazy-scanned on first access).
- `Demo.available_datasets()` static method returning the list of valid dataset names.
- `Demo.load()` method to batch-load multiple datasets in a single parse pass.
- All DataFrame properties auto-load on first access and can be batch-loaded via `load()`.
- `hero_names()` module-level function returning `dict[int, str]` of hero ID to name.
- `team_names()` module-level function returning `dict[int, str]` of team number to name.
- `ability_names()` module-level function returning `dict[int, str]` of ability hash ID to name.
- `modifier_names()` module-level function returning `dict[int, str]` of modifier hash ID to name.
- `game_mode_names()` module-level function returning `dict[int, str]` of game mode ID to name.
- Custom exceptions: `InvalidDemoError`, `DemoHeaderError`, `DemoInfoError`, `DemoMessageError`.

### boon-cli

- CLI with commands: `verify`, `info`, `messages`, `classes`, `send-tables`, `string-tables`, `entities`, `events`.
- `ability-upgrades` command for tracking hero ability point spending (skill tier upgrades).
- `shop-events` command for item shop transactions (purchased, upgraded, sold, swapped, failure).
- `chat` command for in-game chat messages (all chat and team chat).
- `objectives` command for per-tick objective entity health (walkers, titans, barracks, mid boss).
- `mid-boss` command for mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire).
- `troopers` command for per-tick alive lane trooper state (position, health, lane).
- `neutrals` command for neutral creep state changes with change detection.
- `stat-modifiers` command for per-player cumulative permanent stat bonuses.
- `active-modifiers` command for active buff/debuff modifier events.
- All commands support `--filter`, `--summary`, `--limit`, and `--json` flags.
