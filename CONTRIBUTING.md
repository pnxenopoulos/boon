# Contributing to Boon

Thank you for your interest in Boon. This guide explains how to set up the project, test changes, and submit changes.

## Prerequisites

- **Rust** (stable) &mdash; install with [rustup](https://rustup.rs)
- **Python 3.11–3.14** &mdash; for the Python bindings
- **cargo-nextest** &mdash; `cargo install cargo-nextest --locked`
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
│   └── build-protos/                   # Regenerate Rust code from .proto files
└── .github/workflows/ci.yml    # CI pipeline
```

## Getting Started

```bash
git clone https://github.com/pnxenopoulos/boon.git
cd boon

# Build everything
cargo build --workspace

# Run tests
cargo nextest run --workspace --all-features --locked --exclude boon-python

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
cargo nextest run --workspace --all-features --locked --exclude boon-python
```

### Rust Conventions

Shared dependency versions and lint settings are in the workspace `Cargo.toml`.
Each crate inherits those settings. Enable specific Clippy lints when they fit
this codebase. Do not enable all pedantic or nursery lints at once.

- Parse public string inputs into typed values before internal dispatch.
- Return errors for failed input or data operations. Reserve `expect` for
  documented invariants that indicate a programming error.
- Borrow data used only for inspection or serialization. Clone when independent ownership is necessary
  for the caller.
- Preserve absent fields when applying partial replay updates.
- Document unsafe operations and keep their scope small.
- Keep examples fallible with `?`, and test behavior that a refactor could change.

### Python Checks

From `crates/boon-python`, run:

```bash
uv sync --locked --no-install-project --group quality --group docs
uv run --no-sync ruff check python/boon
uv run --no-sync ty check python/boon
uv run --no-sync sphinx-build -W -b html docs docs/_build/html
```

CI uses Ruff and ty for checks of the Python package. The quality dependency group
pins their versions. Update these pins together after a review of new diagnostics.
Use `uv run --no-sync pytest tests/` after building the extension or installing
a wheel. This prevents uv from replacing the build under test.

CI builds Linux wheels for x86-64 and ARM64 with Python 3.11–3.14.
ARM64 builds use `ubuntu-24.04-arm`. CI and releases use native manylinux2014
containers for these builds. The Python test jobs use the x86-64 wheels.

## Writing Style

Use [ASD-STE100](https://www.asd-ste100.org/) as the writing target for maintained documentation, API text,
command help, and code comments. A plain-language review alone does not establish
full compliance. Do a check of approved words, meanings, and technical terms before
claiming compliance with the standard.

- Use active voice.
- Put one idea in each sentence.
- Limit descriptive sentences to 25 words. Limit procedural sentences to 20 words.
- Use the same term for the same thing.
- Do not use contractions.
- Do not use a vague word such as "this" without a clear noun.
- Put behavior and its reason in separate sentences.
- Keep exact API names, game field names, and Source 2 terms.

Use these technical terms consistently:

| Term | Meaning |
| --- | --- |
| demo | A recorded Deadlock match in a `.dem` file |
| dataset | A named set of parsed records returned as a DataFrame |
| tick | A numbered step in a demo |
| snapshot | Recorded state at a selected tick or post-match sample |
| catalog | A boon-data JSON file with names and game definitions |
| client version | The Deadlock `ClientVersion`, separate from Boon's package version |
| display name | A localized label, separate from an internal game name |

Keep exact API identifiers and technical names. Do not replace them with
ordinary words. Do not edit generated files or upstream protobuf text solely
to change the writing style.

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

## Updating Name Data

Hero, ability/item, modifier, and breakable names are read from boon-data releases at runtime.
Use `boon get` to install the latest catalogs. The lookup functions select the
newest local client version or download latest when none is installed.
Rust callers use `CatalogNames::load`; Python functions accept `version=`.
Update the boon-data pipeline when the source catalog format changes.

Breakable subclass names use `misc_id` and `misc_name` from `misc.json` records
whose `definition._class` is `citadel_breakable_prop`. Boon has no VData name-table
generator or embedded breakable table.

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
for match in 108575009 109108139 100655353; do
  gh release download "$match" --repo pnxenopoulos/boon-fixtures \
    --dir crates/boon-python/tests/fixtures/
done
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
    return get_demo(FIXTURE_PATH)
```

4. Update CI to download the new fixture.

### Current fixtures

| Match ID | Game Mode | Description |
|----------|-----------|-------------|
| 108575009 | 6v6 | Current-format API tests, scoreboard and fight-state checks |
| 109108139 | Street Brawl | Mode-specific regression checks only |
| 100655353 | 6v6 | Silver-to-Victor hero-swap regression only |

General tests use `108575009.dem`. Use `109108139.dem` for Street Brawl.
The older hero-swap fixture tests one regression; it does not establish support
for old formats.

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

## Performance investigations

See [the benchmark guide](crates/boon-python/benchmarks/README.md) for Python
API workloads, Rust parser phases, before/after comparisons and profiling.
Build optimized binaries, keep the benchmark harness identical across revisions,
and compare output fingerprints as well as timing. CI smoke-tests the harness;
use repeated measurements on an idle machine for performance decisions.
