# 🚗 Roadmap

This page lists possible features and improvements for Boon. Items are not
commitments. The list has no fixed order, and the items can change. Send
feedback on [GitHub](https://github.com/pnxenopoulos/boon/issues) or
[Discord](https://discord.gg/WmjZHxWrCD).

## ID → name mappings

Some columns contain only numeric IDs. Add `*_names()` lookups when a name makes
the data easier to use. Use the same format as `team_names()` and
`patron_phase_names()`.

- **`lane_names()`** — Map player `start_lane` and objective `lane` values.
  Values come from the `CMsgLaneColor` protobuf enum: `1=yellow`,
  `3=green`, `4=blue`, `6=purple`, `0=none`.
- Add more lookups when Boon exposes an enum without names.

## Visualization

Add helpers that create visuals from parsed DataFrames:

- **Static plots** for common views: net worth over time, kill timelines,
  damage-dealt-vs-taken matrices, lane control.
- **Position heatmaps** for each hero or team, calculated from `player_ticks`.
- **Animated GIFs** of player movement for a match or time window. Show the
  movement on a stylized map.
- A small style API that applies the same Deadlock colors to teams, heroes, and
  lanes.

## Analysis helpers

Add analysis functions that return calculated statistics:

- A `match_summary` function that returns KDA, GPM, XPM, last hits, hero damage,
  and objective damage for each player.
- **Combat encounter detection** — Group kills and damage into fights. Add the
  participants, location, and result.
- **Win probability over time** — Add an interface for a model. Put its
  `win_prob` values in `world_ticks`.

## Performance and ergonomics

- **Streaming / incremental parsing** for partial or in-progress demos.
- **Bulk-match utilities** for analysing many demos at once (parallel parse).

## Have an idea?

Open a [GitHub issue](https://github.com/pnxenopoulos/boon/issues) or send a
message on [Discord](https://discord.gg/WmjZHxWrCD).
