# 📝 Changelog

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
