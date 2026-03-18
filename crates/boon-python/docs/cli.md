# CLI

Boon includes a command-line tool for inspecting demo files. It is built from the
`boon` crate.

## Installation

Install a prebuilt binary via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
(no compilation needed):

```bash
cargo binstall boon-cli
```

Or download a binary from the
[GitHub Releases](https://github.com/pnxenopoulos/boon/releases) page.

Or build from source (requires Rust):

```bash
cd boon
cargo build --release -p boon-cli
# Binary is at target/release/boon
```

## Commands

### `verify`

Check that a file is a valid demo.

```bash
boon verify match.dem
```

---

### `info`

Display file header and game information: build number, map, playback time, match ID,
game mode, winner, and player list.

```bash
boon info match.dem
```

---

### `messages`

List all commands in the demo file with metadata.

```bash
boon messages match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--cmd <TYPE>` | Filter by command type (substring match) |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |
| `--min-size <BYTES>` | Minimum message size |
| `--max-size <BYTES>` | Maximum message size |
| `--limit <N>` | Maximum messages to display |

**Example:**

```bash
# Show only full packets
boon messages match.dem --cmd FullPacket

# Show messages in a tick range
boon messages match.dem --min-tick 1000 --max-tick 2000
```

---

### `classes`

Display the class ID to network name mapping.

```bash
boon classes match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by class name (substring) |
| `--limit <N>` | Maximum classes to display |

**Example:**

```bash
# Find all Citadel player-related classes
boon classes match.dem --filter Player
```

---

### `send-tables`

Display serializer (send table) definitions — the field schemas for each entity class.

```bash
boon send-tables match.dem --summary
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by serializer name (substring) |
| `--summary` | Show only names and field counts |
| `--limit <N>` | Maximum serializers to display |

**Example:**

```bash
# See all fields on the player pawn
boon send-tables match.dem --filter CCitadelPlayerPawn
```

---

### `string-tables`

Display string tables from the demo initialization.

```bash
boon string-tables match.dem --summary
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by table name (substring) |
| `--summary` | Show only names and entry counts |
| `--limit <N>` | Maximum tables to display |

**Example:**

```bash
# Inspect instance baselines
boon string-tables match.dem --filter instancebaseline
```

---

### `events`

List decoded game events from a demo file (user messages parsed from embedded packets).

```bash
boon events match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter events by name (substring match) |
| `--summary` | Show only event names and counts |
| `--tick <TICK>` | Maximum tick to parse up to |
| `--limit <N>` | Maximum events to display |
| `--inspect` | Decode and display full message contents |

**Example:**

```bash
# Count all event types
boon events match.dem --summary

# Show only kill events
boon events match.dem --filter HeroKilled

# Inspect full message payloads for damage events
boon events match.dem --filter Damage --inspect --limit 5
```

---

### `summary`

Print a post-match summary extracted from the last-tick game event, including match
overview, player stats with gold breakdowns, objectives, mid boss kills, and damage
matrix info.

```bash
boon summary match.dem
```

---

### `entities`

Inspect entity state at a specific game tick.

```bash
boon entities match.dem --tick 10000 --summary
```

**Options:**

| Flag | Description |
|------|-------------|
| `--tick <TICK>` | **(required)** Game tick to parse to |
| `--filter <NAME>` | Filter by class name (substring) |
| `--summary` | Show only class names and counts |
| `--fields <N>` | Max fields per entity (default: 20) |
| `--limit <N>` | Max entities to display |

**Example:**

```bash
# Show all player controllers with full fields
boon entities match.dem --tick 10000 --filter CCitadelPlayerController --fields 50

# Count all entity types at a given tick
boon entities match.dem --tick 10000 --summary
```

---

### `abilities`

List important ability usage events from a demo.

```bash
boon abilities match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter abilities by name (substring) |
| `--summary` | Show only ability names and counts |
| `--limit <N>` | Maximum abilities to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `ability-upgrades`

List hero ability point spending events (skill tier upgrades).

```bash
boon ability-upgrades match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter abilities by name (substring) |
| `--summary` | Show only ability names and counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `shop-events`

List item shop transactions (purchases, upgrades, sells, swaps).

```bash
boon shop-events match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by ability name or change type (substring) |
| `--summary` | Show only ability+change combos and counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `chat`

List in-game chat messages.

```bash
boon chat match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <TEXT>` | Filter by text or chat type (substring) |
| `--summary` | Show only hero message counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `objectives`

Track per-tick objective entity health (walkers, titans, barracks, mid boss).

```bash
boon objectives match.dem --summary
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <TYPE>` | Filter by objective type (substring: walker, titan, barracks, mid_boss) |
| `--summary` | Show only objective type/team/lane counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `boss-kills`

List objective destruction events.

```bash
boon boss-kills match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <CLASS>` | Filter by entity class (substring) |
| `--summary` | Show only entity class counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `mid-boss`

List mid boss lifecycle events (spawn, kill, rejuvenator buff pickup/use/expire).

```bash
boon mid-boss match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <EVENT>` | Filter by event type (substring) |
| `--summary` | Show only event type counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `troopers`

Track alive lane trooper position and state per tick. Includes `CNPC_Trooper` and
`CNPC_TrooperBoss` entities.

```bash
boon troopers match.dem --limit 20
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <TYPE>` | Filter by trooper type (substring: trooper, trooper_boss) |
| `--summary` | Show only trooper type/team/lane counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `neutrals`

Track neutral creep state changes. Only emits rows when state changes (health, position),
significantly reducing output compared to per-tick tracking.

```bash
boon neutrals match.dem --summary
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <TYPE>` | Filter by neutral type (substring: neutral, neutral_node_mover) |
| `--summary` | Show only neutral type/team counts |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `stat-modifiers`

Track per-player cumulative permanent stat bonuses (idol and breakable pickups).

```bash
boon stat-modifiers match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by stat name (substring) |
| `--summary` | Show per-hero final stat values |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |

---

### `active-modifiers`

Track active buff/debuff modifiers on players (applied/removed events).

```bash
boon active-modifiers match.dem
```

**Options:**

| Flag | Description |
|------|-------------|
| `--filter <NAME>` | Filter by modifier or ability name (substring) |
| `--summary` | Show applied event counts per ability per hero |
| `--limit <N>` | Maximum entries to display |
| `--tick <TICK>` | Filter by exact tick |
| `--min-tick <TICK>` | Minimum tick |
| `--max-tick <TICK>` | Maximum tick |
