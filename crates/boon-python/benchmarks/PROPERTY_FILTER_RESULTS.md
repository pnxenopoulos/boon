# Combat and ability-state property filtering — 2026-10-15

Compare matched Boon 0.17.0 release builds: the original loader with pbdems2
0.4.0, and the filtered loader with local pbdems2 0.4.1. Both use Rust 1.96.1,
Python 3.13.14 and Polars 1.38.0 on a Ryzen 7 7800X3D under WSL2.

## Python API

Use the existing `bench_parse.py` workload functions in fresh processes. Warm
each input file, run one warmup, then seven samples per build in alternating
order. Use equal CPU affinity and thread settings. Four-thread runs use CPUs
0, 2, 4 and 6; one-thread runs use CPU 2. No builds or tests run during timing.

The table shows median seconds. Timers include parsing and result construction.
Imports, input warming, output checks and IPC writes are outside the timer.
Construction is excluded except for `construct/combat`, which includes default
combat preloading. Every returned table matches exactly, including row order.

| Demo | Workload, four threads | Before (s) | After (s) | Change |
| --- | --- | ---: | ---: | ---: |
| 108575009.dem | `load/combat` | 2.307 | 2.173 | -5.8% |
| 108575009.dem | `dataset/ability_ticks` | 2.303 | 2.172 | -5.7% |
| 108575009.dem | `load/combat_state` | 2.398 | 2.276 | -5.1% |
| 108575009.dem | `construct/combat` | 2.343 | 2.212 | -5.6% |
| 108575009.dem | `snapshot/sampled` | 2.745 | 2.745 | -0.0% |
| 108575009.dem | `load/economy` | 2.456 | 2.477 | +0.8% |
| 111155916.dem | `load/combat` | 2.172 | 2.063 | -5.0% |
| 111155916.dem | `dataset/ability_ticks` | 2.180 | 2.056 | -5.7% |
| 111155916.dem | `load/combat_state` | 2.281 | 2.169 | -4.9% |
| 111155916.dem | `construct/combat` | 2.215 | 2.086 | -5.8% |
| 111155916.dem | `snapshot/sampled` | 2.524 | 2.545 | +0.8% |
| 111155916.dem | `load/economy` | 2.312 | 2.330 | +0.8% |

Single-thread combat loads improve by 5.3–5.7%; `ability_ticks` improves by
5.3–5.5%. These are end-to-end results for these two replays, not a general
guarantee for other files or machines. Snapshot and economy controls differ by
less than 1%.

## Full-decoding controls

Keep full and projected benchmarks in the candidate native binary. Pin runs to
CPU 2 and alternate both build orders. The initial ten-sample Criterion controls
showed no material regression across six full-field workloads. After the skip
cleanup, repeat all six with Criterion quick sampling, twice per build. One noisy
pawn baseline receives two additional paired rounds; retain its original samples.
Values below are medians of run medians. Small changes are not evidence of a
general full-decoding speedup.

| Demo | Full-field workload | Before (s) | After (s) | Change |
| --- | --- | ---: | ---: | ---: |
| 108575009.dem | `run_to_end` | 2.911 | 2.878 | -1.1% |
| 108575009.dem | `run_to_end_filtered_pawn` | 2.216 | 2.227 | +0.5% |
| 108575009.dem | `run_to_end_filtered_abilities` | 2.001 | 1.999 | -0.1% |
| 111155916.dem | `run_to_end` | 2.708 | 2.714 | +0.2% |
| 111155916.dem | `run_to_end_filtered_pawn` | 2.096 | 2.094 | -0.1% |
| 111155916.dem | `run_to_end_filtered_abilities` | 1.935 | 1.910 | -1.3% |

Full decoding retains its separate field loop. Property selection adds no
per-field checks to that loop. An initial dependency build increased sampled
snapshot time by 1.5–2%. Moving binary-block validation to a private helper
removed most of that cost while retaining limits, errors and cursor behavior.
The word-scan experiments did not improve results and were removed.

## Correctness and scope

- Combat and `ability_ticks` retain only their required pawn and ability fields.
- Unlisted classes retain all fields. Mixed `active_modifiers` and `urn` requests
  use full fields. Later schema commands also use full decoding.
- Differential checks compare all four combat/state datasets on both replays.
  Additional checks cover Street Brawl and the hero-swap regression replay.
- Mixed modifier and urn requests match the original loader on both replays.
- The final wheel passes 678 Python tests, with four existing skips, and 12
  benchmark-helper tests. Rust tests, all 18 native smoke cases, Clippy, format,
  Ruff, ty and documentation builds pass.

Raw samples, output checks, saved binaries and native extension SHA-256 values
are in the ignored `target/property-filter/` directory. The local dependency
uses a command-scoped Cargo patch. After publishing pbdems2 0.4.1, refresh the
Boon registry lock entry with `cargo update -p pbdems2 --precise 0.4.1` before CI.
