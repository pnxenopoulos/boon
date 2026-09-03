# 📝 Changelog

## Unreleased

### boon-python

- Fixed finite modifiers that stayed active after their duration ended. Boon
  now uses the Source 2 simulation clock and the modifier duration to find the
  effective end tick. Explicit removals, aura exits, and slot reuse can end an
  effect earlier. Old demos without a compatible simulation clock keep the
  recorded transition behavior.

## 0.8.0

### boon-python

- New `ability_display_names()` maps exact current internal ability/item VData
  names—including `ability_*`, `upgrade_*`, and `citadel_ability_*`—to their
  English in-game names. The generated table joins current VData keys to
  Valve's localization catalogs; unlocalized
  hidden/test/retired entries are omitted instead of receiving guessed names.

- `demo.damage` now includes raw `ability_id`, `damage_type`, `citadel_type`,
  and `damage_flags`, plus `is_melee` and nullable `melee_type`. `is_melee`
  covers every melee-typed hit; Valve's explicit damage flags classify `light`
  and `heavy`, while abilities, NPC attacks, and ambiguous flags remain `other`.

- New opt-in `demo.sinners_sacrifice` dataset combines pbdems2 entity
  lifecycle/state with Damage messages to report Sinner's Sacrifice machine
  spawns, resets, and exact hits. Rows include stable index+serial identity,
  resolved attacker hero, incoming damage, tick-final health, team, and world
  position; unmatched health decreases are retained as unattributed hits.

- New opt-in `demo.breakables` dataset reports destruction of
  `CCitadel_BreakableProp` map props with tick, stable entity identity, raw
  subclass ID, resolved subclass name, team, and last-known world position.
  It uses pbdems2 0.3.0 lifecycle transitions
  to retain a PVS leave as a break candidate, cancel it if the same entity
  identity reactivates, and ignore full-packet delete/create replacements.
  This avoids a scan of every tracked prop for each tick. It also does not add
  a false health-zero or dead state that the server did not send.

- New `demo.stat_ticks(...)` selectively tracks native, persistent baseline,
  and tick-effective player stats for bullet/spirit resistance, spirit power,
  fire rate, weapon damage, cooldown reduction, status resistance, and both
  lifesteal types. It supports explicit ticks, ranges, strides, seconds, and
  event-aligned sampling; requested stats share one parallel modifier/entity
  pass and include a per-stat completeness flag.
- New `demo.stat_effects(...)` change-only details frame lists generated item
  and modifier contributions. Columns include layer, operation, resolved value,
  ability/modifier identity, runtime serial, caster/provider, stacks, duration,
  active state, and formula completeness.
- **Fixed:** `active_modifiers`, barrier snapshots, and effective stat tracking
  now consume one shared full-protobuf modifier state. Partial string-table
  updates preserve omitted fields, while removals, slot reuse, serial changes,
  aura range, and keyframe rebuilds are handled consistently.

- New `barrier` column in `demo.player_ticks` and sampled player snapshots. It
  reports the player's current barrier remaining from the
  `modifier_barrier_tracker` entry; older demos without that tracker return
  `0.0`.
- New `bullet_resist_baseline` and `spirit_resist_baseline` columns in
  `demo.player_ticks` and sampled player snapshots. They calculate the baseline
  percentage from hero
  VData progression, spirit scaling, and unconditional stats on the player's
  equipped items, using Deadlock's multiplicative resistance stacking. Temporary
  buffs, barriers, auras, and enemy resistance reductions are not included.
- **Faster:** `teamfights()` now batch-loads damage and kills in one filtered
  event pass. It gets player positions only at relevant damage ticks. It does
  not create all of `player_ticks`. On a 115 MB demo, this reduced
  end-to-end runtime from 20.2s to 12.1s (about 40%) with identical output.
- **Faster:** `in_combat()` and `time_dead()` request `player_ticks` and
  `world_ticks` together, collecting both in one parallel snapshot pass on a
  cold `Demo`. The paired input load fell from 4.23s to 2.49s on the same demo.
- **Faster:** mixed `Demo.load(...)` requests keep `player_ticks`,
  `world_ticks`, and `troopers` on their parallel keyframe-segmented path
  while event/entity datasets share a separate filtered pass. A mixed request
  no longer makes heavy snapshot datasets fall back to serial decoding.
- **Faster:** the change-only `ability_upgrades` and `stat_modifier_events`
  datasets process only player controllers updated on the current tick rather
  than rescanning every controller.
- **Fixed:** `stat_modifier_events` recognises the disjoint `EModifierValue`
  aliases observed in builds 10725 and 10854. The compatibility decoder is
  shared by `boon-python` and `boon-dev`, and no longer guesses an enum layout
  from one controller snapshot.
- **Fixed:** `regulation_ticks`, `regulation_seconds`, and
  `regulation_clock_time` now use the replicated HUD match clock at game over.
  They no longer include pregame recording time. Old demos without the clock
  field keep the previous pause-aware fallback.
- **Fixed:** `active_modifiers` now distinguishes concurrent raw modifier
  instances with a `serial` column and emits `"changed"` when a live
  modifier's duration or application timestamp changes, as well as when its
  stack count changes. A single ability may create multiple internal modifier
  instances (for example, an attached effect plus a kill-check window), so row
  count is not the stack count. Use `stacks` as the stack count.
  Source 2 may reuse a serial after removal, so a serial distinguishes
  concurrently live instances rather than serving as a globally unique
  lifecycle ID.
- **Fixed:** effective stat evaluation sorts modifiers that are active at the
  same time by runtime serial. Randomized map iteration can no longer
  change results when non-commutative resistance and flat-reduction effects
  overlap.
- **Faster:** `summary()` caches its decoded post-match message and four
  DataFrames after the first call. The scan, protobuf decode, and frame builds
  release the Python interpreter; repeated calls return shallow frame clones
  without reparsing (about 0.35ms on the benchmark demo).

### boon

- Updated the entity parser from `pbdems2` 0.2.2 to 0.3.0. Boon now
  re-exports stable `EntityId` values and typed `EntityChange` /
  `EntityChangeKind` lifecycle events, so callers can distinguish updates,
  PVS leaves, permanent deletes, and entity-slot reuse without rebuilding
  identity from an index alone.
- New generated `ability_display_name` / `all_ability_display_names` and
  `breakable_name` / `all_breakables` lookup APIs expose exact English
  ability/item labels and breakable-prop subclass names to Rust callers.
- New shared `decode_stat_modifier_value_type` compatibility API normalizes
  the observed build-10725 and build-10854 `EModifierValue` aliases, including
  signed bullet/spirit resistance entries.
- New fixed-size `StatBlock` / `StatMask` engine and explicit `StatId`,
  `StatOperation`, and native/baseline/effective layer semantics.
- The VData name-table generator now emits a compact catalog of unconditional
  item effects and live modifier effects, keyed primarily by ability and
  modifier ID with guarded cross-build fallbacks.
- New reusable `ModifierState` builds complete `ActiveModifiers` entries
  from partial protobuf deltas and emits typed applied/changed/removed events.

### boon-proto

- Synced protobuf definitions to game build **6684** (`SourceRevision`
  10933105); `boon-proto` is now `0.3.10933105+6684`. **Breaking:** upstream
  removed `CMsgClientToGCGetMatchHistoryResponse.Match.not_scored` from the
  match-history schema. The schema also adds server-mode metadata and
  low-priority-pool flags. Generated lookup tables refreshed from the same
  snapshot contain 794 abilities, 1,106 modifiers, 457 exact English
  ability/item display names, and 30 breakable prop subclasses.

### Documentation

- Maintained documentation, API text, and code comments now use ASD-STE100
  English where practical. Generated files and upstream protobuf text are
  excluded.

### Community contributions

- Thanks to [ghaif](https://github.com/ghaif) for identifying and investigating
  four changes included in 0.8.0. Their reports and initial implementations
  informed the final changes:

  - [PR #38](https://github.com/pnxenopoulos/boon/pull/38) — Sinner's
    Sacrifice machine events
  - [PR #37](https://github.com/pnxenopoulos/boon/pull/37) — breakable map prop
    events
  - [PR #35](https://github.com/pnxenopoulos/boon/pull/35) — melee damage
    detection
  - [PR #34](https://github.com/pnxenopoulos/boon/pull/34) — cross-build
    `EModifierValue` renumbering

  These pull requests were not merged directly. Their findings helped guide the
  final implementations.

### Release process

- Releases are manual and separate for each component. Publish `boon-proto`,
  `boon`, and `boon-python` independently. The workflow creates a tag only
  after the package-index upload succeeds.
- Merge the changes to `main` and wait for **CI Check**. Then run the
  **Release Boon** workflow for `boon-proto`, `boon`, and `boon-python`, in that
  order. Wait for each registry upload before you start the next release. The
  workflow checks the dependency order and creates the tags and GitHub Releases.
  Do not create release tags manually. See `CONTRIBUTING.md` for the full list.

## 0.7.0

### boon-python

- New `rift` dataset (`demo.rift`) — the Rift, a periodic king-of-the-hill objective added to the game (`Koth` in the game files), whose winner gets buffed troopers in that lane. One row per Rift with `rift_num`, `announce_tick`, `active_tick`, `capture_tick`, `expire_tick`, `winning_team`, `lane`, and `x`/`y`/`z`; exactly one of `capture_tick`/`expire_tick` is set per row. The winner is taken from the game rules' scoring team (`m_nKothScoringTeam`) rather than the Rift entity's own `m_iTeamNum` — that field tracks whoever last made capture progress and reports the wrong team. `lane` is derived from the cash-in location, since the Rift entities carry no lane field, and is `0` at any location that isn't a known Rift site. Demos from builds without the Rift return an empty frame rather than failing.

  Note: `expire_tick` is wired to the uncaptured path (the Rift clearing with no scoring team, per `m_timeKothGiveUp`) but is **untested against a real expiry** — every Rift in the demos on hand was captured.

- New `banned_heroes` property (`demo.banned_heroes`) — the heroes banned from a match, as `hero_id` (joins to `players.hero_id`) and resolved `hero_name`. Read from the one-shot `BannedHeroes` user message, which the server sends early in the demo and only when the match has bans. The message carries nothing but the hero IDs — no team, no banning player, and no pick/ban ordering — so it cannot be used to reconstruct a draft. An empty frame means no bans were recorded, which is indistinguishable from a build that never emits the message. Like `players` and `winning_team_num`, this is not a `load()` dataset: it shares the lightweight events-only scan with those properties, so it is free once any of them has been touched.

- New `rank` column in `demo.players`, sourced from the player controller's packed competitive display rank. It matches the post-match `initial_display_rank` when rank metadata is present; `0` means unranked, calibrating, or unavailable.

- **Faster:** event-backed datasets now tell the parser exactly which final
  message types they consume. Unrelated particle, sound, and combat messages
  are skipped before their payloads are copied or allocated; lightweight
  metadata properties scan only `GameOver` and `BannedHeroes`, and `summary`
  scans only `PostMatchDetails`. Kill and damage protobufs are decoded once
  instead of cloning their payload for a second decode. Bulk and lazy dataset
  parsing also release the Python interpreter so independent Python threads can
  continue while Rust parses a demo. General `snapshots(...)` queries now
  release the interpreter as well, including event-tick loading, sampling,
  seeking, and parallel snapshot decoding.

### boon

- New `boon-dev rift` command, listing the same one-row-per-Rift lifecycle, with `--summary` for captured/expired counts.
- New `Entity::get_vector3` accessor, for fields that carry a whole world coordinate in a single value (Source 2's `VectorWS`, e.g. `m_vKothCashInCurrentLocation`) as opposed to positions split across cell + offset halves.

### boon-proto

- Synced protobufs and the ability / modifier name tables to game build **6668** (`SourceRevision` 10879761). The updated schema adds ranked matchmaking and per-player rank progression metadata, hero XP rewards, player match outcomes, and current GC/client/user messages. The refreshed name tables contain 794 abilities and 921 modifiers.

## 0.6.2

### boon

- **Fixed:** `active_modifiers` under-reported modifier stack counts. The stack count was read once, when a modifier was first seen, and never updated — so a stacking modifier (e.g. the Spellslinger Headshots debuff accruing headshots) that climbed from 2 → 4 stacks kept reporting `2`, and its `removed` row echoed that stale value instead of the final total. In-place stack updates are now captured: a new `event` value **`"changed"`** is emitted on the tick a live modifier's `stacks` changes, and the `removed` row reports the final count. `applied`/`removed` semantics are otherwise unchanged. The `boon-dev active-modifiers` command gains the same `changed` events.

### boon-python

- **Changed:** opening a demo no longer requires a match ID. `Demo(path)` previously raised `DemoMessageError: could not resolve match ID from CCitadelGameRulesProxy` when a demo didn't carry one on its first tick (e.g. partial captures or sandbox / custom content), even though the rest of the demo was fully parseable. `Demo.match_id` is now `int | None` — recorded when present, left `None` otherwise — and `Demo.game_mode` (previously resolved in the same step, so it failed together with the match ID) now falls back to `0` when the game-rules entity is unavailable. Such demos open normally instead of failing at construction.

## 0.6.1

### boon

- **Fixed:** a bare `char` entity field — a scalar 8-bit integer such as the count `m_nAvailableHelperCount` on `CCitadel_Ability_Familiar_HelpingHands` — was decoded as a null-terminated string (a mapping introduced in 0.6.0). On any nonzero value the string decoder over-read past the field, desyncing the packet-entities bitstream so that a later entity index decoded as garbage and the parse aborted with `entity index … out of range`. Some demos failed outright (e.g. on `demo.regulation_clock_time` / `boon info`, which build `world_ticks`). A bare `char` now decodes as an unsigned varint, matching pre-0.6.0 behavior; `char[N]` string buffers are unchanged. Added a regression test.

## 0.6.0

### boon-python

- New `boon` command-line tool, bundled with the package. `pip install boon-deadlock` (or `uv add boon-deadlock`) now puts a `boon` executable on your PATH, built on [Typer](https://typer.tiangolo.com). It reads a demo through the same parser the library uses and prints Polars DataFrames, so you can inspect a match without writing any code:
  - `boon info match.dem` — match metadata (map, mode, build, duration, winner).
  - `boon players match.dem` — the roster, with hero names resolved.
  - `boon datasets` — list every inspectable dataset.
  - `boon show match.dem <dataset> [--limit N] [--tail] [--json]` — any dataset as a table (or JSON).
  - `boon summary match.dem [--part ...]` — the post-match summary.
  - `boon stats match.dem -m <metric>` — derived metrics (`kill-participation`, `time-dead`, `in-combat`).
  - `boon verify match.dem` — validate a file.

  (This is a separate tool from the standalone Rust `boon` binary on GitHub Releases, which keeps its own broader set of lower-level commands. Both are invoked as `boon`.)
- Now built and tested against Python 3.11–3.14: `requires-python` is bounded to `>=3.11,<3.15` and 3.14 is advertised in the package classifiers.
- **Fixed:** `demo.players` could return an empty (or, in principle, incomplete) DataFrame on recordings whose post-game tail outlives the player entities. The `CCitadelPlayerController` controllers are torn down a few seconds before the final recorded tick, and `players` snapshotted that final tick. It now snapshots the roster at the game-over tick — late enough that every field is populated (heroes locked, lanes assigned) but before the post-game teardown — and falls back to the final tick only when a demo has no game-over event. Rosters for unaffected demos are unchanged.
- **Faster:** the per-tick snapshot datasets — `player_ticks`, `world_ticks`, and `troopers` — now decode across `DEM_FullPacket` keyframe segments in parallel (one thread per segment). Each entity class is re-keyframed at every full packet, so the per-segment cold restarts stitch back into byte-for-byte the same frames as a serial pass (~3.3× faster on an 8-core machine for `player_ticks`). Requesting several of them together via `demo.load("player_ticks", "world_ticks", "troopers")` collects them all in a **single** parallel pass rather than one per dataset. Set `BOON_TICK_SEGMENTS=1` to force the serial path, or to a fixed integer to pin the segment count.
- New `demo.snapshots(...)` — sample per-tick state at *selected* ticks in the same single parallel pass, so you materialize only the ticks you need instead of the whole frame. Select ticks by `ticks=` (specific), `every=`/`seconds=` (stride), `start_tick`/`end_tick` (window), or `events="kills"` (aligned to an event dataset's ticks), and choose one or more of `player_ticks`/`world_ticks`/`troopers` via `datasets=`. Returns a DataFrame for a single dataset or a dict keyed by name for several. e.g. `demo.snapshots(every=64)` builds ~1 row/sec of ticks rather than all ~250k. A single-tick query (`demo.snapshots(ticks=T)`) seeks directly to that tick instead of decoding the whole demo (~11× faster).
- New `boon.stats.teamfights(demo)` (`Demo.teamfights()`) — detects teamfights from hero-vs-hero damage clustered in **space and time**: a fight is a localized period where the teams trade damage (not merely where kills happen), which separates concurrent skirmishes in different lanes that time-only clustering would merge. Tunable `gap_seconds` / `radius` / `min_players`; one row per fight with its tick/second window, duration, centre, participants, total hero damage, and kills (each attributed to exactly one fight).

### boon-proto

- Synced protobuf definitions to the latest Deadlock build (`6580` → `6635`); `boon-proto` is now `0.2.10822189+6635`. **Breaking (proto field rename):** `CMsgServerSignoutData_DetailedStats.urn_captures` (message `UrnCapture`) was renamed `koth_captures` (`KothCapture`) upstream — update any code that read that post-match field. The sync also adds an optional `process_id` to `CMsgServerToGcEnterMatchmaking` and a `k_EPingMarkerInfo_NoMarkerYesSoundMiniMap` ping-marker enum variant; the remaining changes re-add a redundant `[default = 0]` to a few `uint32` fields (a no-op, since `0` is already the implicit default).

### boon-cli → boon-dev

- The standalone Rust CLI is renamed **`boon-dev`** and is **no longer published** — there are no more release binaries (the `boon-cli-v*` tag track and its GitHub Release workflow are removed). It stays in the repository as a low-level debugging tool you can build from source (`cargo build --release -p boon-dev`). Everyday demo inspection now lives in the Python `boon` command bundled with `boon-deadlock` (see above); the two no longer collide on the name `boon`.

### boon

Hardening and a decode optimization ported from the sibling CS2 parser (both are Source 2):

- **Robustness:** the entity field-path walk now returns a parse error instead of panicking when a corrupted field path (a bitstream desync) addresses a nonexistent field, so one bad decode no longer takes down the whole parse.
- **Robustness:** hardened several more parse paths against malformed/truncated demos — bounds-checked the serializer field and symbol indices, the packet-entities entity index (previously an unchecked add that could also trigger a runaway slot-array allocation), and the string-table user-data sizes. Fuzzing 1000+ corrupt inputs now yields clean errors rather than panics/aborts.
- Added decoders for Source 2 field types the parser previously ignored — `Quaternion` / `CTransform` (multi-component float vectors, exposed as a new `FieldValue::FloatVector`), polymorphic pointer fields (e.g. game-mode rules), `CUtlBinaryBlock`, and the `CGlobalSymbol` / bare-`char` string types. These occur in Deadlock send tables, so decoding them keeps the bitstream aligned instead of risking a desync.
- **Fixed:** a wide unaligned bit read (≥57 bits at a non-byte-aligned offset, e.g. a mid-stream 64-bit field) silently dropped its high bits; the missing bits are now pulled from the following byte.
- Handle the legacy `has_pvs_vis_bits_deprecated` visibility bits in packet-entities updates (a no-op for modern demos, correct for old ones).
- **Faster:** entities are stored in a flat slot array indexed directly by entity index (`Vec<Option<Entity>>`) rather than a hash map, so `get` / `get_by_handle` are O(1) bounds-checked indexing. Class-filtered decode (the path the datasets use) is ~3.6% quicker on the ability set with byte-for-byte identical output, and iteration is now in deterministic index order. **API:** `EntityContainer::iter()` now yields `(i32, &Entity)` (was `(&i32, &Entity)`).
- New `Parser::full_packet_offsets()` and `Parser::decode_segment(start, end_tick, filter, cb)` primitives: decode one `DEM_FullPacket`-keyframe-delimited segment of the demo starting from that keyframe's snapshot. These are the building blocks for parallel per-tick decoding (used by the Python `player_ticks` above); `Parser` is `Sync`, so segments run on separate threads via `std::thread::scope`.

## 0.5.0

### boon-python

- New `boon.stats` metric `in_combat(demo)` (`Demo.in_combat()`) — a per-tick `in_combat` boolean keyed on `(tick, hero_id)`, derived from the pawn's `in_combat_end_time` window so it joins directly onto `player_ticks`.
- New `player_ticks` column: `in_item_shop` (zone bool alongside `in_regen_zone`).
- New `ability_ticks` dataset — change-only ability cooldown/charge state (`cooldown_start`/`cooldown_end`, `remaining_charges`, `charge_recharge_start`/`charge_recharge_end`) per `(hero_id, ability_id, slot)`, emitting a row only when an ability's state changes. Not loaded by default.
- **Faster:** the change-only entity datasets (`ability_ticks`, `objectives`, `neutrals`, `urn`) now process only the entities each tick actually changed instead of rescanning every active entity. An ability's cooldown/charge state can only change on a tick it was updated, so `demo.ability_ticks` is ~2.4× faster; output is byte-for-byte identical. The gain compounds when several of these datasets are loaded together.

### boon-cli

- New `ability-ticks` command — inspect ability cooldown/charge state changes (the CLI counterpart of the `ability_ticks` dataset), with `--filter`, `--summary`, tick-window flags, and `--json`.
- **Faster:** the `ability-ticks`, `objectives`, and `neutrals` commands use the same change-only scan — `ability-ticks` drops from ~1010 ms to ~426 ms on a 64 MB demo, with identical output.

### boon

- New `EntityContainer::updated_indices()` / `clear_updated()`: the parser now records which tracked entities were created or updated on each tick and exposes them on the per-tick `Context`, letting change-only consumers process just what changed instead of rescanning every active entity. The set is cleared after each `run_to_end*` callback.
- **Faster:** entity decode is ~5% quicker on a full-demo parse (back-to-back measurement), from two changes with identical output: (1) `SerializerContainer` now uses an `FxHashMap` rather than the default SipHash `HashMap`, since the per-update serializer lookup hashes the class-name string on the hot path; and (2) a new entity's field map is pre-sized to its class's field count, avoiding repeated rehashing as the baseline and create delta populate it.

## 0.4.0

### boon-python

- New `boon.stats` module — derived metrics computed from parsed demo data, keyed on `hero_id`. Each is also a thin `Demo` method. Initial metrics:
  - `kill_participation(demo)` (`Demo.kill_participation()`) — `(kills + assists) / team_kills` per player (a `[0, 1]` fraction), with an optional `start_tick` / `end_tick` window.
  - `time_dead(demo)` (`Demo.time_dead()`) — per-player `ticks_dead`, `seconds_dead`, and `pct_regulation_dead`, counting only non-paused ticks up to game-over so totals align with `regulation_ticks` / `regulation_seconds`.
- `hitgroup_names()` returning `dict[int, str]` for the `hitgroup_id` column on `damage`. Values are Source 2's `HitGroup_t` enum (`-1=invalid`, `0=generic`, `1=head`, … `19=head_no_resist`; the `HITGROUP_COUNT` sentinel is omitted).
- `lifestate_names()` returning `dict[int, str]` for the `lifestate` column on `player_ticks`. Values are Source 2's `LifeState_t` enum (`0=alive`, `1=dying`, `2=dead`, `3=respawnable`, `4=respawning`).
- **Changed:** `patron_phase_names()` renames patron phase `2` from `shields_down` to `transforming`. The raw `phase` integer is unchanged (still `2`); update any code comparing the resolved string against `"shields_down"`.

### boon-cli

- No functional changes; the version is bumped in step with the workspace.

### boon

- New `hitgroups` and `lifestates` lookup tables: `hitgroup_name(id)` / `all_hitgroups()` and `lifestate_name(id)` / `all_lifestates()`, mapping Source 2's `HitGroup_t` and `LifeState_t` enums to names and re-exported at the crate root. These back the Python `hitgroup_names()` / `lifestate_names()`.
- **Changed:** `patron_phase_name(2)` now returns `transforming` (was `shields_down`).

## 0.3.0

### boon-python

- **Faster:** all parsing is a few percent quicker — packet messages the parser doesn't consume (sounds, temp entities, etc.) are now skipped in place instead of copied out of the bitstream first.
- **Fixed & faster:** `active_modifiers` (and the idol events in `urn`) emitted duplicate `applied`/`removed` rows on nearly every tick. Source 2 keeps a modifier's original `ActiveModifiers` entry and adds a separate `entry_type=2` removal entry (the table never shrinks), so the old per-tick full-table rescan re-applied and re-removed it indefinitely — ~99% of rows were per-tick duplicates (e.g. 3.16M rows where ~41k were real). The scan now processes only the entries each string-table delta touches, reporting each modifier once applied and once removed. Much faster too: the ActiveModifiers decode dropped from ~7.3s to ~0.06s (full dataset load ~18s → ~5s). `urn` output is unchanged apart from dropping the duplicate idol events.
- **Fixed:** `modifier_names()` resolved only ~87 modifiers — the generic ones defined as top-level keys in `modifiers.vdata`; most gameplay modifiers are nested `subclass:` blocks the generator never scanned. It now unions every top-level key in `modifiers.vdata`, every nested `_my_subclass_name` there, and the `_my_subclass_name` of each modifier subclass in `abilities.vdata` (those whose `_class` starts with `modifier_`) — ~917 entries. This is the right field because a demo identifies a modifier by the `modifier_subclass` token on `CModifierTableEntry`, the `CUtlStringToken` (`MurmurHash2`) of its `_my_subclass_name`. Many modifiers live only in engine/C++ code and appear in no vdata file, so a share of `modifier_id` values stay `MODIFIER_NOT_FOUND` (name-list-bound, not a hashing limitation).
- `ability_names()`, `modifier_names()`, and `hero_names()` reflect the latest Deadlock build (`6557`). The hero table gains Raven (`hero_operative`, id 62) and a test hero (id 83), and renames id 82 (`hero_opera`) from "Raven" to "Opera" — Valve moved "Raven" onto the new slot. The ability table also tracked a few upstream removals.
- **Fixed:** the `max_health` column on `player_ticks` reported the pawn's `m_iMaxHealth`, a stale base value that current health exceeds on over half of all ticks (e.g. `817` vs a reported max of `780`). It now reads the controller's `m_PlayerDataGlobal.m_iHealthMax` — the live effective max (level growth, items, buffs) — falling back to the pawn value only before the controller is populated. The `health` column is unchanged.

### boon-cli

- **Fixed & faster:** the `active-modifiers` command had the same per-tick flicker — re-emitting an `applied`/`removed` pair for stale entries on nearly every tick (e.g. 435,795 events where ~5,763 were real). It now processes only the entries each string-table delta touches, reporting each modifier once applied and once removed, and runs much faster.

### boon-proto

- Synced protobuf definitions to the latest Deadlock build (`6536` → `6557`); `boon-proto` is now `0.2.10717574+6557`. The notable change is a new `CMsgServerSignoutData_DetailedStats.UrnCapture` message (per-urn post-match stats) added as a repeated `urn_captures` field. An earlier build also dropped a redundant `[default = 0]` from two `usermessages.proto` `uint32` fields — a no-op, since `0` is already the implicit default.

## 0.2.0

### boon-python

- `Demo.summary()` method returning the post-match summary as a dict with `snapshots`, `last_hits`, `objectives`, and `damage` keys. `snapshots` is a Polars DataFrame with one row per (snapshot, player) — a `snapshot_time_s` column plus per-player running totals (kills/deaths/assists, net worth, denies, level, lane, creep/neutral kills, player damage, and the per-source gold/orbs breakdown). `last_hits` is a Polars DataFrame of `hero_id` and `last_hits` (the final scoreboard last-hit total, only recorded per match). `objectives` is a Polars DataFrame of post-match objective records (destruction time and damage taken). `damage` is a Polars DataFrame of the damage matrix in long form — one row per (dealer, target, source, sample) with dealer/target as both `*_player_slot` and resolved `*_hero_id` (null for non-player slots, joinable to the other frames on `hero_id`), the per-interval (additive) `damage` per `stat_type` (a readable string), an `is_category` flag distinguishing coarse damage-type buckets from specific sources, and `sample_time_s`. Filter to `is_category == False` and `sum` for totals, or `cumsum` over `sample_time_s` for the running total.
- `Demo.regulation_ticks`, `Demo.regulation_seconds`, `Demo.regulation_clock_time` properties for the duration of actual gameplay (active, paused-time-excluded ticks up to the game-over event), distinct from the full-recording `total_ticks`/`total_seconds`/`total_clock_time`. Return `None` when no game-over event is present.
- `patron_phase_names()` module-level function returning `dict[int, str]` of patron phase ID to name (`0=normal`, `1=final`, `2=shields_down`) for the `phase` column on patron objective rows.
- **Fixed:** the `start_lane` / `lane` column docs previously claimed `1=left, 4=center, 6=right`, which is wrong — the values are `CMsgLaneColor` color IDs and `3=green` was also missing. Docs now correctly read `1=yellow, 3=green, 4=blue, 6=purple, 0=none`.
- **Fixed:** `player_ticks` dropped most players, often leaving only one hero. Player controllers link to their pawn through a `CHandle` whose entity index is the low 14 bits; the index was masked with `0x7FFF` (15 bits) instead of `0x3FFF`, so any handle with an odd serial resolved to the wrong entity and that player was silently skipped. The mask is now `0x3FFF`, and `player_ticks` again covers every player on the roster.
- **Fixed:** the `x` / `y` / `z` columns on `player_ticks`, `objectives`, `troopers`, `neutrals`, and `urn` previously emitted only the in-cell offset half of Source 2's split position storage — values bounded to `[0, 512)` that reset to `0` every time the entity crossed a cell boundary, producing a sawtooth instead of a trajectory. They now emit full world (Hammer-unit) positions, combining the networked `m_cellX/Y/Z` cell index with the `m_vecOrigin.m_vec{X,Y,Z}` offset via the new `boon::position::cell_to_world` helper. No display-side scaling is applied; downstream plotters supply their own map projection.

### boon-cli

- `summary` command for post-match details: a match overview, a timing section (total ticks/time and tick rate from the recording, the game-over tick, and the regulation/gameplay duration), each player's final snapshot (with the scoreboard last-hit total), and an objectives table mirroring the Python `summary()` `objectives` frame. `--json` dumps the full decoded metadata.
- Name resolution reflects the refreshed ability and modifier name tables from the latest Deadlock build.
- **Fixed:** the `neutrals` and `troopers` commands' `x` / `y` / `z` columns now report full world coordinates (Hammer units) instead of just the in-cell offset half of Source 2's split position storage — see the matching Python fix above.

### boon-proto

- Synced protobuf definitions to the latest Deadlock build and regenerated the ability and modifier name lookup tables (surfaced via `ability_names()` and `modifier_names()`).
- Versioned independently from the rest of the workspace to track the game build: `MAJOR.MINOR.<SourceRevision>+<GameBuild>` (e.g. `0.2.10691905+6536`). The monotonic `SourceRevision` is the patch, so each proto sync yields a higher, publishable version while staying compatible within the `0.2` line.

## 0.1.0

### boon-python (breaking changes from pre-release)

- **Breaking:** Removed `hero` and `team` string columns from `players` DataFrame. Use `hero_names()` and `team_names()` to resolve IDs to names.
- **Breaking:** Removed `teams` DataFrame property. Use `team_names()` module-level function instead.
- **Breaking:** Removed `winning_team` property. Use `winning_team_num` with `team_names()`.
- **Breaking:** Removed `banned_heroes` property. The `k_EUserMsg_BannedHeroes` event is no longer reliably present in GOTV demo recordings (see Known Limitations).
- **Breaking:** Moved `Demo.hero_names()` and `Demo.team_names()` from static methods to module-level functions `hero_names()` and `team_names()`. Import directly from `boon`.
- **Breaking:** `purchases` and `shop_events` datasets merged into `item_purchases`. Columns: `tick`, `hero_id`, `ability_id`, `change`.
- **Breaking:** `ability` column removed from `ability_upgrades`. Use `ability_names()` to resolve `ability_id`.
- **Breaking:** `modifier` and `ability` columns removed from `active_modifiers`. Use `modifier_names()` and `ability_names()` to resolve IDs.

### boon-python

- `Demo` class with metadata properties: `path`, `total_ticks`, `total_seconds`, `total_clock_time`, `build`, `map_name`, `match_id`, `tick_rate`, `game_mode`.
- `Demo.players` property returning a Polars DataFrame of player info.
- `Demo.player_ticks` property returning per-tick, per-player state (48 columns).
- `Demo.world_ticks` property returning per-tick world state.
- `Demo.kills` property for hero kill events with attacker, victim, and assisters.
- `Demo.damage` property for damage events with pre/post mitigation, hitgroups, and crit damage.
- `Demo.flex_slots` property for flex slot unlock events.
- `Demo.abilities` property for important ability usage events.
- `Demo.ability_upgrades` property for hero ability point spending events.
- `Demo.item_purchases` property for item shop transactions.
- `Demo.chat` property for in-game chat messages.
- `Demo.objectives` property for objective health state changes.
- `Demo.mid_boss` property for mid boss lifecycle events.
- `Demo.troopers` property for per-tick alive lane trooper state (opt-in, large dataset).
- `Demo.neutrals` property for neutral creep state changes with change detection (opt-in).
- `Demo.stat_modifier_events` property for permanent stat bonus change events (opt-in).
- `Demo.active_modifiers` property for active buff/debuff modifier events (opt-in).
- `Demo.urn` property for urn (idol) lifecycle events (picked up, dropped, returned) and delivery point tracking (active, inactive with position and team).
- `Demo.street_brawl_ticks` property for per-tick street brawl state (round, scores, state transitions).
- `Demo.street_brawl_rounds` property for street brawl round scoring events.
- `NotStreetBrawlError` exception raised when accessing street brawl datasets on non-street-brawl demos.
- `Demo.winning_team_num`, `Demo.game_over_tick` properties for game-over state (lazy-scanned on first access).
- `Demo.available_datasets()` static method returning the list of valid dataset names.
- `Demo.load()` method to batch-load multiple datasets in a single parse pass.
- All DataFrame properties auto-load on first access and can be batch-loaded via `load()`.
- `hero_names()` module-level function returning `dict[int, str]` of hero ID to name.
- `team_names()` module-level function returning `dict[int, str]` of team number to name.
- `ability_names()` module-level function returning `dict[int, str]` of ability hash ID to name.
- `modifier_names()` module-level function returning `dict[int, str]` of modifier hash ID to name.
- `game_mode_names()` module-level function returning `dict[int, str]` of game mode ID to name.
- Custom exceptions: `InvalidDemoError`, `DemoHeaderError`, `DemoInfoError`, `DemoMessageError`.

### boon-cli

- CLI with commands: `verify`, `info`, `messages`, `classes`, `send-tables`, `string-tables`, `entities`, `events`.
- `ability-upgrades` command for tracking hero ability point spending (skill tier upgrades).
- `shop-events` command for item shop transactions (purchased, upgraded, sold, swapped, failure).
- `chat` command for in-game chat messages (all chat and team chat).
- `objectives` command for per-tick objective entity health (walkers, titans, barracks, mid boss).
- `mid-boss` command for mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire).
- `troopers` command for per-tick alive lane trooper state (position, health, lane).
- `neutrals` command for neutral creep state changes with change detection.
- `stat-modifiers` command for per-player cumulative permanent stat bonuses.
- `active-modifiers` command for active buff/debuff modifier events.
- All commands support `--filter`, `--summary`, `--limit`, and `--json` flags.
