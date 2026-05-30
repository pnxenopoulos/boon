# 🚗 Roadmap

This page tracks features and improvements being considered for Boon. It is
not a commitment — items here are ideas, in no particular order, and may
change. Contributions and feedback are welcome on [GitHub](https://github.com/pnxenopoulos/boon/issues)
or [Discord](https://discord.gg/WmjZHxWrCD).

## ID → name mappings

Several columns are exposed as raw numeric IDs that would be more ergonomic
with a `*_names()` lookup, mirroring the shape of `team_names()` or
`patron_phase_names()`:

- **`lane_names()`** — for the `start_lane` (players) and `lane` (objectives)
  columns. Values come from the `CMsgLaneColor` proto enum: `1=yellow`,
  `3=green`, `4=blue`, `6=purple`, `0=none`.
- **`hitgroup_names()`** — for the `hitgroup_id` column in `damage`. Source 2
  uses a standard set (head, chest, stomach, limbs, …); confirming the exact
  set Deadlock uses needs a pass over the game data.
- More as new opaque enums are surfaced.

## Visualization

Helpers to turn parsed DataFrames into visuals without writing ad-hoc
plotting code each time:

- **Static plots** for common views: net-worth-over-time, kill timelines,
  damage-dealt-vs-taken matrices, lane control.
- **Position heatmaps** per hero or per team, computed from `player_ticks`.
- **Animated GIFs** of player movement over a match (or a window), rendered
  on a stylized map.
- A small style API so plots/GIFs use consistent Deadlock colours (team,
  hero, lane) by default.

## Analysis helpers

Composable, derived stats rather than only raw rows:

- A `match_summary` helper returning per-player KDA, GPM, XPM, last-hits,
  hero damage, objective damage, etc. in a single frame.
- **Combat-encounter detection** — grouping kills and damage into fights and
  tagging participants, location, and outcome.
- **Win-probability over time** — an interface for plugging in a model and
  exposing a `win_prob` column on `world_ticks`.

## Performance and ergonomics

- **Streaming / incremental parsing** for partial or in-progress demos.
- **Bulk-match utilities** for analysing many demos at once (parallel parse).

## Have an idea?

Open a [GitHub issue](https://github.com/pnxenopoulos/boon/issues) or chime
in on the [Discord](https://discord.gg/WmjZHxWrCD).
