# 🔬 Demo File Internals

This section documents how Deadlock demo files (`.dem`) are structured internally.
Deadlock uses Valve's **Source 2** demo format, a binary streaming format built on
Protocol Buffers. Understanding these internals is useful if you want to extend the
parser or work with raw entity data.

```{toctree}
:maxdepth: 1

file-structure
messages
string-tables
class-info
serializers
entities
parsing-flow
```
