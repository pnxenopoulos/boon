# Initial performance findings

Historical Boon 0.10.0 measurements. These reports do not describe current-format demos or the current release.

These measurements precede snapshot buffer and preload changes. See
[the snapshot comparison](SNAPSHOT_RESULTS.md) for the subsequent experiment.

Measured 2026-09-26 UTC (September 25 locally), using the release extension from
Boon 0.10.0 at commit `49c9c4612e531085cdcf173d3eeac5bd6a91f60a`, with the benchmark
changes in the working tree. This is an exploratory baseline, not a performance
promise or a statistically controlled optimization result.

## Environment and coverage

- AMD Ryzen 7 7800X3D, 16 logical CPUs; WSL2 Linux, about 15.2 GiB VM RAM;
  Python 3.13.14 and Polars 1.38.0.
- Four Rayon/Polars threads and four snapshot segments; one untimed warmup and
  three measured repetitions per case, run sequentially without concurrent builds
  or tests. Input files were warmed in the OS page cache.
- Locally installed boon-data 6698; network access blocked in Python workers.
- `97104148.dem`: Street Brawl, 119,611,721 bytes, 57,513 ticks, build 10854.
- `103129247.dem`: standard match, 458,141,549 bytes, 130,852 ticks, build 10854.
- 47 cases per demo: **92 passed**, with two expected Street Brawl-only skips on
  the standard match. Every registered dataset had nonempty coverage on at least
  one fixture. Repeated outputs matched; grouped, sequential and cached group
  outputs also matched one another.

The demo SHA-256 values are:

```text
97104148.dem  492b2f2827a604e9f689f4611b7d7155f996eadce42fde14e38de5721adc5042
103129247.dem 608316f569eda999e334adfb77b93cc74550c61fc23e9a7bcdbf805abd7ed76f
```

## Python API baseline

Median seconds, excluding construction unless the row is `construct`:

| Case | Street Brawl | Standard |
| --- | ---: | ---: |
| `construct` | 0.017254 | 0.028902 |
| `load/combat` | 0.539771 | 1.909900 |
| `sequential/combat` | 1.059475 | 4.253698 |
| `cached/combat` | 0.000292 | 0.000261 |
| `load/economy` | 0.458225 | 1.864012 |
| `sequential/economy` | 0.464452 | 1.906724 |
| `load/mixed` | 0.936556 | 3.385625 |
| `sequential/mixed` | 1.625034 | 5.956732 |
| `dataset/ability_ticks` | 0.539131 | 2.076776 |
| `dataset/player_ticks` | 0.394381 | 1.276175 |
| `dataset/troopers` | 0.297323 | 1.289639 |
| `snapshot/single` | 0.031306 | 0.021681 |
| `snapshot/sparse` | 0.086311 | 0.129993 |
| `snapshot/many` | 0.188073 | 0.642593 |
| `snapshot/sampled` | 0.191721 | 0.665837 |
| `derived/teamfights` | 0.888080 | 3.425244 |
| `names/cold` | 0.063946 | 0.062413 |
| `names/warm` | 0.054685 | 0.052757 |

`combat` requests kills, damage and abilities; `economy` requests chat, purchases
and upgrades; `mixed` requests player/world ticks, kills and damage. Cached
access excludes its preload. Each names case calls all five public name-map
functions, so its time is not the cost of a single dictionary lookup.

Three repetitions are enough to locate large costs, not small regressions.
For example, standard-demo trooper extraction had an 8% sample standard deviation
relative to its median. Use longer, alternating runs before accepting a speedup.

## Segment scaling

Same standard demo, release extension and thread limits. The four-segment samples
are taken from the full baseline above; the one-segment run was measured afterward.
The comparison tool verified identical output fingerprints, including row order.

| Case | One segment | Four segments | Wall-time reduction |
| --- | ---: | ---: | ---: |
| `dataset/player_ticks` | 3.198 s | 1.276 s | 60.1% |
| `snapshot/sampled` | 2.235 s | 0.666 s | 70.2% |

## Memory and CPU

Standard demo, four segments. Peak RSS is the **whole worker's high-water mark**,
including imports, mapped input, warmup, output checking and allocator retention;
it is not dataset allocations. Returned frame size is Polars' estimate.

| Case | Wall time | Process CPU time | Peak RSS | Returned frames |
| --- | ---: | ---: | ---: | ---: |
| `load/combat` | 1.910 s | 1.910 s | 535 MiB | 5.6 MiB |
| `dataset/ability_ticks` | 2.077 s | 2.077 s | 509 MiB | 0.5 MiB |
| `dataset/player_ticks` | 1.276 s | 3.719 s | 1233 MiB | 391.3 MiB |
| `dataset/troopers` | 1.290 s | 3.873 s | 1354 MiB | 236.4 MiB |
| `load/mixed` | 3.386 s | 5.870 s | 1306 MiB | 397.9 MiB |
| `snapshot/sampled` | 0.666 s | 2.507 s | 562 MiB | 6.1 MiB |
| `derived/teamfights` | 3.425 s | 6.597 s | 612 MiB | 0.0 MiB |

## Rust seek baseline

Criterion estimates for the standard demo, ten samples per case with a one-second
warmup and at least five seconds of measurement. Fresh seeks create a parser in
untimed setup and include signon/index preparation; prepared seeks reuse those
structures. Both decode to the target tick; mapping and result teardown are
outside timing. These seek different entity sets from Python player snapshots,
so their absolute times should not be subtracted from the Python measurements.

| Target tick | Fresh | Prepared |
| --- | ---: | ---: |
| 32,712 | 75.55 ms | 52.52 ms |
| 65,423 | 39.59 ms | 18.73 ms |
| 98,134 | 72.76 ms | 45.08 ms |

Preparation reuse saves roughly 21-28 ms here. Target-dependent decode cost still
varies with distance from the preceding full packet. Local Criterion results are
under `target/criterion/`, saved as baseline `initial`. Reproduce from the root:

```bash
BOON_BENCH_DEMO="$PWD/103129247.dem" \
  cargo bench -p boon-deadlock --bench parse --locked -- seek --save-baseline initial
```

## Where to investigate first

1. **Serial dataset passes and repeated scans.** Combat takes about 1.91 seconds
   grouped versus 4.25 seconds sequentially; mixed extraction takes 3.39 versus
   5.96 seconds. Their outputs match. Ability-tick extraction takes 2.08 seconds
   with essentially equal wall and process CPU time, indicating mostly serial
   work. Profile these paths through [loader.rs](../src/loader.rs) and the Rust parser before
   choosing lower-level pbdems2 routines to optimize. Users can already avoid
   redundant work by calling `demo.load(...)` for related datasets together.
2. **Full per-tick frame construction and memory.** Player/trooper outputs are
   391/236 MiB on the standard demo. The existing segmented path already reduces
   wall time substantially; investigate allocations, frame construction and
   merging in [snapshots.rs](../src/snapshots.rs) before adding more threads. Use an allocation
   profile to separate necessary output storage from temporary buffers.
3. **Derived metrics after separating input acquisition.** `teamfights()` takes
   3.43 seconds, but this includes loading its inputs. Profile its acquisition
   and aggregation phases separately before attributing the cost to the Python
   algorithm in [stats.py](../python/boon/stats.py).
4. **Repeated catalog verification for name-heavy callers.** Warm map access still
   takes about 53 ms for all five functions, versus 62 ms with parsed-map caches
   cleared. [names.py](../python/boon/names.py) calls version resolution and verification on
   each access. Profile those checks and dictionary copies; consider caching
   verified state with correct invalidation. This is lower absolute cost than
   full parsing unless callers request names repeatedly.

The checker initially exposed nondeterministic row ordering in
`stat_modifier_events`: a Rust HashMap emits simultaneous changes in arbitrary
order. Three standard-demo parses contained exactly the same 219 rows and values.
The harness now checks chronological ticks and canonicalizes ties outside timing.
No production parser behavior was changed.

## Reproduce and compare

From `crates/boon-python/`, after the release build described in [README.md](README.md):

```bash
uv run --no-sync python benchmarks/bench_parse.py \
  --demo ../../97104148.dem --demo ../../103129247.dem --all \
  --repeat 3 --warmup 1 --threads 4 --segments 4 \
  --output ../../target/benchmarks/baseline.json

uv run --no-sync python benchmarks/bench_parse.py \
  --demo ../../103129247.dem --case dataset/player_ticks --case snapshot/sampled \
  --repeat 3 --warmup 1 --threads 4 --segments 1 \
  --output ../../target/benchmarks/serial-segments.json
```

The local raw reports are under the ignored `target/benchmarks/` directory.
`baseline.json` contains the full run, including samples, schemas, hashes, catalog
fingerprints and build metadata. `parallel-segments.json` is a two-case subset of
that report, suitable for comparison with `serial-segments.json` using
`compare.py --allow-config-change`. Reports from different harness versions must
be remeasured; do not relabel old results to bypass comparison checks.

Ten harness tests cover fresh-instance/preload isolation, output drift, the
stat-event ordering exception, environment/case guards and the offline guard.
All 16 Rust parser benchmark cases also passed a `--test` smoke run against
`97104148.dem`, as did all seven `boon-dev` CLI cases. Ruff, ty, Clippy and
the Sphinx build with warnings as errors passed. CI runs the Python helper tests and a three-case API smoke check;
it deliberately has no hardware-dependent timing gate.
