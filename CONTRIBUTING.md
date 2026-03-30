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
│   ├── boon-cli/       # CLI tool
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

# Build the CLI
cargo build --release -p boon-cli
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

Ability/item and modifier IDs in demo events are MurmurHash2 hashes of their string names. The lookup tables at `crates/boon/src/abilities.rs` and `crates/boon/src/modifiers.rs` are generated from Deadlock's `abilities.vdata` and `modifiers.vdata` (extracted from the game's VPK data using [Source2Viewer](https://github.com/ValveResourceFormat/ValveResourceFormat)):

```bash
# Run from the repo root with abilities.vdata and modifiers.vdata in the working directory
cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
```

This regenerates `crates/boon/src/abilities.rs` and `crates/boon/src/modifiers.rs`.

## Release Strategy

Boon has three independent version tracks, each with its own tag pattern and CD workflow:

### Parser (`boon` + `boon-proto`) &mdash; `boon-v*` tags

Publishes both `boon-proto` and `boon-deadlock` to crates.io (in dependency order).

```bash
# 1. Bump version in Cargo.toml [workspace.package] and [workspace.dependencies]
# 2. Update changelog

git commit -am "boon 0.2.0"
git tag boon-v0.2.0
git push origin main --tags
```

**Secret required:** `CARGO_REGISTRY_TOKEN`

### CLI (`boon-cli`) &mdash; `boon-cli-v*` tags

Cross-compiles for Linux x86_64, macOS x86_64/aarch64, and Windows, then creates a GitHub Release with the binaries.

```bash
# 1. Bump version in crates/boon-cli/Cargo.toml
# 2. Update changelog

git commit -am "boon-cli 0.2.0"
git tag boon-cli-v0.2.0
git push origin main --tags
```

### Python (`boon-python`) &mdash; `boon-python-v*` tags

Builds wheels for Linux x86_64/aarch64, macOS x86_64/aarch64, and Windows, then publishes to PyPI via trusted publishing.

```bash
# 1. Bump version in crates/boon-python/Cargo.toml (pyproject.toml reads it automatically)
# 2. Bump version in crates/boon-python/docs/conf.py
# 3. Update changelog

git commit -am "boon-python 0.2.0"
git tag boon-python-v0.2.0
git push origin main --tags
```

**Setup required:** Configure [trusted publishing](https://docs.pypi.org/trusted-publishers/) on PyPI and create a `pypi` environment in GitHub repo settings.

### Notes

- Version bumps are manual &mdash; workflows verify the tag matches the version but don't bump it for you.
- Tracks are independent &mdash; you can release the parser without releasing the CLI or Python package.
- If the CLI or Python crate depends on a new parser feature, publish the parser first.

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
