# Changelog

## Unreleased

### boon (parser)

_No changes yet._

### boon-cli

_No changes yet._

### boon-python

_No changes yet._

---

## boon-python 0.1.0

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
- `Demo.player_ticks` property returning per-tick, per-player state (50 columns).
- `Demo.world_ticks` property returning per-tick world state.
- Custom exceptions: `InvalidDemoError`, `DemoHeaderError`, `DemoInfoError`,
  `DemoMessageError`.
- CLI with commands: `verify`, `info`, `messages`, `classes`, `send-tables`,
  `string-tables`, `entities`.
