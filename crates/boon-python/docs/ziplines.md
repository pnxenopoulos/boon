# Ziplines

```python
from boon import Demo

demo = Demo("match.dem", preload=False)
nodes = demo.zipline_nodes
changes = demo.zipline_changes
# Both tables share one cached history pass.
```

Read zipline routes and ownership from the demo. No boon-data catalog is required.
Boon stores geometry once and adds a row only when it changes. Ownership and
visibility use a separate change table. Boon does not load these tables by default.

<a id="node-definitions"></a>

## Read Boon Demo.zipline_nodes geometry

`demo.zipline_nodes` contains initial definitions and later revisions:

| Column | Meaning |
| --- | --- |
| `tick` | First observation tick for this definition. |
| `node_handle` | Full UInt32 entity handle, including the serial. |
| `lane` | Raw lane ID. |
| `x`, `y`, `z` | World coordinates. |
| `connections` | List of full UInt32 target handles. Null means unknown or incomplete; `[]` means empty. |
| `cumulative_distance` | The game's cumulative path distance, in world units. |
| `capturable` | Whether the game marks the node as capturable. |

The key for a definition is `(tick, node_handle)`. A handle can have more than one
geometry revision. Use only revisions at or before the query tick. Do not use the
last definition for all ticks. Connections keep raw handles; resolve targets by
full handle at the same tick. An absent target remains unknown.

<a id="ownership-changes"></a>

## Read Boon Demo.zipline_changes ownership

`demo.zipline_changes` contains `tick`, `node_handle`, `change`, and `team_num`.
Accepted `change` strings are:

| Change | Meaning |
| --- | --- |
| `observed` | First observation, or observation after leaving visibility. |
| `owner_changed` | The recorded team changed. |
| `left_pvs` | The node left the visible set; it was not deleted. |
| `removed` | The replay explicitly deleted the node. |

`team_num` keeps the recorded integer. It is null when unknown, on `left_pvs`,
and on `removed`. No timeout is applied. Missing observations do not remove nodes.
Full handles keep different occupants of one entity slot separate. Rows describe
state after each tick. A keyframe can delete and recreate the same handle.
If the node stays active through that tick, Boon adds no removal or observation row.

<a id="snapshots"></a>

## Query Boon zipline snapshots at an exact tick

```python
state = demo.snapshots("ziplines", ticks=10000)
sampled = demo.snapshots("ziplines", seconds=5)
frames = demo.snapshots(["player_ticks", "ziplines"], ticks=[321, 10000])
```

Snapshots contain the node columns plus `team_num`. Here, `tick` is the query tick,
not the definition tick. Boon uses only observations through that tick and omits
nodes whose last transition is `left_pvs` or `removed`. Absence is not proof of
permanent removal. Unknown fields stay null.

All snapshot selectors work: `ticks`, `every`, `seconds`, `events`, `start_tick`,
and `end_tick`. Windows are inclusive. Tick strides start at the recording's first
available tick; a window does not reset the stride. Use `seconds=5` for a five-second
tick stride. This can include paused samples. The snapshot dataset is `ziplines`;
`load()` accepts the two compact table names, not `ziplines`.

Direct queries and sampled passes use the same serial history. The first query
reads that history once; later queries reuse it. Zipline queries do not change the
seek or parallel paths used by player, world, and trooper snapshots.

<a id="plotting"></a>

## Draw Boon ziplines with plot.positions

```python
from boon import plot

fig, ax = plot.positions(demo, tick=10000, version="6795", ziplines=True)
fig.savefig("ziplines.png", dpi=160, bbox_inches="tight")
```

Select an image version for your replay. Routes use team colors.
Lane-name badges use yellow for York, blue for Broadway, and green for Greenwich.
A small tick marks different known owners at a connection's midpoint.
This is a display boundary, not a recorded capture position. See {doc}`plot`.

<a id="limits"></a>

## Boon zipline evidence limits

Ownership describes zipline control. It is not a recorded troop-front position or
lane-push percentage. Combine it with troopers and objectives for lane analysis.
Cumulative distance is not normalized progress. Route branches can have different
origins; use the recorded connections when you construct paths.

<a id="rust"></a>

## Read ziplines with the Boon Rust API

```rust
use boon::{Parser, ziplines::ZiplineHistory};
use std::path::Path;

let parser = Parser::from_file(Path::new("match.dem"))?;
let history = ZiplineHistory::build(&parser)?;
history.visit_snapshots(&[10000], |tick, node, team| {
    println!("{tick}: {} {team:?}", node.node_handle);
});
```

`nodes()` and `changes()` return the compact history. `visit_snapshots()` sorts
and deduplicates its tick selection. It visits nodes in full-handle order.
