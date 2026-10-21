# Boon

```python
from boon import Demo

demo = Demo("match.dem", preload=False)
demo.load("kills", "damage")
print(demo.players)
print(demo.kills.head())
demo.kills.write_parquet("kills.parquet")
```

Boon parses Valve Deadlock Source 2 `.dem` files with Rust and PyO3.
Python results are [Polars](https://pola.rs) DataFrames. Use Polars for filters,
groups, joins, and Apache Parquet output. Optional plots use matplotlib.

**Demo compatibility:** Use Boon **0.10.0 or earlier** for demos recorded before
the **City Never Sleeps** update (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos recorded with that update or later.

## Get started

Install with `uv add boon-deadlock` or `pip install boon-deadlock`.
Read {doc}`getting-started` for dataset queries and {doc}`examples` for complete examples.

Use {doc}`hero-stats` for hero values, {doc}`ability-stats` for ability bonuses and
imbues, and {doc}`player-states` for recorded states. Set a boon-data version for these queries; see {doc}`data`. The guides list accepted strings and enum members.

Use {doc}`ziplines` for recorded zipline routes and ownership.
Use {doc}`plot` for scoreboards, team totals, and player positions.
Read {doc}`known-issues` for calculation limits.
Report errors on [GitHub](https://github.com/pnxenopoulos/boon/issues)
or [Discord](https://discord.gg/WmjZHxWrCD).

Use the [llms.txt index](https://boon.readthedocs.io/llms.txt) for plain-text guides and typed API references.

## Useful links

- [Deadlock](https://www.playdeadlock.com/) — official home page
- [Steam store page](https://store.steampowered.com/app/1422450/Deadlock/)
- [Deadlock Wiki](https://deadlock.wiki/)
- [r/DeadlockTheGame](https://www.reddit.com/r/DeadlockTheGame/) — Reddit community

```{toctree}
:maxdepth: 2

getting-started
examples
api
cli
data
plot
hero-stats
ability-stats
player-states
ziplines
demo-checklist
benchmarks
faq
known-issues
reference/index
internals/index
roadmap
contributors
changelog
```
