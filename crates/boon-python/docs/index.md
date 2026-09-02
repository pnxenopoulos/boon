# Boon

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo parser. The Rust core has native Python bindings. Boon reads Source 2 `.dem` files and returns [Polars](https://pola.rs) DataFrames.

## Why Boon?

Deadlock demos contain player positions, kills, damage, item builds, objective state, and other match data. The Source 2 demo format is complex and undocumented. Boon handles the format so that you can analyze structured data.

- ⚡ **Fast.** The core parser is written in Rust. Parsing a full match takes seconds, not minutes.
- 📊 **Structured output.** Each dataset is a Polars DataFrame. You can filter, group, join, and display the data.
- 🎯 **Parse only what you need.** Boon loads each dataset on demand. Use `load()` to parse multiple datasets in one pass.
- 🗂️ **Comprehensive.** Player state, combat, economy, objectives, map props, Sinner's Sacrifice, derived stats, buffs/debuffs, urn and Rift tracking, and street brawl scoring.
- 💻 **CLI included.** The Python package installs a `boon` command for quick inspection without writing code.

## Get started

Install Boon with `uv add boon-deadlock` or `pip install boon-deadlock`. Then read {doc}`getting-started`.
If you have a problem, check {doc}`known-issues`. Report other problems on [GitHub](https://github.com/pnxenopoulos/boon/issues) or in [Discord](https://discord.gg/WmjZHxWrCD).

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
faq
known-issues
reference/index
internals/index
roadmap
changelog
```
