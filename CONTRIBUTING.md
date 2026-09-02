# Contributing to Boon

Thank you for your interest in Boon. This guide explains how to set up the project, test changes, and submit changes.

## Prerequisites

- **Rust** (stable) &mdash; install with [rustup](https://rustup.rs)
- **Python 3.11+** &mdash; for the Python bindings
- **maturin** &mdash; `pip install maturin` (or `uv add maturin`)

## Repository Structure

```
boon/
├── crates/
│   ├── boon/           # Core parser library (Rust)
│   ├── boon-dev/       # Low-level dev / debug CLI (in-repo only, not published)
│   ├── boon-proto/     # Auto-generated protobuf definitions
│   └── boon-python/    # Python bindings (PyO3 + pyo3-polars)
├── scripts/
│   ├── sync-protos.sh                  # Fetch latest Deadlock .proto files
│   ├── build-protos/                   # Regenerate Rust code from .proto files
│   └── generate-name-tables/           # Regenerate ability and modifier name lookup tables
└── .github/workflows/ci.yml    # CI pipeline
```

## Getting Started

```bash
git clone https://github.com/pnxenopoulos/boon.git
cd boon

# Build everything
cargo build --workspace

# Run tests
cargo nextest run --workspace --all-features

# Build the dev / debug CLI
cargo build --release -p boon-dev
```

### Python Development

```bash
cd crates/boon-python

# Using pip + maturin
pip install maturin
maturin develop --release

# Using uv
uv sync
uv run maturin develop --release
```

## Code Quality

Run these checks before you submit a pull request. CI runs the same checks.

```bash
# Formatting
cargo fmt --all -- --check

# Linting
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests
cargo nextest run --workspace --all-features
```

## Writing Style

Use ASD-STE100 English where practical. Apply this rule to maintained
documentation, API text, command help, and code comments.

- Use active voice.
- Put one main idea in each sentence.
- Keep sentences short. Use no more than 25 words when practical.
- Use the same term for the same thing.
- Do not use contractions.
- Do not use a vague word such as "this" without a clear noun.
- Put behavior and its reason in separate sentences.
- Keep exact API names, game field names, and Source 2 terms.

Do not edit generated files or upstream protobuf text to change the writing
style. Edit the generator or source text when possible.

## Updating Protobuf Definitions

When Valve updates Deadlock's protobuf definitions, sync and regenerate:

```bash
# 1. Fetch the latest .proto files from SteamDB
./scripts/sync-protos.sh

# 2. Regenerate Rust code from the .proto files
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

The command updates the files in `crates/boon-proto/proto/`. It also regenerates
`crates/boon-proto/src/proto.rs`.

## Updating the Name Lookup Tables

Ability, item, modifier, and breakable subclass IDs are MurmurHash2 hashes of
internal names. The generator joins four Deadlock VData files to the English
hero and item localization catalogs. The VData files are `abilities.vdata`,
`modifiers.vdata`, `heroes.vdata`, and `misc.vdata`. The generator creates token
lookups, display names, resistance inputs, and stat-effect metadata.

```bash
# Fetch the latest vdata files from SteamDB and regenerate the tables
./scripts/sync-name-tables.sh
```

This regenerates `abilities.rs`, `ability_display_names.rs`, `breakables.rs`,
`modifiers.rs`, `resistances.rs`, and `stat_catalog.rs` under
`crates/boon/src/`.

If you already have those VData and localization inputs locally (for example,
after extracting the game's VPK data with
[Source2Viewer](https://github.com/ValveResourceFormat/ValveResourceFormat)),
place all six files in the repository root and run the generator directly:

```bash
# Run from the repo root with the four VData and two localization files present
cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
```

## Release Strategy

Boon has three independent release tracks. Start each release manually from the
**Release Boon** workflow on the `main` branch. Do not create or push a release
tag. The workflow verifies the selected version and uploads the package. After
the upload succeeds, the workflow creates the tag and GitHub Release.

| Workflow selection | Package index | Tag |
| --- | --- | --- |
| `boon-proto` | crates.io (`boon-proto`) | `boon-proto-v<version>` |
| `boon` | crates.io (`boon-deadlock`) | `boon-v<version>` |
| `boon-python` | PyPI (`boon-deadlock`) | `boon-python-v<version>` |

For a coordinated release, run the workflow to completion three times in this
order:

1. `boon-proto`
2. `boon`
3. `boon-python`

The workflow enforces this order. Wait until each upload is visible before you
start the next release. A `boon` release requires the exact `boon-proto` version
on crates.io. A `boon-python` release requires the exact `boon-deadlock` version
on crates.io.

Before dispatching a release:

1. Bump the selected package version and update the changelog.
   - `boon-proto` uses its build-derived version in
     `crates/boon-proto/Cargo.toml`.
   - `boon` uses `[workspace.package].version` and the `boon` entry under
     `[workspace.dependencies]` in the root `Cargo.toml`.
   - `boon-python` uses `crates/boon-python/Cargo.toml`; the documentation
     reads this value automatically.
2. Merge the version bump into `main` and wait for **CI Check** to pass on that
   exact commit.
3. Open **Actions > Release Boon > Run workflow**, select the component, enter
   its version without a leading `v`, and dispatch it from `main`.

You can run a partially completed release again. The workflow skips an identical
version that is already on crates.io or PyPI. It also keeps a tag that points to
the release commit. The workflow does not move a tag that points to a different
commit.

`boon-dev` does not have a release package or binary. Build it locally with
`cargo build --release -p boon-dev`.

## Test Fixtures

The repository does not contain demo files (`.dem`). Download them from the
[boon-fixtures releases](https://github.com/pnxenopoulos/boon-fixtures).

### Downloading fixtures

Each fixture is a named release whose tag is the match ID:

```bash
gh release download 70555151 \
  --repo pnxenopoulos/boon-fixtures \
  --dir crates/boon-python/tests/fixtures/

gh release download 70537442 \
  --repo pnxenopoulos/boon-fixtures \
  --dir crates/boon-python/tests/fixtures/
```

Tests that require a missing fixture are skipped automatically.

### Adding a new fixture

1. Place the `.dem` file in `crates/boon-python/tests/fixtures/` locally.
2. Create a release in [boon-fixtures](https://github.com/pnxenopoulos/boon-fixtures):

```bash
gh release create <match_id> \
  crates/boon-python/tests/fixtures/<match_id>.dem \
  --repo pnxenopoulos/boon-fixtures \
  --title "<match_id>.dem" \
  --notes "Description of the fixture (for example, game mode, notable properties)"
```

3. Add fixture-specific tests in `crates/boon-python/tests/test_<match_id>.py` with a skip guard:

```python
FIXTURE_PATH = FIXTURES_DIR / "<match_id>.dem"

@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("<match_id>.dem fixture not available")
    d = Demo(str(FIXTURE_PATH))
    d.load(*ALL_DATASETS)
    return d
```

4. Update CI to download the new fixture.

### Current fixtures

| Match ID | Game Mode | Description |
|----------|-----------|-------------|
| 70555151 | 6v6 | Standard 6v6 match |
| 70537442 | Street Brawl | Street brawl (game_mode=4) match |

## Submitting Changes

1. Fork the repository and create a feature branch from `main`.
2. Make the changes. Keep each commit focused and descriptive.
3. Run `cargo fmt`, `cargo clippy`, and the tests.
4. Open a pull request against `main`. Describe the change and its reason.

## Reporting Issues

Open an issue on [GitHub](https://github.com/pnxenopoulos/boon/issues). Include
this information in a bug report:

- Boon version or commit hash.
- Steps to reproduce the problem.
- Expected behavior and actual behavior.
- Demo match ID, if applicable.
