# String Tables

String tables are a Source 2 mechanism for efficiently storing and synchronizing
collections of keyed data between server and client.

## Structure

Each string table is a named list of entries. An entry has an optional string key
and optional binary user data:

```
StringTable
  name: "instancebaseline"
  entries:
    [0] string: "5",   user_data: <binary field data>
    [1] string: "12",  user_data: <binary field data>
    ...
```

Tables are configured with metadata such as whether user data has a fixed size and
whether varint bit counts are used.

## Lifecycle

1. **Create** — sent via `SVC_CreateStringTable` during initialization. Contains the
   full table definition and initial entries. Entry data may be Snappy-compressed.

2. **Update** — sent via `SVC_UpdateStringTable` during gameplay. Contains only
   changed entries with delta-encoded indices.

3. **Full replace** — sent inside `DEM_FullPacket` commands. Provides a complete
   snapshot of all tables, replacing the current state.

## Entry Encoding

Entries use a history-based encoding for string keys:

- A 32-entry circular history buffer tracks recent strings
- When encoding a new string, the encoder can reference a previous history entry
  and copy a prefix from it, then append new characters
- History index: 5 bits, copy length: 5 bits
- Maximum string size: 32 bytes (per entry key)

This saves bandwidth when many similar keys are added in sequence.

## Instance Baselines

The most important string table for entity parsing is `"instancebaseline"`. It
stores the **default field values** for each entity class.

- **Key**: the string representation of a class ID (e.g., `"5"`, `"23"`)
- **User data**: a bit stream of encoded field values using the class's serializer

When a new entity is created, the parser:

1. Looks up the class ID in the instance baselines table
2. Decodes the baseline bit stream to populate initial field values
3. Applies the creation delta on top of the baseline

Baselines are cached per class ID so the baseline bit stream only needs to be
decoded once per class. Subsequent entities of the same class clone the cached
baseline and apply their own deltas.

Baselines can change during the demo (via string table updates), so the parser
re-extracts them whenever the string table is updated.
