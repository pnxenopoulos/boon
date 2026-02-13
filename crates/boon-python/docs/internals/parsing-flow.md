# Parsing Flow

This page describes the end-to-end flow of parsing a demo file, from opening the
file to extracting per-tick game state.

## Overview

```
Open file
  → Verify magic bytes (PBDEMS2)
  → Initialization phase (up to DEM_SyncTick)
  → Playback phase (DEM_SyncTick to DEM_Stop)
```

## Initialization Phase

The initialization phase processes commands sequentially until `DEM_SyncTick`:

### 1. File Header (`DEM_FileHeader`)

Extracts metadata:
- Build number
- Map name
- Game directory

### 2. Send Tables (`DEM_SendTables`)

The `CsvcMsgFlattenedSerializer` message is decoded to build the full
[serializer](serializers.md) registry. This defines the field schema for every
entity class in the demo. All symbols, fields, and serializer definitions are
resolved into an in-memory registry keyed by class name.

### 3. Class Info (`DEM_ClassInfo`)

Maps integer class IDs to network class names. Also computes the number of bits
needed to encode a class ID in the entity bit stream. See [Class Info](class-info.md).

### 4. Signon Packets (`DEM_SignonPacket`)

These packets contain the initial service messages:

- **`SVC_ServerInfo`** — provides the tick interval (e.g., `1/30` seconds)
- **`SVC_CreateStringTable`** — creates each [string table](string-tables.md),
  including `"instancebaseline"` which holds default entity field values
- **`SVC_PacketEntities`** — creates the initial set of entities

After processing signon packets, the instance baselines are extracted from the
string tables and cached.

### 5. Sync Tick (`DEM_SyncTick`)

Marks the end of initialization. The parser now has a complete `Context`:

- `serializers` — all entity field schemas
- `class_info` — class ID to name mapping
- `string_tables` — all string tables including instance baselines
- `entities` — initial entity state
- `tick_interval` — seconds per tick
- `tick` — current game tick

## Playback Phase

After `DEM_SyncTick`, the parser processes tick-by-tick updates:

### Per-Tick Packets (`DEM_Packet`)

Each packet's inner message stream is decoded:

1. **String table updates** — merged into existing tables; instance baselines
   re-extracted if changed
2. **Entity updates** (`SVC_PacketEntities`) — processed as a bit stream of
   creates, updates, and deletes (see [Entities](entities.md))

### Full Packets (`DEM_FullPacket`)

Periodic snapshots that contain:

- Complete string table state (replaces current tables)
- Full entity state via `SVC_PacketEntities`

Full packets enable the parser to skip ahead efficiently. When parsing to a
specific tick, the parser can jump to the nearest full packet before the target
and replay from there, rather than processing every tick from the start.

### Stop (`DEM_Stop`)

Marks the end of the demo.

## Parsing Modes

The parser supports three modes:

### `parse_to_tick(target_tick)`

Parses up to the specified tick and returns the full `Context` with entity state at
that point. Uses full packets to skip ahead when possible.

### `run_to_end(callback)`

Streams the entire demo, calling the provided callback with the `Context` at every
tick. The callback receives a reference to the same context, which is mutated
in-place between ticks.

### `run_to_end_filtered(class_filter, callback)`

Same as `run_to_end`, but only fully tracks entities whose class names are in the
filter set. All other entities are still parsed (to keep the bit reader aligned) but
their field values are skipped rather than decoded and stored. This is significantly
faster when you only need a subset of entity types.

## Bit-Level I/O

All low-level reading uses a **bit reader** that supports:

- Fixed-width reads (1–64 bits)
- Unsigned varints (LEB128)
- Signed varints (zigzag encoding)
- Valve's `ubitvar` (variable-length with 6-bit base)
- IEEE 754 floats
- Specialized decoders: coordinates, normals, angles

The bit reader uses unaligned 64-bit reads for performance, with fallback paths
near the end of the buffer.
