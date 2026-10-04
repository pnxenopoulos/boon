# Player states

Use `Demo.player_states()` to read named states for each player at each tick.
The names come from `modifiers.json` in the selected boon-data version.

```python
from boon import Demo

demo = Demo("108575009.dem", preload=False)
states = demo.player_states(data_version="6712", ticks=[187554])
print(states.select("tick", "steam_id", "hero_id", "states"))
```

Use `boon versions` to list catalog versions and `boon get VERSION` to install
one. The method downloads a missing version and does integrity checks. The catalog must
include `modifier_states`; rebuild older releases that lack this mapping.
If the mapping is missing, Boon reports an error with the install commands.
Choose a catalog from the same game build as the replay. Boon does not select
that version from the demo header.

## Select ticks and players

Without `ticks`, the query reads every recorded tick. An integer selects one tick; a list
selects several. Boon sorts the selection and removes duplicate ticks. Negative
or missing ticks cause an error. An empty tick or Steam ID list returns no rows.

```python
mcginnis = demo.player_states(
    data_version="6712",
    ticks=187554,
    steam_ids=[76561198037652386],  # This player in 108575009.dem.
)

# All ticks for this player.
all_ticks = demo.player_states(
    data_version="6712", steam_ids=[76561198037652386]
)
```

The query reads only player controllers and hero pawns. It does not load
`player_ticks` or calculate stats. It uses the hero pawn during death and
respawn, even when the controller selects a spectator pawn.

## Columns

| Column | Type | Contents |
|---|---|---|
| `tick` | Int32 | Exact demo tick. |
| `steam_id` | UInt64 | Steam account ID, or null if unavailable. |
| `hero_id` | Int64 | Hero ID recorded on the controller at this tick. |
| `states` | List(String) | Names from the recorded predicted-state mask. |
| `enabled_states` | List(String) | Names from the enabled-state mask. |
| `disabled_states` | List(String) | Names from the disabled-state mask. |
| `unknown_states` | List(UInt32) | Unknown bit indices in the predicted mask. |
| `unknown_enabled_states` | List(UInt32) | Unknown bit indices in the enabled mask. |
| `unknown_disabled_states` | List(UInt32) | Unknown bit indices in the disabled mask. |

Names have no `MODIFIER_STATE_` prefix and are sorted by name. Unknown indices
are sorted by number. Each mask is read independently. Boon does not combine masks
or decide which mask has priority.

The predicted-state mask is recorded game data. Boon does not predict these
states. It includes combat and movement states that can be missing from the
enabled-state mask.

An empty list means no matching entries. Read the corresponding `unknown_*`
list before you conclude that no bits are set. Null in the two columns means that
the pawn, mask, or a necessary mask word is unavailable. A player without a Steam
ID retains a row with a null ID. A Steam ID filter selects
only rows with a matching recorded ID. An unknown Steam ID returns no rows.
Stat and imbue queries instead report an error for an missing requested player.

Use `steam_id` to join these rows to `demo.players`.
Use `tick` and `steam_id` to join state rows to stat results.
Do not join null Steam IDs.

## Available state names

State names depend on the selected catalog. They are output strings, not a
fixed Python enum or a selector argument. Examples include `SPRINTING`,
`IN_COMBAT`, `INFINITE_CLIP`, and `SILENCED`. Read the full list for your version:

```python
import json
from boon import data

catalog = json.loads(data.catalog_path("modifiers", "6712").read_text(encoding="utf-8"))
state_names = sorted(
    name.removeprefix("MODIFIER_STATE_")
    for name in catalog["modifier_states"].values()
)
print(state_names)
```

This lists defined states, including states never set in your replay.
The `unknown_*` columns retain bits missing from that catalog.

## Use state lists

```python
import polars as pl

in_combat = states.filter(pl.col("states").list.contains("IN_COMBAT"))
unlimited_ammo = states.filter(pl.col("states").list.contains("INFINITE_CLIP"))
```

State flags report conditions. They do not supply bonus values, effect sources,
or stat equations. For example, `SLOWED` does not give a slow percentage.
These flags do not change the calculated move-speed or sprint-speed stats.
`INFINITE_CLIP` does not change the reported magazine capacity.

A defined flag can stay unset in a replay. Do not use `IN_COMBAT_BULLET_HIT`
alone to count hits.

## Rust

```rust
use boon::{
    Parser,
    player_states::{PlayerStateQuery, StateCatalog},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = Parser::from_file(Path::new("108575009.dem"))?;
    let catalog = StateCatalog::load("6712")?;
    let query = PlayerStateQuery::default()
        .ticks([187554])
        .steam_ids([76561198037652386]);
    let rows = parser.player_states(&query, &catalog)?;
    println!("{rows:?}");
    Ok(())
}
```

Use `visit_player_states` to process rows without collecting the full result.
The decoder resolves array keys once and reuses decoded names for repeated
masks. Rust rows use `Option` for unavailable masks and retain unknown indices.
