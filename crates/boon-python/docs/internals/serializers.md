# Serializers

Serializers (historically called "send tables" in Source 1) define the **field
schema** for each entity class. They describe what fields an entity has, their types,
and how they are encoded in the bit stream.

## Source Data

Serializer definitions arrive in the `DEM_SendTables` command, which contains a
`CsvcMsgFlattenedSerializer` protobuf message. This message has three main parts:

1. **Symbols** — a string table of all type names, field names, and encoder names
2. **Fields** — a flat list of field definitions, each referencing symbols by index
3. **Serializers** — a list of serializer definitions, each referencing a subset of
   fields by index

## Serializer Structure

A serializer is a named collection of fields:

```
Serializer "CCitadelPlayerController"
  fields:
    [0] m_iTeamNum:       U64
    [1] m_iszPlayerName:  String
    [2] m_steamID:        U64
    [3] m_PlayerDataGlobal: (nested serializer)
      [0] m_nHeroID:     U64
      [1] m_bAlive:      Bool
      [2] m_iPlayerKills: I64
      ...
```

Fields can be **nested** — a field may reference another serializer, creating a
hierarchy. This is how dotted paths like `m_PlayerDataGlobal.m_iPlayerKills` work.

## Field Properties

Each field carries:

| Property | Description |
|----------|-------------|
| `var_type` | Type string (e.g., `"bool"`, `"float32"`, `"Vector"`, `"CNetworkUtlVectorBase< int32 >"`) |
| `var_name` | Field name (e.g., `"m_iHealth"`) |
| `bit_count` | Number of bits for quantized encoding |
| `low_value` / `high_value` | Range for quantized floats |
| `encode_flags` | Flags affecting encoding behavior |
| `var_encoder` | Special encoder name (`"coord"`, `"normal"`, `"qangle_precise"`, etc.) |
| `field_serializer` | Nested serializer for compound types |

## Decoders

Each field is assigned a **decoder** based on its type, encoder, and bit count. The
main decoder types are:

| Decoder | Used For |
|---------|----------|
| `Bool` | Boolean fields |
| `I64` / `U64` | Integer fields (varint encoded) |
| `F32NoScale` | Raw 32-bit IEEE floats |
| `F32Quantized` | Floats stored as N-bit values mapped to a [low, high] range |
| `F32Coord` | Source coordinate encoding (14-bit integer + 5-bit fraction) |
| `F32SimulationTime` | Tick count converted to seconds via `tick * tick_interval` |
| `String` | Variable-length byte strings |
| `Vector2/3/4` | Component-wise float vectors |
| `QAngle*` | Euler angles with various precision encodings |

## Special Field Types

- **Fixed arrays**: `Type[N]` — decoded as N sequential values
- **Dynamic arrays**: `CNetworkUtlVectorBase<T>` — prefixed with a varint length,
  then that many elements
- **Pointers**: `Type*` — a single boolean (present/absent)

## Field Resolution

Serializers support resolving **dotted field paths** to packed 64-bit keys:

```python
# Internally, this walks the serializer hierarchy:
key = serializer.resolve_field_key("m_PlayerDataGlobal.m_iPlayerKills")
# key is a packed u64 encoding the path [3, 2] (field indices at each level)
```

The packed key is what gets stored in the entity's field map and used for O(1) lookup.
