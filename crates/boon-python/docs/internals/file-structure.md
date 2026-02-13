# File Structure

A Deadlock demo file uses the **PBDEMS2** (Protobuf Demo Source 2) format. The file
is a sequential stream of commands, each carrying a protobuf-encoded payload.

## Header

The first 16 bytes of the file are fixed:

| Offset | Size | Contents |
|--------|------|----------|
| 0 | 8 bytes | Magic: `PBDEMS2\0` |
| 8 | 4 bytes | Offset to `DEM_FileInfo` |
| 12 | 4 bytes | Offset to spawn groups |

The magic bytes are checked during construction to reject non-demo files. The
`DEM_FileInfo` offset is used by the parser to jump directly to the file info command
at the end of the file, avoiding a full sequential scan. The spawn groups offset is
currently unused.

## Command Stream

After the 16-byte header, the rest of the file is a sequence of **commands**. Each
command has a small header followed by a protobuf body:

```
[Raw CMD]   varint32 — command type + compression flag
[Tick]      varint32 — game tick when this command was recorded
[Body Size] varint32 — size of the body in bytes
[Body]      bytes    — protobuf message, optionally Snappy-compressed
```

The raw command value encodes two pieces of information:

- **Command type**: the lower bits, after masking out the compression flag
- **Compression flag**: `DEM_IsCompressed` — when set, the body is
  Snappy-compressed and must be decompressed before protobuf decoding

## Command Types

| Command | Description |
|---------|-------------|
| `DEM_FileHeader` | File metadata (build number, map name, game directory) |
| `DEM_SendTables` | Serializer definitions (field schemas for all entity classes) |
| `DEM_ClassInfo` | Maps numeric class IDs to network class names |
| `DEM_SignonPacket` | Initialization packets (string tables, server info, initial entities) |
| `DEM_Packet` | Per-tick game state updates |
| `DEM_FullPacket` | Complete state snapshot (string tables + all entities) |
| `DEM_SyncTick` | Marks the end of initialization — game ticks begin after this |
| `DEM_Stop` | End of demo |
| `DEM_FileInfo` | Footer metadata (playback ticks, playback time, game info) |

## Two Phases

Parsing happens in two phases, split by `DEM_SyncTick`:

1. **Initialization** (`DEM_FileHeader` through `DEM_SyncTick`): the file header,
   send tables, class info, and signon packets are processed to build up the
   serializer registry, class mapping, string tables, and initial entity state.

2. **Playback** (after `DEM_SyncTick` through `DEM_Stop`): `DEM_Packet` commands
   carry per-tick entity deltas. Periodic `DEM_FullPacket` commands provide complete
   state snapshots that allow the parser to jump ahead without replaying every tick.
