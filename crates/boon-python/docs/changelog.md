# Changelog

## Unreleased

- Add bounded exact replay checkpoints for repeated Rust stat batches.
- Preserve adapter configuration across checkpoints.
- Use pbdems2 0.3.6.

## 0.14.0

- Share modifier lifetime rules across stat queries, modifier events, and `boon-dev`. Keep raw modifier data.
- Preserve ordered modifier changes. Exclude table snapshots from events.
- Remove the six-player filter from `demo.active_modifiers`.
- Validate full entity handles. Use pbdems2 0.3.5.
- Correct Urn carrier events. Confirm returns from channel completion and delivery-point closure.
- Remove corruption-specific stat detection.
- Update protobufs to Deadlock 6759 (`boon-proto 0.4.11094174+6759`).

## 0.13.0

- Add Rust catalog loading from JSON bytes and single-pass stat batches.
- Add the target stat to hero-stat contributions.

## 0.12.0

- Fix world coordinates for current Deadlock demos.
- Use pbdems2 0.3.4.

## 0.11.0

- Support City Never Sleeps demos (September 29, 2026) and later. Use Boon 0.10.0 or earlier for older demos.
- Add hero and ability stat queries with `current` and `baseline` modes, boon-data inputs, and source explanations.
- Add `player_states()` and `imbues()` for recorded states and item selections.
- Add snapshot `steam_id` and `ammo_fraction`. With `data_version`, add `ammo`, `max_ammo`, and `unlimited_ammo`.
- Add item-purchase `steam_id`, `upgraded_from_ability_ids`, and `get_item_purchases(data_version=...)`.
- Add summary `barrier_absorption`. Keep recorded healing in its own column.
- Use Steam IDs in stat, state, imbue, and summary results. Remove player slots from public results.
- Correct modifier lifetimes and barrier snapshots across seeks and full passes.
- Use catalog weapon data, shop thresholds, scaling defaults, effect bindings, and recorded bonus types.
- Correct spirit scope and multiplier order, movement bonus addition, and light/heavy melee labels.
- Remove old-format fallbacks and `stat_modifier_*` snapshot columns. Share dataset caches and test inputs.
- Use pbdems2 0.3.3. Update protobufs to Deadlock 6746 (`boon-proto 0.4.11080740+6746`). The direct protobuf API has changed fields and types, including map fields.
- Include MIT license files in packages. Do not include Python caches. Use `--locked` for all release wheel builds.

Hero stat strings:

- Weapon: `clip_size`, `bullet_velocity`, `weapon_damage`, `fire_rate`, `reload_time`, `falloff_start`, `falloff_end`, `light_melee_damage`, `heavy_melee_damage`, `melee_distance`.
- Movement: `move_speed`, `sprint_speed`, `slide_distance`, `gravity_scale`, `stamina`, `stamina_cooldown`, `dash_speed`, `dash_duration`, `air_dash_speed`, `air_dash_duration`.
- Resistance and evasion: `bullet_resist`, `spirit_resist`, `melee_resist`, `debuff_resist`, `bullet_evasion`.
- Lifesteal: `bullet_lifesteal`, `spirit_lifesteal`, `melee_lifesteal`.

Ability stat strings: `cooldown_reduction`, `item_cooldown_reduction`, `duration_bonus`, `range_bonus`, `radius_bonus`.

See [Examples](examples.md) for queries and [Known Issues](known-issues.md) for calculation limits.

## 0.10.0

- **API change:** Remove `demo.healing`, `load("healing")`, `demo.barriers()`, and `boon.barriers`.
- Keep recorded healing, regeneration, soul sources, and damage histories in `summary()`.
- Read hero, ability, item, modifier, and breakable names from boon-data. Download missing catalogs.
- Add name-function `version=` selection and `breakable_names()`.
- **Rust API change:** Replace static name functions with `CatalogNames::load(version)` and its lookup methods.
- Add `boon get`, `boon versions`, `boon remove`, and `boon.data` for verified local catalogs.
- Preload kills, damage, and abilities in `Demo(path)`. Use `preload=False` to defer dataset loads.
- Correct hero IDs after hero changes, including chat, damage, and item purchases.
- Add damage absorption, victim shield values, and server ticks. Remove `is_secondary_stat`.
- Read permanent spirit-power modifier ID 159 in client 6698.
- Reduce snapshot allocations. Add Python/Rust benchmarks, Ruff, ty, and CI checks.
- Use pbdems2 0.3.2 and `boon-proto 0.3.11038876+6701`.

Past entries describe APIs at each release. Some APIs below are no longer available.

## 0.9.0

- Remove Rust `resistances`, `stat_catalog`, and `stats` modules and generated gameplay tables.
- **API change:** Remove Python `stat_ticks()` and `stat_effects()`.
- Add `EffectiveModifierState` for effective modifiers alongside replicated rows.
- Replace calculated resistance snapshot columns with recorded `stat_modifier_*` contributions and availability flags.
- Add `demo.barriers()` for inferred barrier grants, absorption, expiry, and hits.
- Add `demo.healing` with events inferred from negative damage messages.
- Add `pregame_seconds`, `game_start_tick`, `tick_to_match_seconds()`, and `tick_to_match_clock()`.
- Add `damage.victim_entity_id` and correct finite modifier expiry against recorded game time.
- Reduce snapshot allocations and parser passes. Group compatible dataset loads and event selectors.

## 0.8.0

- Add Python `ability_display_names()` and Rust ability-display-name and breakable-name lookups.
- Add damage source IDs, types, flags, and light/heavy/other melee classification.
- Add `sinners_sacrifice` and `breakables` datasets with stable entity identity and exact hit records.
- Add `stat_ticks()` and `stat_effects()` for calculated stat values and contributions.
- Add barrier and calculated baseline resistance snapshot columns.
- Add modifier serials and duration/application changes. Keep partial updates and order simultaneous effects by serial.
- Correct regulation time against the HUD clock. Cache the post-match message and summary frames.
- Reduce teamfight and snapshot parser passes. Process only changed controllers for change-only datasets.
- Use pbdems2 0.3.0. Export `EntityId`, `EntityChange`, and `EntityChangeKind`.
- Add `ModifierState`, stat blocks, masks, operations, and modifier-type decoding across game builds.
- Update protobufs to `0.3.10933105+6684`; remove `Match.not_scored` and add server/rank metadata.
- Add manual release workflows for each package. Create tags after successful uploads.
- Add concise technical writing guidance. See [Contributors](contributors.md) for community work on this release.

## 0.7.0

- Add Python `rift` and the Rust CLI `rift` command, with capture/expiry, winner, lane, and position.
- Read the Rift winner from game-rule scoring. Unobserved expiry behavior stays unverified.
- Restore `banned_heroes`; a missing message gives an empty frame.
- Add player `rank`, with zero for missing rank metadata.
- Filter event messages before payload allocation. Release the Python interpreter during parsing and snapshots.
- Add `Entity::get_vector3` for whole-coordinate fields.
- Update protobufs and name tables to Deadlock 6668, source revision 10879761.

## 0.6.2

- Add modifier `changed` events for stack updates. Removed rows retain the final stack count.
- Permit demos without a match ID. Use `match_id=None` and game mode zero when unavailable.

## 0.6.1

- Correct bare `char` fields to unsigned varints. Keep `char[N]` string buffers unchanged.
- Prevent field-type errors from shifting later entity data. Add a regression test.

## 0.6.0

- Add the Python `boon` CLI and Python 3.11-3.14 support.
- Rename the Rust CLI to `boon-dev`; stop publication of its release binaries.
- Read the player roster at game over, before controller removal.
- Add parallel player/world/trooper snapshots and selected-tick `snapshots()` queries.
- Add spatial and temporal `teamfights()` detection.
- Return parse errors for corrupt field paths, indices, entity updates, and string-table sizes.
- Add missing Source 2 field decoders and correct wide unaligned bit reads.
- Add indexed entity storage. `EntityContainer::iter()` returns `(i32, &Entity)`.
- Add `full_packet_offsets()` and `decode_segment()` for keyframe segments.
- Update protobufs to `0.2.10822189+6635`. Rename post-match `urn_captures`/`UrnCapture` to `koth_captures`/`KothCapture`.

## 0.5.0

- Add `in_combat()`, `player_ticks.in_item_shop`, and change-only `ability_ticks`.
- Add the Rust CLI `ability-ticks` command.
- Add `EntityContainer::updated_indices()` and `clear_updated()`.
- Process changed entities for ability, objective, neutral, and urn records. Reduce serializer and field-map allocations.

## 0.4.0

- Add `boon.stats`, `kill_participation()`, and `time_dead()`, with matching `Demo` methods.
- Add Python/Rust hitgroup and life-state name lookups.
- Change patron phase 2 from `shields_down` to `transforming`; retain the numeric ID.

## 0.3.0

- Correct duplicate modifier and urn events. Process string-table deltas instead of all entries at each tick.
- Include nested VData modifier definitions in name tables.
- Read effective `max_health` from the controller, with a pawn fallback.
- Skip unused message payloads before allocation.
- Refresh names for build 6557, including Raven/Opera hero IDs.
- Update protobufs to `0.2.10717574+6557` and add post-match urn records.

## 0.2.0

- Add Python/Rust CLI post-match summaries, damage matrices, and soul-source breakdowns.
- Add regulation time properties and pause-aware tick conversions.
- Correct missing player snapshots caused by handle-index decoding.
- Read full world positions for players, objectives, troopers, neutrals, and urns.
- Add `patron_phase_names()` and correct lane-color labels.
- Version protobufs independently as `MAJOR.MINOR.SourceRevision+GameBuild`, starting with `0.2.10691905+6536`.

## 0.1.0

- Add Python `Demo`, metadata, player/world snapshots, combat, shop, ability, chat, objective, modifier, and Street Brawl datasets.
- Add `load()`, `available_datasets()`, game-over properties, name functions, and parser exceptions.
- Add the Rust CLI for raw messages, entities, send tables, string tables, and gameplay records.
- **API change:** Remove `hero` and `team` string columns, `teams`, `winning_team`, and unreliable bans from the pre-release API.
- Move `hero_names()` and `team_names()` from static methods to module functions.
- Combine `purchases` and `shop_events` as `item_purchases`. Remove duplicated ability/modifier name columns from event datasets.
