# Replay verification checklist

Use this checklist with the demo open in the game viewer and the current Boon
build installed. Record viewer values **before** you inspect Boon's results.
Compare values as well as checking that the file can be read.

## Manual pass with the viewer open

For each check, record **pass**, **fail**, **not observed**, or **not directly
observable**. Record **not observed** for events absent from this match.
Use this log for discrepancies:

| Demo tick / match clock | Player / entity | Dataset and field | Viewer observation | Boon value | Outcome / notes |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

### 1. Establish identity and time

- [ ] Match ID, map, mode, player names and teams agree with `demo` metadata and
  `demo.players`. Compare bans if the viewer exposes them.
- [ ] Hero names agree with the viewer. Hero, ability and modifier lookups come
  from boon-data; record the catalog client version used. A name lookup can
  download data automatically if no verified local version is installed.
- [ ] Compare `demo.tick_to_match_clock(tick)` with the displayed match clock
  near the start, middle and end. Include a match pause if one occurred.
- [ ] Record demo ticks wherever the viewer exposes them. Do not infer a tick
  by multiplying the displayed match clock by the tick rate.
- [ ] If a player switches heroes, compare events before and after the switch.
  `demo.players` describes the game-over/final snapshot, not the starting roster;
  use player identity and event time to follow a switch.

### 2. Pause at three quiet moments

Choose an early, middle and late tick. Check **each player** at each tick with
`demo.snapshots(ticks=[...])`:

- [ ] Current/max health, level, alive/dead state and kills/deaths/assists match
  the HUD or scoreboard.
- [ ] Souls, last hits and denies match where visible. Compare the same quantity:
  current spendable currency is not total earned souls or net worth.
- [ ] Position and facing agree with the view/minimap. Exact world coordinates
  and angles require a coordinate readout; a visual comparison shows only placement.
- [ ] Shop/regen-zone flags and rejuvenator/rebirth possession match where
  independently observable.

### 3. Follow clear gameplay events

For each event, inspect a tick before, the event tick, and a tick after. Start
with isolated actions; use a busy fight as an additional check.

- [ ] **Kills:** compare at least three victims, killers and visible assist credits
  against `kills`; confirm the corresponding K/D/A changes in `player_ticks`.
- [ ] **Death/respawn:** observe one full cycle; compare `is_alive`, health and
  the respawn transition. Include rebirth if present.
- [ ] **Damage:** inspect a body shot, headshot, light/heavy melee and an ability
  hit where available. Compare source, recipient, damage and resulting health with
  `damage`. One damage message can differ from one bullet or the net health decrease
  between two snapshots; simultaneous damage, regeneration and absorption matter.
- [ ] **Abilities:** compare recorded casts in `abilities`, an upgrade in
  `ability_upgrades`, and cooldown/charge transitions in `ability_ticks`.
  `abilities` records important usage events, not necessarily every cast.
  `ability_ticks` is change-only; retain the last state for comparisons with later ticks.
  Its timer timestamps use engine game time, not the displayed match clock.
- [ ] **Items:** verify a purchase, upgrade and sale/swap if present against
  `item_purchases`. Compare the item, hero, action and timing.
- [ ] **Chat:** verify visible text, sender, channel and current hero in `chat`.
  Include messages after a hero switch when available.
- [ ] **Buffs/debuffs:** compare visible application and stack changes with
  `active_modifiers`. Compare source and recipient where known. Record visible
  expiry separately from replicated modifier removal; they can differ.
- [ ] **Permanent pickups:** compare a visible permanent bonus with
  `stat_modifier_events`. These are recorded bonus changes, not final attributes.

### 4. Follow the map events present in this match

- [ ] `objectives` and `flex_slots`: verify destruction, team, timing, patron
  phase changes and slot unlocks.
- [ ] `world_ticks` and `mid_boss`: verify actual match pause/resume, boss
  spawn/death and rejuvenator events. Pausing playback is not a match pause.
- [ ] `urn` and `rift`: follow carrier transitions/delivery-point activation,
  and Rift activation/capture/expiry. Compare teams and locations.
- [ ] `troopers` and `neutrals`: follow a wave and a camp through combat/death;
  compare team, location and health where visible.
- [ ] `breakables` and `sinners_sacrifice`: verify an actual prop destruction,
  and Sacrifice spawn/hits/reset where present.
- [ ] For Street Brawl, compare `street_brawl_ticks` and `street_brawl_rounds`
  against round transitions, countdowns, scoring team and cumulative scores.
  These require a Street Brawl replay.

### 5. Compare the ending and calculated results

- [ ] Winner and game-over timing agree with the viewer.
- [ ] The six `demo.summary()` tables agree with the corresponding available
  post-match screens: snapshots, soul sources, last hits, objectives, damage, and healing. Compare like
  categories; do not add category totals to their component rows.
- [ ] Manually calculate one player's kill participation and death duration,
  then compare `kill_participation()` and `time_dead()`. Death duration excludes
  match pauses and post-game time.
- [ ] Review `in_combat()` and `teamfights()` separately. Combat windows can have no
  direct viewer readout; teamfights use a heuristic and have no official
  scoreboard answer. Do not count visual plausibility as exact verification.

### 6. Verify stats, states, and imbues

Use the matching boon-data client version. See [feature examples](examples.md#stats-states-and-ammo).

- [ ] Record `data_version`, tick, Steam ID, query mode, and the viewer value.
  Use the tick passed to `demo_gototick`; the pause message can show a server tick.
- [ ] Compare each supported hero stat. Check units, `status`, and `diagnostic`.
  `partial` can omit an effect; `unresolved` does not mean zero.
- [ ] Compare `baseline` and `current` before and during a temporary effect.
  Movement values remain nominal in both modes.
- [ ] Compare `ammo_fraction`, `ammo`, and finite `max_ammo`. Check
  `unlimited_ammo` during a slide or another unlimited-ammo effect.
- [ ] Compare `player_states()` with visible combat, movement, and debuff states.
  Check unknown bits and null masks before you interpret absent flags.
- [ ] Compare `imbues().bindings` with each item selection. Check that ability
  stat bonuses apply to the selected ability, including item cooldown rules.
- [ ] Compare summary `player_healing` and `barrier_absorption` separately.
  Some viewer screens show their combined total.

### 7. Record the limits of the check

- [ ] Leave unobservable IDs, exact timers and coordinates marked unverified
  unless you have an independent readout. Record HUD rounding or interpolation
  differences rather than silently shifting ticks to make values agree.
- [ ] Do not expect `demo.healing` or `demo.barriers()`. Use the stat query
  methods to calculate supported hero and ability values.
  Raw healing counters, `player_ticks.barrier` and damage shield fields remain;
  they do not reconstruct healing or barrier events.
- [ ] Do not compare `stat_modifier_*` directly with final UI ammo, fire rate,
  lifesteal or resistances. These columns omit base values and some effects.
- [ ] Record a second replay with features absent from the first. Examples include
  hero switching, pauses, Street Brawl, and optional map events.

## Inspect a tick while watching

This uses the Python API directly. No CLI flags are required. Raw snapshots
do not require `data_version`; stats, states, imbues, and calculated ammo do.
Replace the filename and ticks with those you are viewing.

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("106996573.dem")
ticks = [50707]
names = hero_names()

print(demo.players)
for tick in ticks:
    print(tick, demo.tick_to_match_clock(tick))

state = demo.snapshots(["player_ticks", "world_ticks"], ticks=ticks)
players = state["player_ticks"].with_columns(
    pl.col("hero_id").replace_strict(names, default=None).alias("hero")
)
with pl.Config(tbl_rows=40, tbl_cols=12):
    print(players.select(
        "tick", "hero_id", "hero", "health", "max_health", "level",
        "is_alive", "souls", "kills", "deaths", "assists",
    ))
    print(state["world_ticks"])
```

The helper below inspects nearby event messages. Use it for event tables with a
`tick` column; it does not reconstruct change-only state at an arbitrary tick.

```python
def events_near(dataset: str, tick: int, seconds: float = 1.0) -> pl.DataFrame:
    radius = round(seconds * demo.tick_rate)
    return (
        getattr(demo, dataset)
        .filter(pl.col("tick").is_between(tick - radius, tick + radius))
        .sort("tick")
    )

print(events_near("kills", ticks[0]))
print(events_near("damage", ticks[0]))
print(events_near("item_purchases", ticks[0]))
```

## Optional automated audit

`scripts/check-demo.py` does checks of API behavior and internal consistency. It exports
evidence for the manual checks. It requires Boon and Polars in the environment
used to run it. It does not change the replay or parser.

## Run the automated checks

From the repository root, with the current Boon build installed:

```bash
python scripts/check-demo.py 106996573.dem \
  --out target/demo-checks/106996573 \
  --ticks 50707
```

Use a new output directory for each run. On Windows, enter the command on one
line. If using this repository's WSL virtual environment, replace `python` with
`crates/boon-python/.venv/bin/python`.

Omit `--ticks` to select three ticks from the observed player history. You can
supply several: `--ticks 30000 50707 70000`. Use **demo ticks**, not match seconds.
The report includes both time conversions for the selected ticks.

The full run loads and exports all datasets supported by the replay's mode,
including the large `player_ticks` and `troopers` tables. Allow several GB of
RAM and space for the Parquet files. Use `--skip-troopers` to export sampled
troopers only; full trooper coverage will explicitly remain unverified.
`--cli` launches additional parses to exercise the CLI.

To also do checks of real data downloads and removal:

```bash
python scripts/check-demo.py 106996573.dem \
  --out target/demo-checks/106996573-with-data \
  --ticks 50707 --cli --data-version 6698
```

Choose an available client version from `boon versions`. This option requires
network access. It uses a temporary cache and verifies the catalogs through
the Python data API. It tests a cached download and CLI removal.
Your existing `~/.boon` installation is unchanged.

Output:

- `report.json`: Boon version, replay metadata, checks, coverage gaps, schemas,
  row counts, previews, and selected tick/time conversions.
- `<dataset>.parquet`: complete loaded tables, including players and bans.
- `summary_*.parquet`: the six post-match tables, when available.
- `selected_*.parquet`: player, world, and trooper snapshots at your review ticks.
- `in_combat.parquet`, `kill_participation.parquet`, `time_dead.parquet`, and
  `teamfights.parquet`: derived metrics when their prerequisites are available.
- `*_names.json`: the resolved name lookup tables, including those read from boon-data.

`PASS` means the automated check passed. `FAIL` means an exception or an
inconsistency. `REVIEW` identifies results for human investigation, including empty datasets or
missing post-match data. `SKIP` means a path was not exercised. Exit code 1 means
at least one failure; exit code 0 **does not** mean manual verification is complete.
The script writes the report after each check, so partial progress survives a
later failure. An empty table is not evidence that the feature works.

## Build a replay coverage set

No single replay can prove every feature. Track which file covers each case:

- [ ] Complete standard match with its ending and post-match statistics.
- [ ] Street Brawl match: test rounds, scores, transitions, and mode guards.
- [ ] Early hero switch, including subsequent chat and purchases. Match
  `100655353.dem` is the existing Silver-to-Victor regression example.
- [ ] Pause/resume, death/respawn, and rebirth/rejuvenator if available.
- [ ] Earlier supported client: do a check of absent fields and unknown name IDs.
- [ ] Incomplete/late-start replay: missing pregame/end/summary must be handled
  honestly. A missing frame is not a zero-valued match statistic.
- [ ] Replays containing the optional events below, including both captured and
  expired Rifts if you want to verify both outcomes.

For every manual check, record: replay filename, build, player identity, demo
tick or interval, expected observation, actual rows, and pass/fail/not-observed.
Do not treat a roster's hero ID as a player's permanent identity across a swap.
Use the known player/Steam identity, their hero history, and the event time.

## Watch the replay and compare

### Opening, identity, and clocks

- [ ] `verify()`, metadata, `players`: match ID, map, mode, roster, Steam IDs,
  teams, and starting lanes agree with the replay. Verify the winner and ending.
- [ ] `banned_heroes`: compare bans when recorded. Empty does not establish that
  the match had no bans; some clients omit the message.
- [ ] Name lookups: verify hero, team, ability/internal item name, localized
  ability name, modifier, game mode, patron phase, hitgroup, and lifestate labels.
  For unknown IDs, examine the selected boon-data version and catalog contents.
- [ ] Check `tick_to_seconds`, `tick_to_clock_time`, `tick_to_match_seconds`, and
  `tick_to_match_clock` at pregame, start, middle, and end. Include a pause.
  Demo time and the displayed game clock have different semantics; record any
  mismatch instead of adjusting expected values to make the check pass.
- [ ] Check `game_start_tick`, `pregame_seconds`, `game_over_tick`, and all
  `regulation_*` values. Regulation duration excludes paused ticks.

### Every registered dataset

Use one clear event of each kind. Then do a check of a boundary or repeated event. Mark missing events **not observed**, and find another replay if necessary.

| Dataset | What to compare with the replay |
| --- | --- |
| `player_ticks` | Position, orientation, health, alive/dead state, level, souls, shop/regen-zone flags, death/respawn times, counters, and recorded stat modifiers at several ticks. Check a hero swap and a respawn boundary. |
| `world_ticks` | Pause/resume state and the next-midboss timer. |
| `kills` | Victim, killer, assistants, and tick for several kills, including any unusual death. |
| `damage` | Attacker/victim, ability/weapon, damage, remaining health, crit/hitgroup, melee flags, and available shield/message fields. Check light/heavy melee and lethal overkill. Messages are not necessarily individual bullets. |
| `abilities` | Who cast which ability and when. Distinguish signature casts from movement abilities where applicable. |
| `ability_upgrades` | Which hero spent an ability point, on which ability, and the resulting tier. Earlier demos can have no supported upgrade data. |
| `ability_ticks` | Cooldown start/end and charge changes around a cast/recharge. This table is change-only; absence of a row on a later tick is not absence of the ability. |
| `item_purchases` | Purchase, upgrade, sell/swap events that occur; verify item and hero identity after a hero switch. |
| `chat` | Message text, sender, channel/team information, and the hero active when the message was sent. |
| `flex_slots` | Unlock tick and team. |
| `objectives` | Objective identity/team, health changes, destruction, and patron phase transitions when recorded. |
| `mid_boss` | Spawn/kill and recorded rejuvenator pickup/use/expiry events. |
| `rift` | Announcement, activation, capture **or** expiry, winning team, lane and location. A Rift still active at EOF is omitted. |
| `troopers` | One wave's team, lane, position and health; compare sampled rows with the full table. The game can reuse entity indices later. |
| `neutrals` | A camp's entity identity, location and state changes through combat/death. This is not a full per-tick table. |
| `breakables` | A visible destroyed prop, its position and entity/serial; moving out of view or a keyframe replacement must not create a false destruction. |
| `sinners_sacrifice` | Spawn, hit and reset; attacker when known, damage and final tick health. Multiple hits in a tick can share end-of-tick health. |
| `stat_modifier_events` | A permanent pickup's stat type, signed amount, hero and tick. These are changes to recorded bonuses, not final hero attributes. |
| `active_modifiers` | A buff/debuff's application, stack change and removal; source, recipient and duration. Check expiry/refresh around a pause or keyframe. |
| `urn` | Carrier pickup/drop/return and delivery-point active/inactive transitions. Do not require an event name that the API does not expose. |
| `street_brawl_ticks` | Buy/combat transitions, round number, countdowns and both scores in a Street Brawl replay. |
| `street_brawl_rounds` | Each completed round's end tick and cumulative team scores. Standard matches must raise `NotStreetBrawlError` for these datasets. |

- [ ] All 22 rows above have a recorded outcome or an explicit coverage gap.
- [ ] The recorded `player_ticks.barrier`, healing counters, and damage-message
  shield fields are compared only with their documented raw observations. There is
  no `demo.healing` or `demo.barriers()`. Calculated stats use separate methods.
- [ ] Check `stat_modifier_values_available` and `unknown_stat_modifier_count`.
  Never compare `stat_modifier_*` directly with final UI resistances/fire rate.
- [ ] Do not require `health <= max_health` at every transitional tick without
  a comparison with the game; temporary health effects and replication can complicate it.

### Summary and derived metrics

- [ ] `summary()` has exactly `snapshots`, `last_hits`, `objectives`, `damage`, `healing`, and `gold_sources`.
  Compare final kills/deaths/assists, net worth, last hits, objectives and damage
  totals with the scoreboard and post-match screens.
- [ ] In `summary_damage`, filter `stat_type` to the quantity for comparison and
  exclude category duplicates (`is_category=False`). Values are interval
  amounts; summing categories and individual sources double-counts them.
  Raw healing/regen categories remain part of this original matrix.
- [ ] `summary_healing` contains healing and regeneration amounts and cumulative totals.
  Compare totals by `stat_type` with the corresponding rows in `summary_damage`.
  Use `amount` for intervals and `total` at one reporting period. Category duplicates
  are already excluded. Periods with zero change remain available.
- [ ] Compare `summary_gold_sources` with the soul source breakdown. Compare
  `summary_snapshots` healing counters and damage by target type with the game.
  Use the recorded times; snapshot and matrix schedules can differ.
- [ ] `kill_participation()`: manually count kills, assists and team kills for
  one hero; test a bounded interval too. If the team has no kills, the ratio is null.
- [ ] `in_combat()`: inspect entry/exit around one hero hit and one NPC hit;
  compare with the recorded combat window, including pause behavior.
- [ ] `time_dead()`: verify one death/respawn interval and a player's match total;
  paused ticks and post-game time must not contribute. Missing game-over data
  means this metric is unavailable.
- [ ] `teamfights()`: inspect one fight and two simultaneous distant skirmishes.
  Review participants, time bounds, damage and kills. This is a heuristic:
  test `gap_seconds`, `radius`, and `min_players` against your intended definition.

### Calculated stats, imbues, and state flags

- [ ] Use the demo's client version for `data_version`.
  Record the selected source commit from the result metadata.
- [ ] Select stats with `HeroStat` or `AbilityStat` enum members.
  Compare the reported units with the viewer. Check `partial` and `unresolved` rows.
- [ ] Select a player with `steam_ids`, using an ID from `demo.players`.
  Check that the selection follows the player through a hero change.
- [ ] Use `explain=True` to inspect base values, active effects, upgrades, and recorded counters.
  Do not add intermediate input rows to the final value.
- [ ] Compare `imbues().bindings` with the viewer's item and ability selections.
  Check that ability bonuses apply only to their recorded targets.
- [ ] Inspect `player_states()` before, during, and after a slide or sprint.
  Keep the three masks separate. Check unknown bit indices and null values.

Calculated movement speeds exclude current firing, crouching, and slow states.
State flags do not adjust those speeds. See {doc}`hero-stats`, {doc}`ability-stats`,
and {doc}`player-states` for the full rules and limits.

### API paths, CLI and downloads

- [ ] Fresh property access, bulk `load()`, repeated access, and duplicate load
  requests produce consistent data. The script exercises the bulk/cache paths;
  use `Demo(path, preload=False)` for checks of separate lazy loads below.
- [ ] Snapshot selection works for single/list ticks, windows, `every`, `seconds`,
  and event unions. Single-dataset results are frames; multiple datasets return
  a dictionary. The script compares sampled rows against full frames and does a check of
  the fresh versus cached event path.
- [ ] `boon --version`, `verify`, `info`, `players`, `datasets`, `show`, `summary`,
  and each supported `stats --metric` command agree with the Python results.
  Use `--json` and inspect the parsed values, not just the exit status.
- [ ] Invalid filenames, invalid dataset names, missing snapshot selectors, and
  incompatible selectors fail cleanly. Retired datasets are absent from
  `boon datasets` and rejected by `load()`/`show`/event selection.
- [ ] `boon versions`: client version, source `VersionDate`/`VersionTime`, and
  installed status are plausible. The release timestamp remains available in
  `--json`, not the table. Source dates have no inferred timezone. `boon versions --local` works offline.
- [ ] `boon get VERSION` downloads the exact selected release, repeated get uses
  the verified cache, and installed JSON assets/manifest/checksums agree.
  `boon remove VERSION` removes only that local version. Test `--force` and
  corrupt-cache repair separately if validating release/install robustness.
- [ ] For parser debugging, the separate Rust `boon-dev` CLI can inspect raw
  entities, messages, classes and string tables. It is not part of this Python
  audit; see {doc}`cli` for its commands.

## Inspect the exported evidence

```python
from pathlib import Path
import polars as pl

folder = Path("target/demo-checks/106996573")
tick, hero = 50707, 66

players = pl.scan_parquet(folder / "player_ticks.parquet")
print(players.filter((pl.col("tick") == tick) & (pl.col("hero_id") == hero)).collect())

# Inspect nearby messages; a kill/cast/hit can occur at a different tick.
hits = pl.scan_parquet(folder / "damage.parquet")
print(hits.filter(
    pl.col("tick").is_between(tick - 64, tick + 64)
    & ((pl.col("attacker_hero_id") == hero) | (pl.col("victim_hero_id") == hero))
).collect())

# Inspect a recorded summary statistic without double-counting categories.
matrix = pl.read_parquet(folder / "summary_damage.parquet")
print(matrix.filter((pl.col("stat_type") == "damage") & ~pl.col("is_category"))
      .filter(pl.col("dealer_steam_id").is_not_null())
      .group_by("dealer_steam_id").agg(pl.col("damage").sum()))
```

Compare a fresh lazy load with the exported evidence from the bulk load:

```python
from boon import Demo
from polars.testing import assert_frame_equal

fresh = Demo("106996573.dem", preload=False)
assert_frame_equal(fresh.damage, pl.read_parquet(folder / "damage.parquet"))
```

## Turn independent observations into repeatable assertions

Transcribe a value from the replay UI, not from Boon's output. Suppose Victor has
123 health at demo tick 50707. Save the following JSON in `observations.json`.
**123 is an example. Replace it with the value you observe.**

```json
[
  {
    "dataset": "player_ticks",
    "where": {"tick": 50707, "hero_id": 66},
    "expect": {"health": 123},
    "tolerance": 0
  }
]
```

Run the script with `--observations observations.json` and a fresh `--out`.
Each filter must identify exactly one row; add fields to disambiguate multiple
messages in the same tick. `tolerance` is an absolute numeric tolerance, default
zero. You can assert several columns per row and use any exported table name,
including `summary_last_hits` or `street_brawl_rounds`. JSON `null` matches null.
All mismatches are recorded as failures with expected and actual values.

Do not sign off until every relevant `FAIL`, `REVIEW`, `SKIP`, and manual
checklist item has an explanation or a follow-up replay that supplies coverage.
This workflow is a per-replay audit, not a substitute for the repository's tests
of malformed inputs, parallel parsing, error handling, and multiple client versions.
