<div align="center">

# boon-deadlock

[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![docs.rs](https://docs.rs/boon-deadlock/badge.svg)](https://docs.rs/boon-deadlock)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

A fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo file (`.dem`) parser for Rust.

Part of the [Boon](https://github.com/pnxenopoulos/boon) project.

**Demo compatibility:** Use Boon **0.10.0 or earlier** for demos recorded before
the **City Never Sleeps** update (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos recorded with that update or later.

## Features

- Memory-mapped input and selected entity decoding
- Match metadata (map, players, duration, build number)
- Full entity state at any tick with snapshot seeking
- Stable entity identities and typed update/PVS-leave/delete lifecycle events
- Game event extraction with protobuf decoding
- Filtered tick streaming for efficient analysis by entity class
- Versioned hero, ability/item, modifier, and breakable names from boon-data
- Hero and ability stat queries, recorded states, and item imbues

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
boon-deadlock = "0.11"
```

Use Rust 1.88 or later (edition 2024).

## Quick Start

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

## API Overview

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
| `parse_to_tick(tick)` | Parse to a specific tick, returning full entity state |
| `run_to_end(callback)` | Stream every tick with a callback |
| `run_to_end_filtered(filter, callback)` | Stream with an entity class filter |
| `calculate_hero_stats(query, catalog, rules)` | Calculate selected hero stats |
| `calculate_ability_stats(query, catalog, rules)` | Calculate ability bonus percentages |
| `player_states(query, catalog)` | Read named player states and unknown bits |
| `imbues(query, catalog)` | Read item selections and catalog effects |

### `Context`

Returned by `parse_init`, `parse_to_tick`, and passed to tick callbacks. Contains:

- `entities()` &mdash; all active entities (`EntityContainer`)
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

### Helper Functions

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

See the [hero stat guide](https://boon.readthedocs.io/en/latest/hero-stats.html#rust),
[ability stat guide](https://boon.readthedocs.io/en/latest/ability-stats.html#rust), and
[player state guide](https://boon.readthedocs.io/en/latest/player-states.html#rust) for complete Rust examples.
The guides list enum members, units, equations, and calculation limits.

## Examples

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
```

| Example | What it shows |
|---------|---------------|
| [`info`](examples/info.rs) | `file_header()`, `file_info()`, match metadata and player list |
| [`events`](examples/events.rs) | `events()`, event filtering, `decode_event_payload()` |
| [`entities`](examples/entities.rs) | `parse_to_tick()`, entity iteration, `get_by_name()`, `CatalogNames::ability_name()` |
| [`player_ticks`](examples/player_ticks.rs) | `run_to_end_filtered()`, `resolve_field_key()`, streaming at each tick |

## Performance

Use `run_to_end_filtered` with a class filter to select specific entity
types. Boon does not decode fields for entities outside the filter.

```rust,no_run
use std::collections::HashSet;
use std::path::Path;
use boon::Parser;

let parser = Parser::from_file(Path::new("match.dem"))?;
let filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();
parser.run_to_end_filtered(&filter, |ctx| {
    for (_, entity) in ctx.entities().iter() {
        println!("{}", entity.class_name);
    }
})?;
# Ok::<(), boon::Error>(())
```

## License

MIT &mdash; see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
