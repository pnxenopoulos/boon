# ❓ FAQ

## Where do I get demo files?

Deadlock demo files (`.dem`) are GOTV match recordings. Download them from the in-game match history or from a replay service.
The file name usually contains the match ID. For example, `70555151.dem` contains match 70555151.

## Why does boon return Polars DataFrames instead of pandas?

[Polars](https://pola.rs) is fast for common replay analysis operations. These operations include filters, groups, and joins.
Polars also uses memory efficiently. Use `df.to_pandas()` when you need a pandas DataFrame.

## Why do DataFrames use integer IDs instead of names?

Raw IDs keep the data compact. They also make filters, groups, and joins fast. IDs do not change when Valve renames a hero or ability.
Use `hero_names()`, `team_names()`, `ability_names()`, and `modifier_names()` to resolve IDs. See {doc}`examples` for examples.

## How do I see what datasets are available?

Call `Demo.available_datasets()` to get all dataset names. You can pass these names to `load()` or access them as properties.

## What is the difference between a property and `load()`?

Accessing a property parses that dataset on first access. For example, `demo.kills` parses the kills dataset.
`load("kills", "damage", "player_ticks")` parses multiple datasets in one pass. Boon caches the result after either operation.

## Why is `player_ticks` missing some heroes?

GOTV recordings do not always include all player pawns. Boon can return data only for pawns that are in the demo.

## Why is `ability_upgrades` empty?

Valve renamed `m_nUpgradeBits` to `m_nUpgradeInfo` and changed its encoding. Boon uses the current field name. Older demos return an empty DataFrame. See {doc}`known-issues`.

## What is `trooper_boss`?

In the `troopers` dataset, `trooper_type` is `"trooper"` for a regular lane creep. It is `"trooper_boss"` for the lane guardian.

## How do I work with street brawl demos?

Street brawl is game mode 4. Check the mode with `demo.game_mode`. Street brawl demos have `street_brawl_ticks` and `street_brawl_rounds`. Other modes raise `NotStreetBrawlError` when you access these datasets.

## How do I convert a tick to a timestamp?

Use `demo.tick_to_seconds(tick)` or `demo.tick_to_clock_time(tick)`. Both methods exclude paused time. They load `world_ticks` on the first call.

## What does `damage` include?

The `damage` dataset includes all recorded damage events. It includes damage to heroes, objectives, troopers, and neutral units. Filter `attacker_hero_id` or `victim_hero_id` to select the required events.

## Can I use boon without Python?

Yes. The core parser is the `boon-deadlock` Rust crate on [crates.io](https://crates.io/crates/boon-deadlock).
The repository also contains the low-level `boon-dev` debug tool. Build it with `cargo build --release -p boon-dev`. See {doc}`cli`. The `boon` command in the Python package requires Python.

## Something is not working. Where do I report it?

Check {doc}`known-issues` first. If the page does not list the problem, create a [GitHub issue](https://github.com/pnxenopoulos/boon/issues) or ask in [Discord](https://discord.gg/WmjZHxWrCD).
