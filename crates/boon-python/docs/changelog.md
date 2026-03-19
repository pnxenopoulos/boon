# Changelog

## Unreleased

### boon-python (breaking)

- **Breaking:** Removed `hero` and `team` string columns from `players` DataFrame. Use `hero_names()` and `team_names()` to resolve IDs to names.
- **Breaking:** Removed `teams` DataFrame property. Use `team_names()` module-level function instead.
- **Breaking:** Removed `winning_team` property. Use `winning_team_num` with `team_names()`.
- **Breaking:** Removed `banned_heroes` property. Use `banned_hero_ids` with `hero_names()`.
- **Breaking:** Moved `Demo.hero_names()` and `Demo.team_names()` from static methods to module-level functions `hero_names()` and `team_names()`. Import directly from `boon`.
- Added `hero_names()` module-level function returning `dict[int, str]` of hero ID to name.
- Added `team_names()` module-level function returning `dict[int, str]` of team number to name.
- Added `ability_names()` module-level function returning `dict[int, str]` of ability hash ID to name.
- Added `modifier_names()` module-level function returning `dict[int, str]` of modifier hash ID to name.
- Added `modifier_id` and `ability_id` columns to `active_modifiers` DataFrame (raw `u32` hash values).

### boon-cli

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
- All new commands support `--filter`, `--summary`, `--limit`, and `--json` flags.

### boon-python

- `Demo.ability_upgrades` property for hero ability point spending events.
- `Demo.shop_events` property for item shop transactions.
- `Demo.chat` property for in-game chat messages.
- `Demo.objectives` property for per-tick objective entity health.
- `Demo.boss_kills` property for objective destruction events.
- `Demo.mid_boss` property for mid boss lifecycle events.
- `Demo.troopers` property for per-tick alive lane trooper state (opt-in, large dataset).
- `Demo.neutrals` property for neutral creep state changes with change detection (opt-in).
- `Demo.stat_modifiers` property for per-player cumulative permanent stat bonuses (opt-in).
- `Demo.active_modifiers` property for active buff/debuff modifier events (opt-in).
- All new datasets are lazy-loaded on first access and can be batch-loaded via `load()`.

---

## boon-python 0.0.1

- Ability name resolution for item purchases using MurmurHash2 lookup
  from `abilities.vdata`. The `purchases` DataFrame now has an `ability`
  string column instead of a numeric `ability_id`.
- `Demo.purchases` property for item purchase/sell events.
- `Demo.kills` property for hero kill events with attacker, victim, and assisters.
- `Demo.damage` property for damage events with pre/post mitigation, hitgroups, and crit damage.
- `Demo.respawns` property for player respawn events.
- `Demo.flex_slots` property for flex slot unlock events.
- `Demo.teams` property for team number to team name mapping.
- `Demo.winning_team_num`, `Demo.game_over_tick`, and `Demo.winning_team`
  properties for game-over state (lazy-scanned on first access).
- `Demo.banned_hero_ids` and `Demo.banned_heroes` properties for hero bans.
- `Demo.load()` method to batch-load multiple datasets in a single parse pass.
- All DataFrame properties (`player_ticks`, `world_ticks`, `kills`, `damage`,
  `purchases`, `respawns`, `flex_slots`) now auto-load on first access.
- `events` CLI command for listing and inspecting decoded game events.
- `Demo` class with metadata properties: `path`, `total_ticks`, `total_seconds`,
  `total_clock_time`, `build`, `map_name`, `match_id`, `tick_rate`.
- `Demo.players` property returning a Polars DataFrame of player info.
- `Demo.player_ticks` property returning per-tick, per-player state (48 columns).
- `Demo.world_ticks` property returning per-tick world state.
- Custom exceptions: `InvalidDemoError`, `DemoHeaderError`, `DemoInfoError`,
  `DemoMessageError`.
- CLI with commands: `verify`, `info`, `messages`, `classes`, `send-tables`,
  `string-tables`, `entities`.
