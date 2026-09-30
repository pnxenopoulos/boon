# Player states

Use `Demo.player_states()` to read named states for each player at each tick.
The names come from `modifiers.json` in the selected boon-data version.

```python
from boon import Demo

demo = Demo("106996573.dem", preload=False)
states = demo.player_states(data_version="6694", ticks=[50707])
print(states.select("tick", "steam_id", "hero_id", "states"))
```

Use `boon versions` to list catalog versions and `boon get VERSION` to install
one. The method downloads and verifies a missing version. The catalog must
include `modifier_states`; older releases need a new build with this mapping.
If the mapping is absent, Boon reports an error with the install commands.
Choose a catalog from the same game build as the replay. Boon does not select
that version from the demo header.

## Select ticks and players

Omit `ticks` to read every recorded tick. An integer selects one tick; a list
selects several. Boon sorts the selection and removes duplicate ticks. Negative
or absent ticks cause an error. An empty tick or Steam ID list returns no rows.

```python
venator = demo.player_states(
    data_version="6694",
    ticks=50707,
    steam_ids=[76561197999389679],  # This player in 106996573.dem.
)

# All ticks for this player.
all_ticks = demo.player_states(
    data_version="6694", steam_ids=[76561197999389679]
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

Names omit the `MODIFIER_STATE_` prefix and are sorted by name. Unknown indices
are sorted by number. Each mask is read separately. Boon does not combine masks
or decide which mask has priority.

The predicted-state mask is recorded game data. Boon does not predict these
states. It includes combat and movement states that can be absent from the
enabled-state mask.

An empty list means no matching entries. Check the corresponding `unknown_*`
list before you conclude that no bits are set. Null in both columns means that
the pawn, mask, or a required mask word is unavailable. A player without a Steam
ID retains a row with a null ID. A Steam ID filter selects
only rows with a matching recorded ID. An unknown Steam ID returns no rows.
Stat and imbue queries instead report an error for an absent requested player.

Use `steam_id` to join these rows to `demo.players`.
Use `tick` and `steam_id` to join state rows to stat results.
Do not join null Steam IDs.

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

A defined flag can stay unset in a replay. In `106996573.dem`, no hero has
`IN_COMBAT_BULLET_HIT` set, although the demo contains bullet damage. Do not use
that flag alone to count hits.

## Rust

```rust
use boon::{
    Parser,
    player_states::{PlayerStateQuery, StateCatalog},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = Parser::from_file(Path::new("106996573.dem"))?;
    let catalog = StateCatalog::load("6694")?;
    let query = PlayerStateQuery::default()
        .ticks([50707])
        .steam_ids([76561197999389679]);
    let rows = parser.player_states(&query, &catalog)?;
    println!("{rows:?}");
    Ok(())
}
```

Use `visit_player_states` to process rows without collecting the full result.
The decoder resolves array keys once and reuses decoded names for repeated
masks. Rust rows use `Option` for unavailable masks and retain unknown indices.
