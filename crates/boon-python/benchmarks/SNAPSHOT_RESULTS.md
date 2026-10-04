# Snapshot allocation and preload experiment

Historical Boon 0.10.0 measurements. These reports do not describe current-format demos or the current release.

This experiment follows [the initial baseline](RESULTS.md). It uses the same
standard and Street Brawl demos, four segments/threads, one warmup and three
measured repetitions. Both revisions use the same updated harness and output
fingerprints. Timings exclude construction except for constructor cases.

The implementation transfers numeric vectors into Polars, builds frames in each
segment worker, and appends their chunks in segment order. Strings and booleans
retain their required conversion. Output dtypes and row order are preserved.

Bounded initial reservations were also tested using requested tick counts and
first-observed entity counts. They improved some smaller workloads, but increased
standard-demo trooper peak RSS from about 1,225 to 1,371 MiB compared with chunked
assembly alone, without a clear corresponding speed benefit. That heuristic was
removed; the simpler owned-buffer/chunked implementation is retained.

`Demo(path)` now preloads kills, damage and abilities together; `preload=False`
keeps construction lightweight. CLI commands choose the latter and request only
the data they need. Benchmarks explicitly disable implicit preloading when timing
fresh dataset loads. On the earlier revision, `construct/combat` measures
construction followed by the equivalent grouped load.

During the experiment, the original ordering check was found to copy wide chunked
frames when adding row indices. The revised checker hashes the row hashes with
their positions instead. Baseline and final runs were both remeasured with that
checker; reports from the old harness are not directly comparable. Process RSS
still includes imports, mapped input, warmup, output checks and retained allocator
memory, so it is not a dataset allocation count.
## Results

Measured on WSL2, Ryzen 7 7800X3D, Python 3.13.14 and Polars 1.38.0,
using release builds. Values are median seconds, before → after.

| Workload | Street Brawl (`97104148`) | Standard (`103129247`) |
| --- | ---: | ---: |
| `construct` | 0.009 → 0.009 (-2.2%) | 0.013 → 0.010 (-26.6%) |
| `construct/combat` | 0.453 → 0.450 (-0.6%) | 1.824 → 1.821 (-0.1%) |
| `dataset/player_ticks` | 0.316 → 0.213 (-32.4%) | 1.034 → 0.748 (-27.6%) |
| `dataset/world_ticks` | 0.114 → 0.119 (+3.7%) | 0.441 → 0.435 (-1.5%) |
| `dataset/troopers` | 0.244 → 0.203 (-16.6%) | 1.060 → 0.764 (-28.0%) |
| `snapshot/single` | 0.024 → 0.023 (-0.2%) | 0.015 → 0.016 (+1.0%) |
| `snapshot/sampled` | 0.150 → 0.147 (-2.0%) | 0.542 → 0.535 (-1.1%) |
| `snapshot/sparse` | 0.065 → 0.065 (+0.7%) | 0.094 → 0.094 (-0.1%) |
| `snapshot/many` | 0.151 → 0.149 (-1.4%) | 0.534 → 0.520 (-2.5%) |
| `snapshot/window` | 0.149 → 0.159 (+7.0%) | 0.534 → 0.520 (-2.7%) |
| `derived/teamfights` | 0.755 → 0.729 (-3.5%) | 2.823 → 2.825 (+0.1%) |
| `derived/in_combat` | 0.330 → 0.205 (-37.8%) | 1.099 → 0.761 (-30.8%) |
| `derived/time_dead` | 0.346 → 0.264 (-23.6%) | 1.196 → 0.940 (-21.4%) |
| `load/mixed` | 0.752 → 0.633 (-15.8%) | 2.769 → 2.533 (-8.6%) |

Full player/trooper snapshots improved by 17–32%; sampled and sparse
snapshots changed little. The small-demo window rose 7%, with substantial
sample variation. Three repetitions on one machine do not establish small
performance differences.

Default construction now pays for the combat scan: about 0.45 seconds for
Street Brawl and 1.82 seconds for the standard demo, compared with about
0.01 seconds for `preload=False`. This moves the grouped scan into construction;
it does not remove that work. The three combat datasets are cached afterward.

### Memory tradeoffs

| Workload | Street Brawl peak MiB | Standard peak MiB |
| --- | ---: | ---: |
| `dataset/player_ticks` | 450 → 368 | 1,254 → 1,225 |
| `dataset/troopers` | 391 → 335 | 1,374 → 1,229 |
| `derived/in_combat` | 468 → 424 | 1,305 → 1,622 |
| `derived/time_dead` | 480 → 371 | 1,363 → 1,331 |
| `load/mixed` | 453 → 368 | 1,317 → 1,366 |

Peak memory did not improve for every workload. A repeat of the standard
`in_combat` and mixed-load cases measured 1,394 and 1,445 MiB respectively,
compared with 1,622 and 1,367 MiB in the main candidate run. These worker
high-water measurements vary with allocator retention and execution scheduling;
they are insufficient to attribute the increases to a particular allocation.
The changes improve full-table extraction, but do not promise lower peak memory
for every downstream analysis.

## Validation and reproduction

- All 28 baseline/candidate cases matched schemas, row counts, value hashes and
  order-sensitive hashes.
- All 70 selected tests passed, covering parallel/serial snapshots, preloading
  against grouped lazy loading, CLI behavior, malformed input, and hero switching.
- Workspace Clippy with warnings denied, Rust formatting, Ruff, ty, and the
  Sphinx documentation build passed.

Raw local reports are `target/benchmarks/snapshot-baseline.json` and
`target/benchmarks/snapshot-optimized.json`; the repeat is
`target/benchmarks/snapshot-memory-check.json`. They record input, harness and
native-extension SHA-256 values, settings and individual samples. The baseline
uses the saved original extension and the candidate uses the final release
build; both are from the working tree, so use their extension hashes to identify
the measured binaries. Reports under `target/` are not committed.

After building each revision with the same harness, from `crates/boon-python/`:

```bash
cases=(construct construct/combat dataset/player_ticks dataset/world_ticks
       dataset/troopers snapshot/single snapshot/sampled snapshot/sparse
       snapshot/many snapshot/window derived/teamfights derived/in_combat
       derived/time_dead load/mixed)
args=()
for case in "${cases[@]}"; do args+=(--case "$case"); done
uv run --no-sync python benchmarks/bench_parse.py \
  --demo ../../97104148.dem --demo ../../103129247.dem \
  "${args[@]}" --repeat 3 --warmup 1 --threads 4 --segments 4 \
  --output ../../target/benchmarks/snapshot-optimized.json
uv run --no-sync python benchmarks/compare.py \
  ../../target/benchmarks/snapshot-baseline.json \
  ../../target/benchmarks/snapshot-optimized.json
```
