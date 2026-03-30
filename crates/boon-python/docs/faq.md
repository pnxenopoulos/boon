# ❓ FAQ

## Where do I get demo files?

Deadlock demo files (`.dem`) are GOTV recordings of matches. You can download them from the in-game match history or from third-party sites that host replays. The files are typically named by match ID (e.g., `70555151.dem`).

## Why does boon return Polars DataFrames instead of pandas?

[Polars](https://pola.rs) is significantly faster than pandas for the kind of operations common in replay analysis (filtering, grouping, joins). It also has lower memory usage and a more expressive API. If you need pandas, you can convert with `df.to_pandas()`.

## Why do DataFrames use integer IDs instead of names?

Keeping raw IDs makes the data compact and fast to filter, group, and join. IDs are also stable — they don't change if Valve renames a hero or ability. Use the module-level mapping functions (`hero_names()`, `team_names()`, `ability_names()`, `modifier_names()`) to resolve IDs to names when you need them. See {doc}`examples` for patterns.

## How do I see what datasets are available?

Call `Demo.available_datasets()` to get the full list of dataset names you can pass to `load()` or access as properties.

## What's the difference between accessing a property and calling `load()`?

Accessing a property (e.g., `demo.kills`) parses that dataset on first access. Calling `load("kills", "damage", "player_ticks")` parses multiple datasets in a single pass over the file, which is faster when you need several datasets. After either approach, the data is cached — subsequent accesses are free.

## Why is `player_ticks` missing some heroes?

GOTV demo recordings don't always include all player pawns. The parser can only return data for pawns that are present in the demo. This is a limitation of the demo format, not boon.

## Why is `ability_upgrades` empty?

Valve renamed the underlying entity field from `m_nUpgradeBits` to `m_nUpgradeInfo` in a recent update. Boon uses the current field name, so older demos will return an empty DataFrame. See {doc}`known-issues` for details.

## What is `trooper_boss`?

In the `troopers` dataset, `trooper_type` is either `"trooper"` (regular lane creeps) or `"trooper_boss"` (the lane guardian).

## How do I work with street brawl demos?

Street brawl is game mode 4. You can check with `demo.game_mode`. Street brawl demos have two additional datasets: `street_brawl_ticks` and `street_brawl_rounds`. Accessing these on a non-street-brawl demo raises `NotStreetBrawlError`.

## How do I convert a tick to a timestamp?

Use `demo.tick_to_seconds(tick)` or `demo.tick_to_clock_time(tick)`. Both exclude paused time and automatically load `world_ticks` on first call.

## What does `damage` include?

The `damage` dataset includes all damage events in the game — hero vs hero, hero vs objectives, troopers, neutrals, and everything else. Filter by `attacker_hero_id` or `victim_hero_id` to focus on what you need.

## Can I use boon without Python?

Yes. The `boon-cli` command-line tool lets you inspect demos without writing code. See {doc}`cli`. The core parser is also available as a Rust crate (`boon-deadlock`) on [crates.io](https://crates.io/crates/boon-deadlock).

## Something isn't working. Where do I report it?

Check {doc}`known-issues` first. If your problem isn't listed, file a [GitHub issue](https://github.com/pnxenopoulos/boon/issues) or ask in the [Discord](https://discord.gg/WmjZHxWrCD).
