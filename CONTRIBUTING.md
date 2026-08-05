# Contributing to Boon

Thanks for your interest in contributing to Boon! This guide covers setup, coding standards, and how to submit changes.

## Prerequisites

- **Rust** (stable) &mdash; install via [rustup](https://rustup.rs)
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

Before submitting a PR, make sure all checks pass locally. CI runs these same checks:

```bash
# Formatting
cargo fmt --all -- --check

# Linting
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests
cargo nextest run --workspace --all-features
```

## Updating Protobuf Definitions

When Valve updates Deadlock's protobuf definitions, sync and regenerate:

```bash
# 1. Fetch the latest .proto files from SteamDB
./scripts/sync-protos.sh

# 2. Regenerate Rust code from the .proto files
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

This updates the files under `crates/boon-proto/proto/` and regenerates `crates/boon-proto/src/proto.rs`.

## Updating the Name Lookup Tables

Ability/item and modifier IDs in demo events are MurmurHash2 hashes of their string names. The lookup tables at `crates/boon/src/abilities.rs` and `crates/boon/src/modifiers.rs` are generated from Deadlock's `abilities.vdata` and `modifiers.vdata`.

```bash
# Fetch the latest vdata files from SteamDB and regenerate the tables
./scripts/sync-name-tables.sh
```

This regenerates `crates/boon/src/abilities.rs` and `crates/boon/src/modifiers.rs`.

If you already have `abilities.vdata` and `modifiers.vdata` locally (e.g., extracted from the game's VPK data using [Source2Viewer](https://github.com/ValveResourceFormat/ValveResourceFormat)), you can skip the fetch step and run the generator directly:

```bash
# Run from the repo root with abilities.vdata and modifiers.vdata in the working directory
cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
```

## Release Strategy

Boon has three independent release tracks. All releases are started manually
from the **Release Boon** workflow on the `main` branch; maintainers do not
create or push release tags themselves. The workflow verifies the selected
version, uploads it to the relevant package index, and only then creates the
tag and GitHub Release.

| Workflow selection | Package index | Tag |
| --- | --- | --- |
| `boon-proto` | crates.io (`boon-proto`) | `boon-proto-v<version>` |
| `boon` | crates.io (`boon-deadlock`) | `boon-v<version>` |
| `boon-python` | PyPI (`boon-deadlock`) | `boon-python-v<version>` |

For a coordinated release, run the workflow three times in this order:

1. `boon-proto`
2. `boon`
3. `boon-python`

This order is enforced. A `boon` release requires the repository's exact
`boon-proto` version to exist on crates.io, and a `boon-python` release
requires the exact `boon-deadlock` version to exist there.

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

Publishing is idempotent: rerunning a partially completed release skips an
identical version already present on crates.io or PyPI. A tag that already
points at the release commit is also a no-op; the workflow refuses to move a tag
that points anywhere else.

The workflow uses trusted publishing through the GitHub environment `release`.
Configure both crates.io packages and the PyPI project to trust
`.github/workflows/release.yml` with that environment. Optional hand-written
GitHub release notes can be stored at
`.github/release-notes/<tag>.md`; otherwise GitHub generates them.

`boon-dev` is not released: it has no crates.io package or release binary and
is built locally with `cargo build --release -p boon-dev`.

## Test Fixtures

Demo files (`.dem`) are not checked into this repository (they are gitignored). They are hosted as GitHub releases in [pnxenopoulos/boon-fixtures](https://github.com/pnxenopoulos/boon-fixtures).

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
  --notes "Description of the fixture (e.g., game mode, notable properties)"
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

1. Fork the repository and create a feature branch from `main`
2. Make your changes, keeping commits focused and descriptive
3. Ensure `cargo fmt`, `cargo clippy`, and tests all pass
4. Open a pull request against `main` with a clear description of what changed and why

## Reporting Issues

Open an issue on [GitHub](https://github.com/pnxenopoulos/boon/issues). For bug reports, include:

- Boon version / commit hash
- Steps to reproduce
- Expected vs actual behavior
- Demo file match ID (if applicable)
