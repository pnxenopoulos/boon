# Boon

Boon is a fast [Deadlock](https://store.steampowered.com/app/1422450/Deadlock/) demo / replay parser written in Rust with native Python bindings. It parses Source 2 demo files (`.dem`) and returns [Polars](https://pola.rs) DataFrames, giving you structured access to match data without dealing with the binary format yourself.

## Why Boon?

Deadlock demo files contain a wealth of match data — player positions, kills, damage, item builds, objective state, and more — but the Source 2 demo format is complex and undocumented. Boon handles the low-level parsing so you can focus on analysis.

- **Fast.** The core parser is written in Rust. Parsing a full match takes seconds, not minutes.
- **Structured output.** Every dataset is a Polars DataFrame, ready for filtering, grouping, joins, and visualization.
- **Parse only what you need.** Each dataset is loaded on demand. Request one property and Boon skips everything else. Batch multiple datasets with `load()` to share a single parse pass.
- **Comprehensive.** Player state, kills, damage, item purchases, ability upgrades, objectives, chat, lane troopers, neutral creeps, buffs/debuffs, urn tracking, and street brawl scoring.
- **CLI included.** A standalone command-line tool for quick inspection without writing any code.

## Get started

Install with `uv add boon-deadlock` or `pip install boon-deadlock`, then head to {doc}`getting-started` for a walkthrough. If something isn't working as expected, check {doc}`known-issues` first — then file a [GitHub issue](https://github.com/pnxenopoulos/boon/issues) or ask in the [Discord](https://discord.gg/tWCwmHDy2u).

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
changelog
```
