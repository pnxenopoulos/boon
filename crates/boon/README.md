<div align="center">

# boon-deadlock

[![crates.io](https://img.shields.io/crates/v/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![crates.io Downloads](https://img.shields.io/crates/d/boon-deadlock.svg)](https://crates.io/crates/boon-deadlock)
[![docs.rs](https://docs.rs/boon-deadlock/badge.svg)](https://docs.rs/boon-deadlock)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

A fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo file (`.dem`) parser for Rust.

Part of the [Boon](https://github.com/pnxenopoulos/boon) project.

## Features

- Memory-mapped, zero-copy parsing for maximum throughput
- Match metadata (map, players, duration, build number)
- Full entity state at any tick via snapshot seeking
- Game event extraction with protobuf decoding
- Filtered tick streaming for efficient per-entity-class analysis
- Ability and modifier name lookups

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
boon-deadlock = "0.1"
```

Requires Rust 1.88+ (edition 2024).

## Quick Start

```rust,no_run
use std::path::Path;
use boon::Parser;

let parser = Parser::from_file(Path::new("match.dem")).unwrap();
parser.verify().unwrap();

// File header
let header = parser.file_header().unwrap();
println!("Map: {:?}", header.map_name);
println!("Build: {:?}", header.build_num);

// File info (playback time, players)
let info = parser.file_info().unwrap();
println!("Duration: {:?}s", info.playback_time);
```

## API Overview

### `Parser`

The main entry point. Owns the demo file data (memory-mapped or in-memory).

| Method | Description |
|--------|-------------|
| `Parser::from_file(path)` | Open and memory-map a `.dem` file |
| `Parser::from_bytes(bytes)` | Parse from an in-memory buffer |
| `verify()` | Check magic bytes |
| `file_header()` | Decode `CDemoFileHeader` (map, server, build) |
| `file_info()` | Decode `CDemoFileInfo` (duration, players) |
| `messages()` | List all command headers in the file |
| `events(max_tick)` | Extract game events (legacy + Citadel user messages) |
| `parse_to_tick(tick)` | Parse to a specific tick, returning full entity state |
| `run_to_end(callback)` | Stream every tick with a callback |
| `run_to_end_filtered(filter, callback)` | Stream with an entity class filter (much faster) |

### `Context`

Returned by `parse_init`, `parse_to_tick`, and passed to tick callbacks. Contains:

- `entities` &mdash; all active entities (`EntityContainer`)
- `serializers` &mdash; field definitions per class
- `class_info` &mdash; class ID to name mappings
- `string_tables` &mdash; key-value tables (models, baselines, etc.)
- `tick` &mdash; current tick
- `tick_interval` &mdash; seconds per tick

### `Entity`

A single networked entity with class name and decoded field values.

```rust,ignore
// Look up fields by dotted path
let health = entity.get_by_name("m_iHealth", serializer);
let x = entity.get_by_name(
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
    serializer,
);
```

### Helper Functions

- `ability_name(id)` &mdash; resolve an ability hash to its name
- `modifier_name(id)` &mdash; resolve a modifier hash to its name
- `decode_event_payload(msg_type, data)` &mdash; decode a game event's protobuf payload

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
| [`entities`](examples/entities.rs) | `parse_to_tick()`, entity iteration, `get_by_name()`, `ability_name()` |
| [`player_ticks`](examples/player_ticks.rs) | `run_to_end_filtered()`, `resolve_field_key()`, per-tick streaming |

## Performance

For best throughput when you only need specific entity types, use `run_to_end_filtered` with a class filter. This skips field decoding for entities outside the filter set.

```rust,ignore
use std::collections::HashSet;

let filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();
parser.run_to_end_filtered(&filter, |ctx| {
    for (_, entity) in ctx.entities.iter() {
        // Only CCitadelPlayerPawn entities are tracked
    }
}).unwrap();
```

## License

MIT &mdash; see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
