# Replay input fields

These fields expose facts already recorded by the Deadlock demo. They do not
infer an enemy's position from the server's world state while the enemy is
hidden.

```python
from boon import Demo

demo = Demo("match.dem", preload=False)
players = demo.snapshots("player_ticks", seconds=1.0)
enemy_icons = demo.enemy_minimap_ticks(every=64)
mid_boss_hp = demo.objectives.filter(
    demo.objectives["objective_type"] == "mid_boss"
)
```

## Current upgrades and boon count

`player_ticks.upgrade_ids` is the complete current
`CCitadelPlayerController.m_PlayerDataGlobal.m_vecUpgrades` vector at that
tick. An empty list is a recorded empty vector; null means the vector or an
entry was unavailable or could not be decoded. The vector contains raw upgrade
tokens, which may include hero abilities. Use the matching game-data catalog
to identify purchased items. Purchase events are not needed to reconstruct the
current token vector.

`player_ticks.boon_count` is read only from a directly recorded controller
field (`m_iBoonCount` or `m_nBoonCount`, with or without the `m_PlayerDataGlobal`
prefix). It is null when no such field exists in the replay schema or its value
is not an integer. This column never substitutes level or a calculated boon
schedule for a direct observation.

## Enemy minimap observations

`enemy_minimap_ticks()` reads `CCitadelTeam.m_vecFOWEntities`, classifying
recorded class-code 1 rows for enemy heroes. `observer_team` identifies the
team's shared minimap channel; this channel applies to each player on that
team. `visible_on_map` is the recorded flag. `map_x` and `map_y` are the native
0–255 minimap coordinates and are null while the icon is hidden or the
coordinates are invalid. These values are not world coordinates.

`target_entity_index` and `target_entity_serial` are filled only when the
recorded FOW index resolves uniquely to a current player pawn. `target_hero_id`
then comes from that pawn. `target_steam_id` comes from the one active player
controller whose current serial-bearing pawn handle points to the same pawn.
It is null if the controller or Steam ID is unavailable, the forward handles
disagree, or multiple controllers claim that pawn. The raw FOW index is always
retained so unresolved rows remain inspectable. `last_seen_tick`, `last_seen_x`, and `last_seen_y`
carry the last *shown* coordinate for that same resolved pawn identity on that
team's channel. These columns stay null when the pawn identity cannot be
resolved or has not yet been shown. The raw `tick_hidden_raw` is also exposed;
its units and display meaning are not interpreted here. Last-seen memory is
updated while scanning every tick, even when the output is sampled.

The game adapter decodes `CEntityIndex` as a signed integer. The reader uses
that index directly. Missing pawns and stale controller handles leave the
identity unknown. The visibility flag describes the team icon, not direct
line of sight.

Omit `ticks` and `every` to return all recorded ticks. Use one of these options
to select exact ticks or a tick interval:

```python
icons = demo.enemy_minimap_ticks(ticks=[187554])
icons = demo.enemy_minimap_ticks(every=64)
hidden = icons.filter(~icons["visible_on_map"].fill_null(False))
```

Only enemy hero icons are included. These rows do not include other map icons
or draw a map. Missing source entries do not produce rows.

Rust callers can use `boon::replay_inputs::UpgradeReader` and `EnemyMinimap`.
Call `EnemyMinimap::update(ctx, emit)` at every completed tick. Set `emit` to
false between selected ticks to preserve last-seen history.

## Mid-boss health

The existing `objectives` dataset includes rows with
`objective_type == "mid_boss"`, `health`, `max_health`, and `tick`. It emits the
first observed nonzero-max-health state of each entity lifetime and subsequent
health/max-health changes. `entity_id` plus `entity_serial` distinguish a new
mid-boss from a later boss that reuses the same entity slot. Consumers can
detect an observed 50% crossing from those rows.
The separate `mid_boss` dataset records lifecycle events.
