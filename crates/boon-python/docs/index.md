# Boon

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo parser. The Rust core has native Python bindings. Boon reads Source 2 `.dem` files and returns [Polars](https://pola.rs) DataFrames.

## Get started

Install with `uv add boon-deadlock` or `pip install boon-deadlock`.
Read {doc}`getting-started` for dataset queries and {doc}`examples` for complete examples.

Use {doc}`hero-stats` for hero values, {doc}`ability-stats` for ability bonuses and
imbues, and {doc}`player-states` for recorded states. These queries require a
boon-data version; see {doc}`data`. The guides list accepted strings and enum members.

Read {doc}`known-issues` for calculation limits.
Report errors on [GitHub](https://github.com/pnxenopoulos/boon/issues)
or [Discord](https://discord.gg/WmjZHxWrCD).

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
hero-stats
ability-stats
player-states
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
