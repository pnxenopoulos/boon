//! End-to-end CLI command benchmarks.
//!
//! Each benchmark runs one `boon` subcommand against a demo, capturing the full
//! cost a user pays at the terminal: parse + the command's (class-filtered)
//! entity/event walk + formatting. This complements `boon`'s `parse` bench,
//! which isolates the parser phases; here the question is "how long does each
//! command take, end to end, and which ones are expensive?"
//!
//! Only full-demo commands are benchmarked — each does a complete pass, so a
//! handful of iterations is meaningful and their `--limit 0` output stays to a
//! couple of lines. Cheap header-only commands (`info`, `summary`) are covered
//! by the parser bench's `init` / `events` phases instead. `ability_ticks` is
//! the heaviest: it decodes every `*Ability*` class.
//!
//! Demo resolution matches the parser bench: `$BOON_BENCH_DEMO`, else the
//! smallest `.dem` under `crates/boon-python/tests/fixtures/`. Fixtures are
//! gitignored, so the bench skips when none is present.

use std::path::PathBuf;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};

/// Resolve the demo file to benchmark against (see `boon`'s `parse` bench).
///
/// Uses `$BOON_BENCH_DEMO` when set, otherwise the smallest `.dem` in the shared
/// fixtures directory. Returns `None` when no demo is available.
fn demo_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BOON_BENCH_DEMO") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../boon-python/tests/fixtures");
    let mut demos: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "dem"))
        .collect();
    demos.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX));
    demos.into_iter().next()
}

/// End-to-end timings for the full-demo commands.
fn bench_commands(c: &mut Criterion) {
    let Some(path) = demo_path() else {
        eprintln!("boon cli bench: no demo available (set BOON_BENCH_DEMO); skipping");
        return;
    };

    let mut g = c.benchmark_group("commands");
    g.sample_size(10);
    g.warm_up_time(Duration::from_secs(2));
    g.measurement_time(Duration::from_secs(20));

    // Each command shares the (file, filter, summary, limit, min_tick, max_tick,
    // json) signature. `limit = Some(0)` keeps stdout to a header + footer line.
    macro_rules! bench_cmd {
        ($name:literal, $cmd:path) => {
            g.bench_function($name, |b| {
                b.iter(|| {
                    $cmd(&path, None, false, Some(0), None, None, false).unwrap();
                });
            });
        };
    }

    bench_cmd!("abilities", boon_cli::commands::abilities);
    bench_cmd!("objectives", boon_cli::commands::objectives);
    bench_cmd!("troopers", boon_cli::commands::troopers);
    bench_cmd!("neutrals", boon_cli::commands::neutrals);
    bench_cmd!("stat_modifiers", boon_cli::commands::stat_modifiers);
    bench_cmd!("active_modifiers", boon_cli::commands::active_modifiers);
    bench_cmd!("ability_ticks", boon_cli::commands::ability_ticks);

    g.finish();
}

criterion_group!(benches, bench_commands);
criterion_main!(benches);
