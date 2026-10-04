# 🔬 Demo File Internals

Deadlock demos use the **PBDEMS2** Source 2 container and entity format.
[pbdems2](https://crates.io/crates/pbdems2) implements these shared functions.
Its docs.rs guide contains this information:

- [File structure and outer commands](https://docs.rs/pbdems2/latest/pbdems2/guide/file_structure/index.html)
- [Inner packet-message framing](https://docs.rs/pbdems2/latest/pbdems2/guide/packet_messages/index.html)
- [Serializers and field paths](https://docs.rs/pbdems2/latest/pbdems2/guide/serializers/index.html)
- [String tables and instance baselines](https://docs.rs/pbdems2/latest/pbdems2/guide/string_tables/index.html)
- [Entities, class information, and handles](https://docs.rs/pbdems2/latest/pbdems2/guide/entities/index.html)
- [Adapters, playback, seeking, and segmentation](https://docs.rs/pbdems2/latest/pbdems2/guide/playback/index.html)

Boon supplies the Deadlock protobuf adapter, entity and property selections,
events, datasets, and catalog lookups. Read the pbdems2 guide when you change
the shared parser. Read {doc}`name-tables` for Deadlock token tables.

```{toctree}
:maxdepth: 1

name-tables
```

## Code structure

- `crates/boon/src/hero_stats/`: Read catalog definitions and resolve stat inputs.
  `inputs/tests.rs` contains the resolver tests.
- `crates/boon/src/rulesets/`: Keep equations different of catalog balance values.
  `percentage.rs` supplies the shared remaining-factor equation.
- `crates/boon-python/src/`: Build Polars columns and cache datasets.
  `DatasetCache` uses the dataset enum for direct lookups.
- `crates/boon-python/python/boon/`: Supply public queries and typed result tables.
  `_selection.py` rejects incorrect ticks, catalog versions, and Steam IDs before native calls.

Keep observed replay values in different fields from calculated values. Read balance values
and effect bindings from boon-data. Keep assumptions in result diagnostics and
{doc}`../known-issues`.
