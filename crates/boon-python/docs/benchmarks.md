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

Runs use explicit local demo files and separate processes for each case.
They record build and input fingerprints, thread settings, initialization and
operation times, output sizes, process peak RSS where supported, and output
fingerprints. Downloads are excluded; install catalogs with `boon get` before
measurements of features that use names.

Use the same harness and environment for both revisions. Compare the reports:

```bash
uv run --no-sync python benchmarks/compare.py \
  ../../target/benchmarks/baseline.json ../../target/benchmarks/candidate.json
```

The tool rejects changed output fingerprints or mismatched measurement settings.
Run repeated comparisons on an idle machine. CI only smoke-tests the harness;
shared-runner timings are not a performance gate.
