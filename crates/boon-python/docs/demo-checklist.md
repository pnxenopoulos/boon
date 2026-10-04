# Replay verification checklist

Use this checklist with the demo open in the game viewer and the current Boon
build installed. Record viewer values **before** you inspect Boon's results.
Compare values; a file that opens can still have incorrect results.

## Manual pass with the viewer open

For each check, record **pass**, **fail**, **not observed**, or **not directly
observable**. Record **not observed** for events missing from this match.
Use this log for discrepancies:

| Demo tick / match clock | Player / entity | Dataset and field | Viewer observation | Boon value | Outcome / notes |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

### 1. Establish identity and time

- [ ] Match ID, map, mode, player names and teams agree with `demo` metadata and
  `demo.players`. Compare bans if the viewer exposes them.
- [ ] Hero names agree with the viewer. Hero, ability and modifier lookups come
  from boon-data; record the catalog client version used. A name lookup can
  download data if no verified local version is installed.
- [ ] Compare `demo.tick_to_match_clock(tick)` with the displayed match clock
  near the start, middle and end. Include a match pause if one occurred.
- [ ] Record demo ticks wherever the viewer exposes them. Do not infer a tick
  by multiplying the displayed match clock by the tick rate.
- [ ] If a player switches heroes, compare events before and after the switch.
  `demo.players` describes the game-over/final snapshot, not the starting roster;
  use player identity and event time to follow a switch.

### 2. Pause at three quiet moments

Choose an early, middle and late tick. Compare **each player** at each tick with
`demo.snapshots(ticks=[...])`:

- [ ] Current/max health, level, alive/dead state and kills/deaths/assists match
  the HUD or scoreboard.
- [ ] Souls, last hits and denies match where visible. Compare the same quantity:
  current spendable currency is not total earned souls or net worth.
- [ ] Position and facing agree with the view/minimap. Exact world coordinates
  and angles are comparable only with a coordinate readout; a visual comparison shows only placement.
- [ ] Shop/regen-zone flags and rejuvenator/rebirth possession match where
  independently observable.

### 3. Compare every dataset

Inspect the event tick and the ticks before and after it.
Use one clear event, then a repeated event or a transition.
Record an outcome for each of these 22 datasets:

| Dataset | Compare with the viewer |
| --- | --- |
| `player_ticks` | Health, barrier, souls, counters, position, facing, shop/regen flags, and death/respawn transitions. |
| `world_ticks` | Match pause/resume and the next-mid-boss timer. Playback pauses are different. |
| `kills` | Victim, killer, assists, tick, and the resulting K/D/A changes. |
| `damage` | Source, recipient, health, shields, hitgroup, critical hits, light/heavy melee, and lethal overkill. |
| `abilities` | Player, ability, and tick. This table contains important usage events, not every cast. |
| `ability_upgrades` | Player, ability, spent point, and resulting tier. |
| `ability_ticks` | Cooldown and charge transitions. Retain the last row; this table records changes only. |
| `item_purchases` | Purchase, upgrade, sale/swap, current hero, and component links by Steam ID and tick. |
| `chat` | Text, sender, channel, and current hero, including after a hero change. |
| `flex_slots` | Team and unlock tick. |
| `objectives` | Team, health, destruction, and patron phase. |
| `mid_boss` | Spawn, death, and rejuvenator pickup/use/expiry. |
| `troopers` | A wave's team, lane, position, and health. Entity indices can be reused. |
| `neutrals` | A camp's entity identity, location, health, and state changes. This table records changes only. |
| `breakables` | Destroyed prop, subclass, position, entity, and serial. Leaving view must not count as destruction. |
| `sinners_sacrifice` | Spawn, hits, attacker, damage, health, and reset. Same-tick hits can share final health. |
| `stat_modifier_events` | Permanent bonus type, signed amount, player, and tick. These are bonus changes, not final stats. |
| `active_modifiers` | Source, recipient, application, stacks, refresh, and removal. Visible expiry can precede recorded removal. |
| `urn` | Carrier pickup/drop/return and delivery-point activation. |
| `rift` | Announcement, activation, capture/expiry, winner, lane, and position. An unfinished Rift has no row. |
| `street_brawl_ticks` | Buy/combat transitions, round, countdowns, and team scores. |
| `street_brawl_rounds` | Completed round, end tick, and cumulative scores. |

A damage message can differ from one bullet or the net health decrease.
Concurrent damage, regeneration, and absorption can change the comparison.
Ability timers use engine game time, not the displayed match clock.
Use `get_item_purchases(data_version=...)` for component links; retain recorded `change` values.

### 4. Use other replays for missing events

- [ ] Compare a pause, death/respawn, and rejuvenator use when present.
- [ ] Compare a hero change before and after the switch.
- [ ] Use a Street Brawl replay for rounds and scores.
  Other modes must raise `NotStreetBrawlError` for these two datasets.
- [ ] Record **not observed** for missing map events. Use another replay to compare those events.

### 5. Compare the ending and calculated results

- [ ] Winner, `game_over_tick`, and `regulation_*` values agree with the viewer. Regulation duration does not include pauses.
- [ ] The six `demo.summary()` tables agree with the corresponding available
  post-match screens: `snapshots`, `gold_sources`, `last_hits`, `objectives`, `damage`, and `healing`. Compare matching
  categories; do not add category totals to their component rows.
- [ ] Compare healing and regeneration by `stat_type`. Use `total` at one reporting period and `amount` for intervals.
  Do not add cumulative totals across periods. Snapshot and matrix reporting times can differ.
- [ ] Compare `gold_sources` with the soul breakdown and `last_hits` with the final scoreboard.
- [ ] Manually calculate one player's kill participation and death duration,
  then compare `kill_participation()` and `time_dead()`. Death duration does not include
  match pauses and post-game time.
- [ ] Review `in_combat()` and `teamfights()` independently. Combat windows can have no
  direct viewer readout; teamfights use a heuristic and have no official
  scoreboard answer. Do not count visual plausibility as exact verification.

### 6. Compare stats, states, and imbues

Use the matching boon-data client version. See [feature examples](examples.md#stats-states-and-ammo).

- [ ] Record `data_version`, tick, Steam ID, query mode, and the viewer value.
  Use the tick passed to `demo_gototick`; the pause message can show a server tick.
- [ ] Compare each supported hero stat. Read units, `status`, and `diagnostic`.
  `partial` can lack an effect; `unresolved` does not mean zero.
- [ ] Compare `baseline` and `current` before and during a temporary effect.
  Movement values stay nominal in the two modes; firing, crouching, bullet-hit slows, and sprint acceleration are not simulated.
- [ ] Compare `ammo_fraction`, `ammo`, and finite `max_ammo`. Compare
  `unlimited_ammo` during a slide or another unlimited-ammo effect.
- [ ] Compare `player_states()` with visible combat, movement, and debuff states.
  Read unknown bits and null masks before you interpret missing flags.
- [ ] Compare `imbues().bindings` with each item selection. Make sure ability
  stat bonuses apply to the selected ability, including item cooldown rules.
- [ ] Compare the individual summary `player_healing` and `barrier_absorption` columns.
  Some viewer screens show their combined total.

### 7. Record the limits of the check

- [ ] Leave unobservable IDs, exact timers and coordinates marked unverified
  unless you have an different readout. Record HUD rounding or interpolation
  differences instead of shifting ticks to make values agree.
- [ ] Do not expect `demo.healing` or `demo.barriers()`. Use the stat query
  methods to calculate supported hero and ability values.
  Raw healing counters, `player_ticks.barrier` and damage shield fields stay;
  they do not reconstruct healing or barrier events.
- [ ] Compare calculated stats with the viewer. Recorded bonus events are not final stats.

- [ ] Record a second replay with features missing from the first. Examples include
  hero switching, pauses, Street Brawl, and optional map events.

### 8. Compare API paths, CLI, and downloads

- [ ] Fresh property access, bulk `load()`, repeated access, and duplicate load
  requests produce consistent data. The script exercises the bulk/cache paths;
  use `Demo(path, preload=False)` for checks of individual lazy loads below.
- [ ] Snapshot selection works for single/list ticks, windows, `every`, `seconds`,
  and event unions. Single-dataset results are frames; multiple datasets return
  a dictionary. The script compares sampled rows against full frames and compares
  fresh and cached event paths.
- [ ] `boon --version`, `verify`, `info`, `players`, `datasets`, `show`, `summary`,
  and each supported `stats --metric` command agree with the Python results.
  Use `--json` and inspect the parsed values, as well as the exit status.
- [ ] Invalid filenames, invalid dataset names, missing snapshot selectors, and
  incompatible selectors fail cleanly. Retired datasets are missing from
  `boon datasets` and rejected by `load()`/`show`/event selection.
- [ ] `boon versions`: client version, source `VersionDate`/`VersionTime`, and
  installed status are plausible. The release timestamp stays available in
  `--json`, not the table. Source dates have no inferred timezone. `boon versions --local` works offline.
- [ ] `boon get VERSION` downloads the exact selected release, repeated get uses
  the verified cache, and installed JSON assets/manifest/checksums agree.
  `boon remove VERSION` removes only that local version. Test `--force` and
  corrupt-cache repair in different tests of installation behavior.
- [ ] For parser debugging, the Rust `boon-dev` CLI can inspect raw
  entities, messages, classes and string tables. It is not part of this Python
  audit; see {doc}`cli` for its commands.

## Inspect a tick while watching

Use the Python API directly. Raw snapshots use no `data_version`.
Set `data_version` for stats, states, imbues, and calculated ammo.
Replace the filename and ticks with those you are viewing.

```python
import polars as pl
from boon import Demo, hero_names

demo = Demo("108575009.dem", preload=False)
ticks = [187554]
names = hero_names(version="6712")

print(demo.players)
for tick in ticks:
    print(tick, demo.tick_to_match_clock(tick))

state = demo.snapshots(["player_ticks", "world_ticks"], ticks=ticks)
players = state["player_ticks"].with_columns(
    pl.col("hero_id").replace_strict(names, default=None).alias("hero")
)
with pl.Config(tbl_rows=40, tbl_cols=12):
    print(players.select(
        "tick", "steam_id", "hero_id", "hero", "health", "max_health", "level",
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
evidence for the manual checks. Install Boon and Polars in the environment
used to run it. It does not change the replay or parser.

## Run the automated checks

From the repository root, with the current Boon build installed:

```bash
python scripts/check-demo.py 108575009.dem \
  --out target/demo-checks/108575009 \
  --ticks 187554
```

Use a new output directory for each run. On Windows, enter the command on one
line. If using this repository's WSL virtual environment, replace `python` with
`crates/boon-python/.venv/bin/python`.

Without `--ticks`, the script selects three ticks from the observed player history. You can
supply several: `--ticks 30000 187554 70000`. Use **demo ticks**, not match seconds.
The report includes the two time conversions for the selected ticks.

The full run loads and exports all datasets supported by the replay's mode,
including the large `player_ticks` and `troopers` tables. Prepare several GB of
RAM and space for the Parquet files. Use `--skip-troopers` to export sampled
troopers only; full trooper coverage will explicitly stay unverified.
`--cli` launches additional parses to exercise the CLI.

To also do checks of real data downloads and removal:

```bash
python scripts/check-demo.py 108575009.dem \
  --out target/demo-checks/108575009-with-data \
  --ticks 187554 --cli --data-version 6712
```

Choose an available client version from `boon versions`. This option uses
network access. It uses a temporary cache and does catalog integrity checks through
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
- [ ] Another supported client: do a check of missing fields and unknown name IDs.
- [ ] Incomplete/late-start replay: report missing pregame/end/summary data. A missing frame is not a zero-valued match statistic.
- [ ] Replays containing the optional events below, including captured and
  expired Rifts to compare the two outcomes.

For every manual check, record: replay filename, build, player identity, demo
tick or interval, expected observation, actual rows, and pass/fail/not-observed.
Do not treat a roster's hero ID as a player's permanent identity across a swap.
Use the known player/Steam identity, their hero history, and the event time.

## Inspect the exported evidence

```python
from pathlib import Path
import polars as pl

folder = Path("target/demo-checks/108575009")
tick, steam_id = 187554, 76561198037652386  # McGinnis

players = pl.scan_parquet(folder / "player_ticks.parquet")
selected = players.filter((pl.col("tick") == tick) & (pl.col("steam_id") == steam_id)).collect()
print(selected)
hero = selected["hero_id"].item()

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

fresh = Demo("108575009.dem", preload=False)
assert_frame_equal(fresh.damage, pl.read_parquet(folder / "damage.parquet"))
```

## Turn different observations into repeatable assertions

Transcribe a value from the replay UI, not from Boon's output. Suppose McGinnis has
123 health at demo tick 187554. Save the following JSON in `observations.json`.
**123 is an example. Replace it with the value you observe.**

```json
[
  {
    "dataset": "player_ticks",
    "where": {"tick": 187554, "steam_id": 76561198037652386},
    "expect": {"health": 123},
    "tolerance": 0
  }
]
```

Run the script with `--observations observations.json` and a fresh `--out`.
Each filter must identify exactly one row; add fields to disambiguate multiple
messages in the same tick. `tolerance` is an absolute numeric tolerance, default
zero. You can assert several columns for each row and use any exported table name,
including `summary_last_hits` or `street_brawl_rounds`. JSON `null` matches null.
All mismatches are recorded as failures with expected and actual values.

Record an explanation or another replay for each `FAIL`, `REVIEW`, `SKIP`, and incomplete manual check before you finish.
This workflow is a replay audit, not a substitute for the repository's tests
of malformed inputs, parallel parsing, error handling, and multiple client versions.
