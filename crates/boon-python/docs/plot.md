# Plotting

```python
from boon import Demo, plot

demo = Demo("match.dem", preload=False)
version = "6795"  # Replace with the image version for your demo.
fig, ax = plot.summary(demo, version=version, title="League Final", builds=True)
fig.savefig("summary.png", dpi=160, bbox_inches="tight")
```

Use `boon.plot` for match summaries, team totals, and map positions.
Each function returns a matplotlib `(fig, ax)` pair.
Use `ax` to add details. Use `fig.savefig(...)` to save a plot.

<a id="install"></a>

## Install Boon plotting and game images

```bash
uv add 'boon-deadlock[plot]'
boon get GAME_VERSION --images map --images heroes --images items
```

Replace `GAME_VERSION` with a version from `boon versions`.
Import plots with `from boon import plot`. An import of `boon` alone does
not load matplotlib.

Set `version=` to select game data for the demo. Without it, image plots use
the newest installed catalog. If none is installed, Boon downloads the latest
release. Boon does not select this version from the replay header.
Boon downloads and checks missing image groups. See {doc}`data`.

<a id="match-summary"></a>

## Draw match summaries with Boon plot.summary

The default title is `Match <match_id>`.
The subtitle shows the winner and match duration. An asterisk marks the winning
team. Both headings are centered. Missing results show `Unknown`.

The table uses each player's last recorded summary values.
Healing and barrier absorption have separate columns.
Values of 1,000 or more use `k`, such as `18.3k` or `500k`.
Missing values show `—`.

Set `builds=True` to show items between the player name and stat columns.
Builds use two rows of five, or two rows of six for 11–12 items.
Larger builds add rows. Item borders show the item type:

- Orange: weapon.
- Green: vitality.
- Purple: spirit.
- Gray: unknown.

Item types come from `m_eItemSlotType` in `abilities.json`.
Builds use recorded purchases, upgrades, and sales through game over.
Incomplete demos can omit earlier purchases. Boon excludes hero upgrades.
Missing item images show the item name. Boon matches players by Steam ID.

The default `columns` are:

| String | Label |
| --- | --- |
| `net_worth` | Souls |
| `kills` | Kills |
| `deaths` | Deaths |
| `assists` | Assists |
| `player_damage` | Player Damage |
| `boss_damage` | Objective Damage |
| `player_healing` | Healing |
| `barrier_absorption` | Barrier Absorption |

Select columns for a smaller table:

```python
fig, ax = plot.summary(
    demo, columns=["net_worth", "player_damage", "player_healing"], portraits=False,
)
```

With `portraits=False` and `builds=False`, no catalogs or images are needed.
A missing hero image does not remove the player row.

<a id="team-totals"></a>

## Draw team totals with Boon plot.timeline

```python
fig, ax = plot.timeline(demo)  # Souls.
fig.savefig("souls.png", dpi=160)

fig, ax = plot.timeline(demo, "player_damage")
fig.savefig("damage.png", dpi=160)
```

`metric` and summary `columns` accept the strings above.
They also accept other numeric columns in `demo.summary()["snapshots"]`.
The fields `steam_id`, `hero_id`, `snapshot_time_s`, and `lane` are excluded.

The horizontal axis is "Time Since Start", in minutes.
The vertical axis uses `k` for thousands. Lines join recorded summary points.
Boon does not add time samples. Missing values or players leave gaps.

Summary and timeline values come from the post-match message.
These values do not come from combat events. A missing summary produces an error.
Timelines need no catalogs or images.

<a id="positions"></a>

## Draw map positions with Boon plot.positions

```python
fig, ax = plot.positions(
    demo, tick=10000, version=version,
    objectives=True, troopers=True, ziplines=True,
)
fig.savefig("positions.png", dpi=160, bbox_inches="tight")
```

This reads player positions at one exact tick.
Team colors, hero images, and health bars use recorded player data.
Dead players use dim crosses. Health bars show `health / max_health`.

| Parameter | Default | Effect |
| --- | --- | --- |
| `labels` | `False` | Show player names. |
| `portraits` | `True` | Show hero images. Set to `False` for plain markers. |
| `objectives` | `False` | Show objective health, patron phases, and destroyed objectives. |
| `troopers` | `False` | Show small dots at trooper positions. |
| `ziplines` | `False` | Show team-colored routes, lane badges, and control boundaries. |
| `facing` | `False` | Show player viewing directions. |

For objectives, Boon uses the last recorded state at or before the tick.
A new entity at the same site replaces the old record.
An unchanged record does not prove that the objective still exists.
Unknown health shows `Unknown`.

Players, troopers, and ziplines use the same exact-tick query.
Boon does not load the full trooper table. It caches zipline history after the
first query. See {doc}`ziplines` for the recorded tables and their limits.

Zipline routes use team colors. Gray means neutral or unknown ownership.
Lane-name badges use yellow for York, blue for Broadway, and green for Greenwich.
A short tick marks different known owners at a connection's midpoint.
This is a display boundary, not a recorded capture position.
Boon omits connections to missing nodes.
Ziplines draw below troopers, objectives, and players.

<a id="plot-tables"></a>

## Draw Polars tables with Boon plot.frame

Use `frame()` for tables that you have already read:

```python
frames = demo.snapshots(["player_ticks", "troopers", "ziplines"], ticks=10000)
fig, ax = plot.frame(
    frames["player_ticks"],
    troopers=frames["troopers"],
    ziplines=frames["ziplines"],
    roster=demo.players,
    version=version,
)
```

Player rows require `x` and `y`. Optional fields are `hero_id`, `team_num`,
`player_name`, `yaw`, `health`, `max_health`, and `is_alive`.
The `roster` table adds names and teams by `steam_id`.

In `frame()`, `objectives=`, `troopers=`, and `ziplines=` accept DataFrames.
All rows must refer to one tick. Supply one objective row per entity.
If needed, keep the objective's original change tick in `last_update_tick`.
Set its `tick` to the frame tick.

Zipline rows require `node_handle`, `connections`, `x`, and `y`.
Use `lane` for the badge and `team_num` for the route color.
An unknown lane has a gray badge. Unknown ownership uses a gray route.
Boon omits missing or invalid positions. It also omits positions outside map bounds.

<a id="map-layers-and-coordinates"></a>

## Select Boon minimap layers and world bounds

`layer` accepts these strings or `plot.MapLayer` members:

| String | Enum |
| --- | --- |
| `surface` | `MapLayer.SURFACE` |
| `tunnels` | `MapLayer.TUNNELS` |
| `rat_tunnels` | `MapLayer.RAT_TUNNELS` |

```python
fig, ax = plot.positions(
    demo, tick=10000, version=version, layer=plot.MapLayer.TUNNELS,
)
```

A layer selects the map image. It does not select a floor.
Use `z_range=(low, high)` to keep rows where `low <= z < high`.
This filter applies to players and all map additions.
Without it, all altitudes are included.

Set `map_name="dl_midtown"` in `positions()` or `frame()` to check the selected
images against Midtown. Replay header names can refer to the loading map.
They do not select the plot background.

Coordinates use Source world units, with +Y up.
Image bounds come from the selected boon-data version.
Boon does not calculate bounds from player positions.
If the image data has no bounds, pass verified values with
`bounds=plot.MapBounds(x_min, y_min, x_max, y_max)`.
Missing bounds produce an error.

`radar()` draws a bare minimap. All plot functions accept `ax=`:

```python
fig, ax = plot.radar(version=version)
ax.scatter([0], [0], color="white", marker="x")
```

Close figures with `matplotlib.pyplot.close(fig)` after use.
