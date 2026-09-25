# Replay verification checklist

Use this checklist with the current Boon build and `scripts/check-demo.py` in the
repository. The script requires Boon and Polars in the Python environment used
to run it. It does not change the replay or the parser.

A successful parse does **not** prove that values match the game. The script
checks API behavior and internal consistency, exports evidence, and leaves
in-game verification to you. Complete both parts before signing off a replay.

## Run the automated checks

From the repository root, with the current Boon build installed:

```bash
python scripts/check-demo.py 106996573.dem \
  --out target/demo-checks/106996573 \
  --ticks 50707 --cli
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

To also check real data downloads and removal:

```bash
python scripts/check-demo.py 106996573.dem \
  --out target/demo-checks/106996573-with-data \
  --ticks 50707 --cli --data-version 6698
```

Choose an available client version from `boon versions`. This option requires
network access. It uses a temporary cache, verifies the installed catalogs
through the Python data API, exercises a cached get and CLI removal, and leaves
your real `~/.boon` installation untouched.

Output:

- `report.json`: Boon version, replay metadata, checks, coverage gaps, schemas,
  row counts, previews, and selected tick/time conversions.
- `<dataset>.parquet`: complete loaded tables, including players and bans.
- `summary_*.parquet`: the four post-match tables, when available.
- `selected_*.parquet`: player, world, and trooper snapshots at your review ticks.
- `in_combat.parquet`, `kill_participation.parquet`, `time_dead.parquet`, and
  `teamfights.parquet`: derived metrics when their prerequisites are available.
- `*_names.json`: the installed parser's name lookup tables.

`PASS` means the automated check passed. `FAIL` means an exception or a checked
inconsistency. `REVIEW` needs human investigation, including empty datasets or
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
- [ ] Older supported client: check absent fields and unknown name IDs explicitly.
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
  Unknown IDs need review and possibly a name-table update.
- [ ] Check `tick_to_seconds`, `tick_to_clock_time`, `tick_to_match_seconds`, and
  `tick_to_match_clock` at pregame, start, middle, and end. Include a pause.
  Demo time and the displayed game clock have different semantics; record any
  mismatch instead of adjusting expected values to make the check pass.
- [ ] Check `game_start_tick`, `pregame_seconds`, `game_over_tick`, and all
  `regulation_*` values. Regulation duration excludes paused ticks.

### Every registered dataset

Use one clear event of each kind, then check at least one boundary or repeated
event. Mark missing events **not observed**, and find another replay if needed.

| Dataset | What to compare with the replay |
| --- | --- |
| `player_ticks` | Position, orientation, health, alive/dead state, level, souls, shop/regen-zone flags, death/respawn times, counters, and recorded stat modifiers at several ticks. Check a hero swap and a respawn boundary. |
| `world_ticks` | Pause/resume state and the next-midboss timer. |
| `kills` | Victim, killer, assistants, and tick for several kills, including any unusual death. |
| `damage` | Attacker/victim, ability/weapon, damage, remaining health, crit/hitgroup, melee flags, and available shield/message fields. Check light/heavy melee and lethal overkill. Messages are not necessarily individual bullets. |
| `abilities` | Who cast which ability and when. Distinguish signature casts from movement abilities where applicable. |
| `ability_upgrades` | Which hero spent an ability point, on which ability, and the resulting tier. Older demos may have no supported upgrade data. |
| `ability_ticks` | Cooldown start/end and charge changes around a cast/recharge. This table is change-only; absence of a row on a later tick is not absence of the ability. |
| `item_purchases` | Purchase, upgrade, sell/swap events that occur; verify item and hero identity after a hero switch. |
| `chat` | Message text, sender, channel/team information, and the hero active when the message was sent. |
| `flex_slots` | Unlock tick and team. |
| `objectives` | Objective identity/team, health changes, destruction, and patron phase transitions when recorded. |
| `mid_boss` | Spawn/kill and recorded rejuvenator pickup/use/expiry events. |
| `rift` | Announcement, activation, capture **or** expiry, winning team, lane and location. A Rift still active at EOF is omitted. |
| `troopers` | One wave's team, lane, position and health; compare sampled rows with the full table. Entity indices may be reused later. |
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
  shield fields are checked only as their documented raw observations. There is
  no `demo.healing`, `demo.barriers()`, or calculated hero-attribute API.
- [ ] Check `stat_modifier_values_available` and `unknown_stat_modifier_count`.
  Never compare `stat_modifier_*` directly with final UI resistances/fire rate.
- [ ] Do not require `health <= max_health` at every transitional tick without
  checking the game; temporary health effects and replication can complicate it.

### Summary and derived metrics

- [ ] `summary()` has exactly `snapshots`, `last_hits`, `objectives`, `damage`.
  Compare final kills/deaths/assists, net worth, last hits, objectives and damage
  totals with the scoreboard and post-match screens.
- [ ] In `summary_damage`, filter `stat_type` to the quantity being checked and
  exclude category duplicates (`is_category=False`). Values are interval
  amounts; summing categories and individual sources double-counts them.
  Raw healing/regen categories remain part of this original matrix.
- [ ] `kill_participation()`: manually count kills, assists and team kills for
  one hero; test a bounded interval too. No team kills should produce a null ratio.
- [ ] `in_combat()`: inspect entry/exit around one hero hit and one NPC hit;
  compare with the recorded combat window, including pause behavior.
- [ ] `time_dead()`: verify one death/respawn interval and a player's match total;
  paused ticks and post-game time must not contribute. Missing game-over data
  means this metric is unavailable.
- [ ] `teamfights()`: inspect one fight and two simultaneous distant skirmishes.
  Review participants, time bounds, damage and kills. This is a heuristic:
  test `gap_seconds`, `radius`, and `min_players` against your intended definition.

### API paths, CLI and downloads

- [ ] Fresh property access, bulk `load()`, repeated access, and duplicate load
  requests produce consistent data. The script exercises the bulk/cache paths;
  use a fresh `Demo` to spot-check independent lazy loads below.
- [ ] Snapshot selection works for single/list ticks, windows, `every`, `seconds`,
  and event unions. Single-dataset results are frames; multiple datasets return
  a dictionary. The script compares sampled rows against full frames and checks
  the fresh versus cached event path.
- [ ] `boon --version`, `verify`, `info`, `players`, `datasets`, `show`, `summary`,
  and each supported `stats --metric` command agree with the Python results.
  Use `--json` and inspect the parsed values, not just the exit status.
- [ ] Invalid filenames, invalid dataset names, missing snapshot selectors, and
  incompatible selectors fail cleanly. Retired datasets are absent from
  `boon datasets` and rejected by `load()`/`show`/event selection.
- [ ] `boon versions`: client version, source `VersionDate`/`VersionTime`, release
  timestamp and installed status are plausible. Source dates have no inferred
  timezone. `boon versions --local` works offline.
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

# Inspect nearby messages; a kill/cast/hit need not occur at the exact selected tick.
hits = pl.scan_parquet(folder / "damage.parquet")
print(hits.filter(
    pl.col("tick").is_between(tick - 64, tick + 64)
    & ((pl.col("attacker_hero_id") == hero) | (pl.col("victim_hero_id") == hero))
).collect())

# Inspect a recorded summary statistic without double-counting categories.
matrix = pl.read_parquet(folder / "summary_damage.parquet")
print(matrix.filter((pl.col("stat_type") == "damage") & ~pl.col("is_category"))
      .group_by("dealer_player_slot").agg(pl.col("damage").sum()))
```

A fresh lazy load can be checked against the exported bulk-loaded evidence:

```python
from boon import Demo
from polars.testing import assert_frame_equal

fresh = Demo("106996573.dem")
assert_frame_equal(fresh.damage, pl.read_parquet(folder / "damage.parquet"))
```

## Turn independent observations into repeatable assertions

Transcribe a value from the replay UI, not from Boon's output. For example, if
Victor is visibly at 123 health at demo tick 50707, save this JSON in
`observations.json` (**123 is an illustration; replace it with your observation**):

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
