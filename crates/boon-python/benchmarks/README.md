# Boon performance benchmarks

Use three layers to decide where to optimize:

| Layer | Measures | Entry point |
| --- | --- | --- |
| Python API | Actual datasets, grouped loads, snapshots, derived metrics, catalog reads | `bench_parse.py` |
| Rust parser | Preparation, event scans, class-filtered decode, fresh/prepared seeks | `cargo bench -p boon-deadlock --bench parse` |
| Developer CLI | Parsing, catalog verification and command formatting together | `cargo bench -p boon-dev --bench cli` |

The sibling pbdems2 benchmark suite covers bit readers, field paths, serializers
and entity decoding. Profile an expensive Boon workload before choosing which
of those microbenchmarks to optimize. See [RESULTS.md](RESULTS.md) for a measured
Boon baseline and its limits, and [SNAPSHOT_RESULTS.md](SNAPSHOT_RESULTS.md) for
the snapshot allocation and preload comparison.

## Build and measure the Python API

From `crates/boon-python/`, build an optimized extension in your environment:

```bash
uv run maturin develop --release --locked
uv run --no-sync python benchmarks/bench_parse.py --list

uv run --no-sync python benchmarks/bench_parse.py \
  --demo ../../103129247.dem --repeat 5 --warmup 1 \
  --threads 4 --segments 4 --output ../../target/benchmarks/baseline.json
```

`--build-profile` records your declared build profile; it does not change or
verify compiler options. Always build first. Use `--build-profile debug` only
for smoke tests. `--no-sync` prevents uv from replacing the measured release
extension with an editable development build. The report records the actual
native library SHA-256, package versions, Git revision/diff, harness SHA-256,
demo SHA-256, OS, architecture, CPU model/count, host, and thread settings.

Pass `--demo` multiple times for a corpus. Inputs are explicit: a missing file
is an error, not a skipped benchmark or a silently substituted smaller demo.
The default cases cover construction, combat/economy groups, ability state,
sparse/periodic snapshots, cached combat access, and post-match summary.
Use `--case NAME` repeatedly to narrow the run. `--all` includes every currently
registered dataset, full player/trooper frames, all derived metrics, and name
lookups. Full frames can use several GiB of RAM. Street Brawl cases are marked
skipped on standard matches; other failures produce a nonzero exit code.

```bash
# Shared serial pass versus separate property access versus cached access.
uv run --no-sync python benchmarks/bench_parse.py --demo ../../103129247.dem \
  --case load/combat --case sequential/combat --case cached/combat \
  --output ../../target/benchmarks/combat.json

# Full extraction, targeted seeks, and parallel sampling.
uv run --no-sync python benchmarks/bench_parse.py --demo ../../103129247.dem \
  --case dataset/player_ticks --case snapshot/single --case snapshot/many \
  --case snapshot/sampled --case snapshot/kills --segments 1 \
  --output ../../target/benchmarks/snapshots-serial.json

# Uses local installed catalogs; prepare them before timing.
boon get
uv run --no-sync python benchmarks/bench_parse.py --demo ../../103129247.dem \
  --case names/cold --case names/warm --case dataset/breakables \
  --output ../../target/benchmarks/names.json
```

`names/cold` clears parsed-map caches before every sample. `names/warm` fills
those caches outside the timer. Both include the public functions' normal local
file verification and returned dictionary copies. Each calls all five catalog
name functions. Network requests are blocked in every benchmark worker;
missing catalogs fail with installation instructions. Catalog fingerprints are
recorded for comparisons. No benchmark silently downloads data.

## What the numbers mean

Each case runs in its own subprocess. Samples run sequentially, with a fresh
`Demo(preload=False)` for each warmup and repetition. `construct` measures this
lightweight construction; `construct/combat` measures construction with combat
preloading, including its parse time. For revisions predating the `preload`
argument, the harness explicitly calls the same grouped `load()` to measure that
workload. `cached/*` explicitly preloads the same
requested group outside the timer. Cache hits are never inferred from a timing
threshold. `sequential/*` uses one fresh Demo and accesses the group in order;
`load/*` requests the group together. They may share different amounts of work.

- `init_seconds`: construction/open/verification and initial metadata parsing.
- `seconds`: the requested operation, excluding construction, explicit cached
  setup, output inspection, garbage collection and destruction. For `construct` and `construct/combat`,
  this is the constructor time instead.
- `cpu_seconds`: CPU across the process's threads during that operation.
- `process_peak_rss_bytes`: entire worker high-water RSS, including imports,
  warmup, construction, mapped pages, output inspection and allocator retention.
  Linux/macOS support this metric; other platforms report null. It is **not**
  the allocation size of the dataset or a per-sample RSS delta.
- `output_bytes`: Polars' estimated size of returned frame data; dictionary
  and Python overhead are not included.

Inputs are streamed through the OS page cache before timing. These are warm
file-cache measurements, not disk benchmarks. Initialization APIs share setup,
so their measurements are not additive. Derived metrics include acquisition of
their input datasets. `snapshot/many` selects eight ticks (beyond the four-tick
seek fast path); `sparse` selects three; `window` selects 65 adjacent ticks.

Every repetition verifies schemas, row counts, tick bounds, value hash sums,
and position-sensitive hash sums outside the timer. Ordering is hashed from the
row-hash column plus row indices, avoiding an index-induced copy of wide frames.
For `stat_modifier_events`,
Rust HashMap iteration leaves simultaneous rows unordered: the checker requires
chronological ticks, then sorts those rows before hashing. This normalization is
outside timing and does not change the returned dataset. Other datasets retain
order-sensitive checks. Empty frames remain visible
as zero rows and do not establish useful coverage. Hashes are regression guards,
not collision-free correctness proofs; retain fixture tests and manual replay
checks. Compare using the same Polars version.

JSON is saved after each case, including failures. A per-case timeout defaults
to 1,800 seconds and can be set with `--timeout`. Reports retain all measured
samples plus median, minimum, maximum and sample standard deviation. Inspect
variation; do not infer a p95 from five runs.

## Compare revisions

Measure both builds with **the same harness**, fixture contents, settings,
Python/Polars versions and machine. Run an idle machine with consistent power
settings; never compile, test or run another benchmark concurrently. Record
several alternating baseline/candidate runs for a serious optimization decision.

```bash
uv run --no-sync python benchmarks/compare.py \
  ../../target/benchmarks/baseline.json ../../target/benchmarks/candidate.json
```

The comparison refuses changed outputs, catalogs, cases, harnesses or environments.
A negative percentage means faster. `--allow-config-change` permits deliberate
segment/thread or build-profile experiments, while retaining output/catalog
checks. It does not turn a changed dataset into a valid optimization.

## Rust phases and CPU profiling

From the repository root:

```bash
export BOON_BENCH_DEMO="$PWD/103129247.dem"
cargo bench -p boon-deadlock --bench parse --locked -- --save-baseline before
# After rebuilding the candidate with the same benchmark harness:
cargo bench -p boon-deadlock --bench parse --locked -- --baseline before

# Fresh preparation versus reuse of the signon/index.
cargo bench -p boon-deadlock --bench parse --locked -- seek
# Execute each case once as a functional smoke test.
cargo bench -p boon-deadlock --bench parse --locked -- --test

# cargo-criterion can also maintain history and emit machine-readable results.
cargo criterion -p boon-deadlock --bench parse --locked \
  --message-format=json --history-id before > target/benchmarks/rust-before.jsonl
```

Criterion IDs include the demo filename. Keep its contents fixed (record the
SHA-256) and use separate histories for different demos. Rust decode/init
benchmarks use owned input with one large input per batch; the clone is setup,
while parser/result teardown is included in these original phase timers. Seek
benchmarks exclude mapping and result teardown, and compare fresh preparation
with a reused prepared parser. `parse_send_tables` and `parse_class_info` both
pay shared signon/index preparation on fresh parsers; they are API costs rather
than disjoint internal stages.

`boon-dev` CLI benchmarks may acquire catalogs once before timing; install them
first with `boon get` for an offline run. They retain ordinary per-command local
verification and formatting costs. They do not time process startup.

For attribution, use a symbols build and a separate CPU profile. Do not compare
instrumented timings with ordinary release runs:

```bash
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --locked \
  -p boon-deadlock --example profile_decode
BOON_PROFILE_ITERS=3 perf record -g --call-graph dwarf -- \
  target/release/examples/profile_decode
perf report
```

For Python API profiles, sample the child processes too (for example, `perf
record --inherit` on Linux). JSON `work_start_ns` / `work_end_ns` identify the
operation windows; preloads, hashing and setup otherwise appear in the same
process profile. The pbdems2 real-world allocation harness can investigate
allocations separately; its instrumentation is not included in these timings.

CI runs helper tests and a short Python API smoke run on Python 3.13. It checks
that the harness executes, without asserting timings on shared runners.
