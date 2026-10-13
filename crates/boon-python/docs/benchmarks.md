# Performance benchmarks

The [benchmark guide](https://github.com/pnxenopoulos/boon/tree/main/crates/boon-python/benchmarks)
covers release builds, Python dataset workloads, Criterion parser phases,
output-checked comparisons, memory interpretation and CPU profiling.

Start with the actual workload you want to speed up: a fresh dataset load,
a grouped load, sparse snapshots, full snapshots or a derived metric. Use the
Rust benchmarks and pbdems2 microbenchmarks to narrow down expensive components.

From `crates/boon-python/`:

```bash
uv run maturin develop --release --locked
uv run --no-sync python benchmarks/bench_parse.py --list
uv run --no-sync python benchmarks/bench_parse.py \
  --demo /path/to/match.dem --output ../../target/benchmarks/baseline.json
```

Runs use explicit local demo files and a process for each case.
They record build and input fingerprints, thread settings, initialization and
operation times, output sizes, process peak RSS where supported, and output
fingerprints. Downloads are not included; install catalogs with `boon get` before
measurements of features that use names.

Use `--case minimap/single` for one selected minimap tick, or
`--case minimap/sampled` for a 64-tick interval. Both cases scan the replay to
preserve last-seen history.

Use the same harness and environment for the two revisions. Compare the reports:

```bash
uv run --no-sync python benchmarks/compare.py \
  ../../target/benchmarks/baseline.json ../../target/benchmarks/candidate.json
```

The tool rejects changed output fingerprints or mismatched measurement settings.
Run repeated comparisons on an idle machine. CI only smoke-tests the harness;
shared-runner timings are not a performance gate.

## Parser core 0.4.0

Boon 0.16.0 uses pbdems2 0.4.0. Paired controls separate checked field paths,
child-value cleanup, shared packet handling, and decoded-baseline caching.
The cache recovers much of the full-parse cost. Packed-prefix cleanup reduces
parse times by a further 1.9–4.5% in the measured replay workloads. Later
field-path changes reduce Deadlock parse times by about 3–5% in paired controls.
A later cleanup change reduces CS2 player-only parse times by 1.3–2.0% across
five demos. A later vector skip change reduces two affected synthetic cases by
about 8–10%. A follow-up reduces coordinate and quantized vector skip times by
34% and 39%. The earlier mixed entity gains remain; an isolated mixed-value
control takes about 2% more time. Replay timing changes are within 1% of the
control; this does not show a clear complete-replay speedup. Costs relative to
the pre-fix core remain. The report records these limits.

Reference-parser comparisons must use the same input, classes, table updates,
allocator, release profile, and timing boundary. The earlier source2-demo
control skipped non-baseline table updates. The corrected comparison includes
them and keeps state parsing separate from Python datasets, stat calculations,
and ordered modifier history. Sample checks do not prove full output equality.

See the [measured results](https://github.com/pnxenopoulos/pbdems2/blob/main/crates/pbdems2-bench/RELEASE_0.4.0.md)
for the controls, inputs, costs, and verification limits.
