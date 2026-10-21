<div align="center">

# boon-deadlock

[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![docs.rs](https://docs.rs/boon-deadlock/badge.svg)](https://docs.rs/boon-deadlock)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

```rust
use boon::Parser;
use std::path::Path;

fn main() -> boon::Result<()> {
    let parser = Parser::from_file(Path::new("match.dem"))?;
    println!("Map: {:?}", parser.file_header()?.map_name);
    Ok(())
}
```

Boon parses Valve [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/)
Source 2 `.dem` files in Rust. The Python package uses PyO3 bindings and returns
Polars DataFrames. The Rust API reads entities, game events, and match metadata.

Part of the [Boon](https://github.com/pnxenopoulos/boon) project.

**Demo compatibility:** Use Boon **0.10.0 or earlier** for demos recorded before
the **City Never Sleeps** update (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos recorded with that update or later.

<a id="features"></a>

## Boon Rust features

- Memory-mapped input and selected entity decoding
- Match metadata (map, players, duration, build number)
- Full entity state at any tick with snapshot seeking
- Stable entity identities and typed update/PVS-leave/delete lifecycle events
- Game event extraction with protobuf decoding
- Tick streaming with entity class and property filters
- Recorded upgrade tokens and team enemy minimap observations
- Versioned hero, ability/item, modifier, and breakable names from boon-data
- Hero and ability stat queries, recorded states, and item imbues

<a id="installation"></a>

## Install the Boon Rust crate

Add to your `Cargo.toml`:

```toml
[dependencies]
boon-deadlock = "0.19"
```

Use Rust 1.88 or later (edition 2024).

<a id="quick-start"></a>

## Read Deadlock match metadata with Boon

```rust,no_run
use std::path::Path;
use boon::Parser;

let parser = Parser::from_file(Path::new("match.dem"))?;
parser.verify()?;

// File header
let header = parser.file_header()?;
println!("Map: {:?}", header.map_name);
println!("Build: {:?}", header.build_num);

// File info (playback time, players)
let info = parser.file_info()?;
println!("Duration: {:?}s", info.playback_time);
# Ok::<(), boon::Error>(())
```

<a id="api-overview"></a>

## Boon Rust parser and entity APIs

### `Parser`

The main entry point. Owns the demo file data (memory-mapped or in-memory).

| Method | Description |
|--------|-------------|
| `Parser::from_file(path)` | Open and memory-map a `.dem` file |
| `Parser::from_bytes(bytes)` | Parse from an in-memory buffer |
| `verify()` | Compare magic bytes |
| `file_header()` | Decode `CDemoFileHeader` (map, server, build) |
| `file_info()` | Decode `CDemoFileInfo` (duration, players) |
| `messages()` | List all command headers in the file |
| `events(max_tick)` | Extract game events (legacy + Citadel user messages) |
| `parse_init()` | Read the initial replay context |
| `parse_to_tick(tick)` | Read entity state at a specific tick |
| `run_to_end(callback)` | Stream every tick with a callback |
| `run_to_end_filtered(filter, callback)` | Stream with an entity class filter |
| `run_to_end_with_properties(classes, properties, event_types, callback)` | Stream selected entity fields and events |
| `calculate_stats(batch, catalog)` | Query hero stats, ability stats, and imbues in one pass |
| `calculate_hero_stats(query, catalog, rules)` | Calculate selected hero stats |
| `calculate_ability_stats(query, catalog, rules)` | Calculate ability bonus percentages |
| `player_states(query, catalog)` | Read named player states and unknown bits |
| `imbues(query, catalog)` | Read item selections and catalog effects |

### `Context`

Returned by `parse_init`, `parse_to_tick`, and passed to tick callbacks. Contains:

- `entities()` &mdash; recorded entity state (`EntityContainer`)
- `serializers()` &mdash; field definitions for each class
- `class_info()` &mdash; class ID to name mappings
- `string_tables()` &mdash; key-value tables (models, baselines, etc.)
- `tick()` &mdash; current tick
- `tick_interval()` &mdash; seconds for each tick

### `Entity`

A single networked entity with class name and decoded field values.

```rust,no_run
# fn inspect(entity: &boon::Entity, serializer: &boon::Serializer) {
// Look up fields by dotted path
let health = entity.get_by_name("m_iHealth", serializer);
let x = entity.get_by_name(
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
    serializer,
);
# }
```

<a id="helper-functions"></a>

### Boon Rust entity helper functions

- `CatalogNames::load(version)` &mdash; load verified boon-data names, downloading missing data
- `CatalogNames::ability_display_name(internal_name)` &mdash; resolve an internal ability/item name to its English label
- `CatalogNames::breakable_name(id)` &mdash; resolve a breakable subclass hash to its name
- `CatalogNames::modifier_name(id)` &mdash; resolve a modifier hash to its name
- `StatModifierTypes::load(version)` &mdash; resolve recorded stat types from boon-data enum definitions
- `decode_event_payload(msg_type, data)` &mdash; decode a game event's protobuf payload

## Stat and state queries

Load the matching client version with `StatCatalog::load("VERSION")` or `StateCatalog::load("VERSION")`.
These loaders use the cache from `boon get` and download missing versions.
Reuse loaded catalogs across queries. Select players with Steam IDs from the replay.
Use `StatMode::Current` for supported active effects or `StatMode::Baseline` for passive and permanent inputs.
Use `StatBatch` with `calculate_stats` to combine hero stats, ability stats, and imbues in one replay pass.
Each query keeps its own ticks, player selection, and mode.
Both catalogs also accept local data through `from_directory` or `from_json_bytes`.
These constructors do not download data or verify checksums.

See the [hero stat guide](https://boon.readthedocs.io/en/latest/hero-stats.html#calculate-hero-stats-with-the-boon-rust-api),
[ability stat guide](https://boon.readthedocs.io/en/latest/ability-stats.html#calculate-ability-bonuses-with-the-boon-rust-api), and
[player state guide](https://boon.readthedocs.io/en/latest/player-states.html#read-player-states-with-the-boon-rust-api) for complete Rust examples.
The guides list enum members, units, equations, and calculation limits.

### Effective modifier lifetimes

Use the same catalog as your stat queries:

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let parser = boon::Parser::from_file(std::path::Path::new("match.dem"))?;
let catalog = boon::hero_stats::StatCatalog::load("VERSION")?;
let initial = parser.parse_init()?;
let clock = boon::ModifierClock::resolve(&initial);
let mut modifiers = boon::EffectiveModifierState::with_catalog(&catalog);
modifiers.rebuild(&initial, clock.game_time(&initial));
parser.run_to_end(|ctx| {
    for change in modifiers.update(ctx, clock.game_time(ctx)) {
        // Use these changes to build modifier spans.
        println!("{} {} {}", ctx.tick(), change.serial, change.kind.as_str());
    }
    // modifiers.entries(): effective applications.
    // modifiers.raw_entries(): merged replay rows, including ended effects.
})?;
# Ok(())
# }
```

Catalog rules require observed state transitions or source-ability deletions.
Missing evidence does not end an effect. A partial value update cannot revive
an ended application; a new application time or full handle can.
`Default` uses duration and aura rules only. It does not load a catalog.
Replay from signon for exact history; a keyframe cannot recover earlier evidence.
Python `demo.active_modifiers` and `boon-dev active-modifiers` use local game data
and download the latest data if none is installed.

### Urn carrier events

Use `boon::urn::UrnState::with_catalog(&catalog)` for Urn carrier events.
Pass the shared modifier changes, modifier time, and observed delivery-point closure to `update()`.
The tracker reads IDs from the catalog. Entry into the delivery area does not prove a return.

## Recorded replay inputs

Use `boon::replay_inputs::UpgradeReader` to read controller upgrade tokens.
Use `boon::replay_inputs::EnemyMinimap` to read each team's enemy minimap observations.
Minimap positions use native 0–255 coordinates, not world coordinates.
Update the reader at every tick to keep last-seen history. Missing fields remain unknown.

## Recorded ziplines

Use `boon::ziplines::ZiplineHistory::build(&parser)` to read compact route and
ownership histories without a catalog. `nodes()` returns initial geometry and
later revisions. `changes()` returns ownership and visibility transitions.
Use `visit_snapshots(&[tick], callback)` for state after selected ticks.
Full handles keep entity-slot reuse separate. Missing fields stay unknown.

<a id="examples"></a>

## Run the Boon Rust examples

Runnable examples are in [`examples/`](examples/). Each accepts a demo file path as a CLI argument.

```bash
# Match metadata
cargo run -p boon-deadlock --example info -- match.dem

# Game events (optionally filtered by name)
cargo run -p boon-deadlock --example events -- match.dem Damage

# Entity snapshot at a specific tick
cargo run -p boon-deadlock --example entities -- match.dem 5000

# Stream all ticks with a class filter
cargo run -p boon-deadlock --example player_ticks -- match.dem

# Hero stats, ability bonuses, and imbues in one pass
cargo run -p boon-deadlock --example stat_batch -- match.dem VERSION 5000
```

| Example | What it shows |
|---------|---------------|
| [`info`](examples/info.rs) | `file_header()`, `file_info()`, match metadata and player list |
| [`events`](examples/events.rs) | `events()`, event filtering, `decode_event_payload()` |
| [`entities`](examples/entities.rs) | `parse_to_tick()`, entity iteration, `get_by_name()`, `CatalogNames::ability_name()` |
| [`player_ticks`](examples/player_ticks.rs) | `run_to_end_filtered()`, `resolve_field_key()`, streaming at each tick |
| [`hero_stats`](examples/hero_stats.rs) | Ammo capacity and source details from a catalog version |
| [`ability_stats`](examples/ability_stats.rs) | Ability bonus percentages and imbues from a catalog directory |
| [`stat_batch`](examples/stat_batch.rs) | Hero stats, ability bonuses, and imbues in one replay pass |

<a id="performance"></a>

## Select Boon Rust replay data

Use `run_to_end_filtered` to select entity classes.
Use `run_to_end_with_properties` to select fields within those classes:

```rust,no_run
use std::collections::HashSet;
use std::path::Path;
use boon::Parser;

let parser = Parser::from_file(Path::new("match.dem"))?;
let filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();
let properties: &[(&str, &[&str])] = &[("CCitadelPlayerPawn", &["m_iHealth"])];
parser.run_to_end_with_properties(&filter, properties, None, |ctx, _events| {
    println!("Tick {}: {} entities", ctx.tick(), ctx.entities().len());
})?;
# Ok::<(), boon::Error>(())
```

Listed paths include their descendants. Unlisted classes retain all fields.
`None` skips event collection. Filters apply only to this replay pass.

## License

MIT &mdash; see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
