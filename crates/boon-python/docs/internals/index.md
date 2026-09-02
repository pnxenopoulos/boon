# 🔬 Demo File Internals

Deadlock demos use Valve's game-neutral **PBDEMS2** Source 2 container and
entity wire format. Those shared mechanisms are implemented by
[pbdems2](https://crates.io/crates/pbdems2) and documented in its docs.rs guide:

- [File structure and outer commands](https://docs.rs/pbdems2/latest/pbdems2/guide/file_structure/index.html)
- [Inner packet-message framing](https://docs.rs/pbdems2/latest/pbdems2/guide/packet_messages/index.html)
- [Serializers and field paths](https://docs.rs/pbdems2/latest/pbdems2/guide/serializers/index.html)
- [String tables and instance baselines](https://docs.rs/pbdems2/latest/pbdems2/guide/string_tables/index.html)
- [Entities, class information, and handles](https://docs.rs/pbdems2/latest/pbdems2/guide/entities/index.html)
- [Adapters, playback, seeking, and segmentation](https://docs.rs/pbdems2/latest/pbdems2/guide/playback/index.html)

Boon supplies the Deadlock protobuf adapter, entity/property selections,
events, datasets, and generated token, display-name, and stat tables. See the pbdems2
guide when extending the shared parser, and the chapter below for the
Deadlock-specific token tables.

```{toctree}
:maxdepth: 1

name-tables
```
