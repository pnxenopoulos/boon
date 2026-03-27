# Changelog

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
- `Demo.respawns` property for player respawn events.
- `Demo.flex_slots` property for flex slot unlock events.
- `Demo.abilities` property for important ability usage events.
- `Demo.ability_upgrades` property for hero ability point spending events.
- `Demo.item_purchases` property for item shop transactions.
- `Demo.chat` property for in-game chat messages.
- `Demo.objectives` property for per-tick objective entity health.
- `Demo.boss_kills` property for objective destruction events.
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
- `boss-kills` command for objective destruction events.
- `mid-boss` command for mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire).
- `troopers` command for per-tick alive lane trooper state (position, health, lane).
- `neutrals` command for neutral creep state changes with change detection.
- `stat-modifiers` command for per-player cumulative permanent stat bonuses.
- `active-modifiers` command for active buff/debuff modifier events.
- All commands support `--filter`, `--summary`, `--limit`, and `--json` flags.
