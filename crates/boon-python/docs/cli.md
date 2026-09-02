# 💻 CLI

Boon includes two CLI tools.

- **Python CLI:** Install `boon-deadlock` with pip or uv. The package adds
  `boon` to your PATH. This command uses the library parser and prints
  [Polars](https://pola.rs) DataFrames. See [Python CLI](#python-cli).
- **`boon-dev`:** This low-level Rust tool inspects entities, send tables,
  string tables, and raw messages. Build it from the repository. See
  [boon-dev](#boon-dev).

Both tools support `--help` for each command.

## Python CLI

Installed with the `boon-deadlock` Python package:

```bash
pip install boon-deadlock   # or: uv add boon-deadlock
```

Run `boon --help` for the full list, or `boon <command> --help` for one command.
Pass `--json` (where available) for machine-readable output.

### `info`

Match metadata: map, mode, build, duration, and result.

```bash
boon info match.dem
boon info match.dem --json
```

---

### `players`

The player roster (name, Steam ID, hero, team, start lane), with hero names resolved.

```bash
boon players match.dem
```

---

### `datasets`

List every dataset that `show` can display.

```bash
boon datasets
```

---

### `show`

Load and print any dataset as a table (or JSON). Dataset names come from
`boon datasets` (they mirror the `Demo` properties).

```bash
boon show match.dem kills --limit 20
boon show match.dem player_ticks --tail --limit 5
boon show match.dem objectives --json
```

**Options:**

| Flag | Description |
|------|-------------|
| `--limit <N>` / `-n <N>` | Max rows to show (`0` = all) |
| `--tail` | Show the last rows instead of the first |
| `--json` | Emit row-oriented JSON |

---

### `summary`

The post-match summary. Choose a part with `--part`.

```bash
boon summary match.dem
boon summary match.dem --part objectives
```

**Options:**

| Flag | Description |
|------|-------------|
| `--part <PART>` | `snapshots`, `last_hits`, `objectives`, `damage`, or `all` (default: `last_hits`) |
| `--limit <N>` / `-n <N>` | Max rows to show (`0` = all) |
| `--json` | Emit JSON |

---

### `stats`

Derived metrics from [`boon.stats`](api.md).

```bash
boon stats match.dem --metric kill-participation
boon stats match.dem -m time-dead
```

**Options:**

| Flag | Description |
|------|-------------|
| `--metric <M>` / `-m <M>` | `kill-participation`, `time-dead`, or `in-combat` |
| `--limit <N>` / `-n <N>` | Max rows to show (`0` = all) |
| `--json` | Emit JSON |

---

### `verify`

Check that a file is a valid Deadlock demo.

```bash
boon verify match.dem
```

## boon-dev

`boon-dev` is a low-level debugging CLI, built from the `boon-dev` crate. It is
**not shipped** — there are no release binaries and no crates.io package. Build
it from source when you need it:

```bash
cargo build --release -p boon-dev
# Binary is at target/release/boon-dev
```

Run `boon-dev --help` for the full list, or `boon-dev <command> --help` for the
flags on any command (most take `--filter`, `--summary`, `--limit`, and
`--min-tick` / `--max-tick`, plus the global `--json`).

### Commands

| Command | Description |
|---------|-------------|
| `verify` | Check that a file is a valid demo. |
| `info` | File header and game info (build, map, playback time, match ID, mode, winner, players). |
| `messages` | List every command/packet in the demo with metadata. |
| `classes` | The class-id → network-name mapping. |
| `send-tables` | Serializer (send table) field schemas per entity class. |
| `string-tables` | String tables from demo initialization. |
| `events` | Decoded game events (user messages); `--inspect` for full payloads. |
| `summary` | Post-match summary from the last-tick event. |
| `entities` | Entity state at a specific `--tick`. |
| `abilities` | Important ability usage events. |
| `ability-upgrades` | Hero ability-point spending (tier 1–3). |
| `ability-ticks` | Ability cooldown / charge state changes (change-only). |
| `shop-events` | Item shop transactions (purchase, upgrade, sell, swap). |
| `chat` | In-game chat messages. |
| `objectives` | Per-tick objective health (walkers, barracks, shrines, patron, mid boss). |
| `mid-boss` | Mid boss lifecycle events (spawn, kill, rejuv pickup/use/expire). |
| `troopers` | Alive lane trooper position/state per tick. |
| `neutrals` | Neutral creep state changes (change-only). |
| `stat-modifiers` | Permanent stat-bonus change events (urn / breakable pickups). |
| `active-modifiers` | Active buff/debuff modifier events (applied/removed). |

Example:

```bash
# All player controllers at tick 10000, with up to 50 fields each
boon-dev entities match.dem --tick 10000 --filter CCitadelPlayerController --fields 50

# Count event types
boon-dev events match.dem --summary
```
