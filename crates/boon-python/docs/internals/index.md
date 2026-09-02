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
events, datasets, and generated tables. Read the pbdems2 guide when you change
the shared parser. Read the next section for Deadlock token tables.

```{toctree}
:maxdepth: 1

name-tables
```
