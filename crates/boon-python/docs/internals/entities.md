# Entities

Entities are the core data objects in a demo. Every game object — players, NPCs,
projectiles, world state — is represented as an entity with a class, an index, and a
set of fields.

## Entity Structure

An entity has:

| Property | Description |
|----------|-------------|
| `index` | Integer index (0–16383), used to identify the entity in the bit stream |
| `serial` | Serial number, incremented when an index is reused |
| `class_id` | Numeric class identifier |
| `class_name` | Network class name (e.g., `"CCitadelPlayerPawn"`) |
| `fields` | Hash map of packed field path keys to values |

## Field Values

Fields are stored as a `FieldValue` enum:

| Variant | Rust Type |
|---------|-----------|
| `Bool` | `bool` |
| `I32` | `i32` |
| `I64` | `i64` |
| `U32` | `u32` |
| `U64` | `u64` |
| `F32` | `f32` |
| `String` | `Vec<u8>` |
| `Vector2` | `[f32; 2]` |
| `Vector3` | `[f32; 3]` |
| `Vector4` | `[f32; 4]` |
| `QAngle` | `[f32; 3]` |

## Lifecycle

Entity state changes arrive in `SVC_PacketEntities` messages. The entity data is a
bit stream containing a sequence of updates, each prefixed with a **delta header**
(2 bits):

| Header | Meaning |
|--------|---------|
| `0b00` | **Update** — apply field deltas to an existing entity |
| `0b10` | **Create** — new entity with baseline + deltas |
| `0b01` | **Leave** — entity leaving (removed from tracking) |
| `0b11` | **Delete** — entity removed |

Entity indices are **delta-encoded**: each update adds `read_ubitvar() + 1` to the
previous index, so only the gaps between changed entities need to be stored.

## Creation Flow

When a new entity is created:

1. Read `class_id` (N bits, where N is from class info) and `serial` (17 bits)
2. Look up the class in [class info](class-info.md) to get the `network_name`
3. Look up the [serializer](serializers.md) by network name
4. Check the baseline cache — if this class was seen before, clone the cached entity
5. Otherwise, create a fresh entity and apply the **instance baseline** from the
   [string table](string-tables.md), then cache it
6. Apply the **creation delta** (field changes specific to this entity) on top

This baseline + delta approach means the wire format only needs to carry the
differences from the class default, saving significant bandwidth.

## Update Flow

Updates are simpler:

1. Look up the existing entity by index
2. Get its serializer by class name
3. Read and apply field path deltas

## Field Paths

Field updates use **field paths** — hierarchical indices that identify which field
changed. A field path is a sequence of up to 7 indices, one per level of nesting:

```
[3]       → 4th top-level field
[3, 2]    → 3rd sub-field of the 4th top-level field
[3, 2, 0] → 1st sub-sub-field
```

Field paths are **Huffman-encoded** in the bit stream. The Huffman tree uses 41
operation types optimized for common patterns:

- **PlusOne / PlusN**: increment the current level's index (common for sequential
  field updates)
- **Push**: descend to a child level
- **Pop**: ascend one or more levels
- **NonTopo**: jump to a non-sequential index
- **FieldPathEncodeFinish**: end of field path list

Each field path is packed into a 64-bit key for storage in the entity's field map.

## Filtered Parsing

For performance, the parser supports **filtered parsing** where only entities of
specified classes are fully tracked. Non-matching entities are still parsed (the bit
reader must advance correctly) but their field values are discarded. This is how
`get_player_ticks()` can efficiently process only `CCitadelPlayerPawn` and
`CCitadelPlayerController` entities while skipping everything else.

## Entity Handles

Some fields store **entity handles** — references to other entities. A handle is a
32-bit value where the lower 15 bits encode the entity index:

```
entity_index = handle & 0x7FFF
```

This is how `CCitadelPlayerController` references its `CCitadelPlayerPawn` via the
`m_hPawn` field. The parser resolves this by extracting the index and looking up the
corresponding entity.
