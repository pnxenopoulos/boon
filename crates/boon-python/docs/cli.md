# CLI

Boon includes a command-line tool for inspecting demo files. It is built from the
`boon` crate.

## Building

```bash
cd boon
cargo build --release -p boon
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
