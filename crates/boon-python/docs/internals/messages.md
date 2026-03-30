# Messages

Commands like `DEM_Packet`, `DEM_SignonPacket`, and `DEM_FullPacket` contain an inner
stream of **service messages**. These are the actual game state updates.

## Packet Structure

Inside a `CDemoPacket.data` (or `CDemoFullPacket.packet.data`) payload, messages are
packed sequentially as:

```
[Msg Type]  ubitvar  — service message enum value
[Msg Size]  varint32 — message body size
[Msg Data]  bytes    — protobuf message
```

This inner stream is read with a **bit reader** since `ubitvar` is a
variable-length bit encoding.

## Service Message Types

| Message | Description |
|---------|-------------|
| `SVC_ServerInfo` | Server configuration, including `tick_interval` |
| `SVC_CreateStringTable` | Defines a new string table |
| `SVC_UpdateStringTable` | Delta update to an existing string table |
| `SVC_PacketEntities` | Entity create / update / delete deltas |

### `SVC_ServerInfo`

Provides the **tick interval** (typically `1/30 ≈ 0.0333` seconds), which is used to
convert simulation time values stored as tick counts back into seconds.

### `SVC_CreateStringTable` / `SVC_UpdateStringTable`

See [String Tables](string-tables.md).

### `SVC_PacketEntities`

The most important message type. It carries a bit stream of entity state changes
(creates, updates, and deletes). See [Entities](entities.md) for details on how this
is decoded.

## Compression

Both the outer command body and inner string table data can be Snappy-compressed.
The command-level compression is indicated by the `DEM_IsCompressed` flag. String
table data has its own compression flag in the `CsvcMsgCreateStringTable` message.
