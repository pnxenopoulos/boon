//! Parser pipeline benchmarks.
//!
//! Each phase of the parse pipeline is timed in isolation so it is obvious where
//! the time goes:
//!
//! - `init` group — the one-time costs paid before any tick is decoded:
//!   `from_file` (open + memory-map), `parse_send_tables`, `parse_class_info`,
//!   and `parse_init` (send tables, classes, string tables, instance baselines).
//! - `decode` group — `messages` (enumerate every message without decoding
//!   entities), `events` (decode all game events), `run_to_end` (full entity
//!   decode: every class, every tick), and two class-filtered decodes. The
//!   filtered variants quantify how much the class filter saves: `pawn` decodes
//!   a single class, while `abilities` decodes every `Ability` class — the set
//!   the `ability_ticks` dataset and the `ability-ticks` CLI command walk.
//!
//! The demo is the smallest `.dem` under `crates/boon-python/tests/fixtures/`,
//! overridable with `BOON_BENCH_DEMO=/path/to/match.dem`. The fixtures are
//! gitignored, so when none is present every benchmark skips (matching the test
//! suite).
//!
//! The `decode` benchmarks report throughput (MiB/s) normalized by demo size,
//! since each scans the whole file — comparable across demos of different sizes.
//! The `init` benchmarks report raw time only: they read just part of the file
//! (and `from_file` merely memory-maps it), so a bytes/sec figure would mislead.
//!
//! Parse phases run from an in-memory copy of the demo (`from_bytes`) so disk
//! I/O is excluded from everything except the dedicated `from_file` benchmark;
//! the per-iteration clone happens in `iter_batched` setup and is not timed.

use std::collections::HashSet;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

/// Resolve the demo file to benchmark against.
///
/// Uses `$BOON_BENCH_DEMO` when set, otherwise the smallest `.dem` in the shared
/// fixtures directory (smallest = fastest to iterate on). Returns `None` when no
/// demo is available, in which case the caller skips.
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

/// One-time setup costs paid before any tick is decoded.
fn bench_init(c: &mut Criterion) {
    let Some(path) = demo_path() else {
        eprintln!("boon parse bench: no demo available (set BOON_BENCH_DEMO); skipping `init`");
        return;
    };
    let bytes = std::fs::read(&path).expect("read demo file");

    // No throughput here: these phases read only part of the demo (and
    // `from_file` just memory-maps it, faulting pages in lazily during parse),
    // so a bytes/sec figure normalized by file size would be meaningless. Raw
    // time is the metric that matters for one-time init cost.
    let mut g = c.benchmark_group("init");
    g.sample_size(20);
    g.warm_up_time(Duration::from_secs(2));
    g.measurement_time(Duration::from_secs(8));

    // Open + memory-map the demo (zero-copy; pages fault in later during parse).
    g.bench_function("from_file", |b| {
        b.iter(|| {
            black_box(boon::Parser::from_file(&path).unwrap());
        });
    });

    // Decode the send-table (serializer) field schemas.
    g.bench_function("parse_send_tables", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                black_box(p.parse_send_tables().unwrap());
            },
            BatchSize::LargeInput,
        );
    });

    // Decode the class-id -> network-name table.
    g.bench_function("parse_class_info", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                black_box(p.parse_class_info().unwrap());
            },
            BatchSize::LargeInput,
        );
    });

    // Full init: send tables + classes + string tables + instance baselines.
    g.bench_function("parse_init", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                black_box(p.parse_init().unwrap());
            },
            BatchSize::LargeInput,
        );
    });

    g.finish();
}

/// Per-tick decode costs, full demo.
fn bench_decode(c: &mut Criterion) {
    let Some(path) = demo_path() else {
        eprintln!("boon parse bench: no demo available (set BOON_BENCH_DEMO); skipping `decode`");
        return;
    };
    let bytes = std::fs::read(&path).expect("read demo file");
    let len = bytes.len() as u64;

    // Collect every `*Ability*` class name — the set `ability_ticks` decodes.
    // Done once, outside the timed region, and kept as owned `String`s so the
    // borrowed `&str` filter below can reference them.
    let ability_classes: Vec<String> = boon::Parser::from_bytes(bytes.clone())
        .parse_send_tables()
        .map(|sc| {
            sc.serializers
                .keys()
                .filter(|n| n.contains("Ability"))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let mut g = c.benchmark_group("decode");
    g.sample_size(10);
    g.warm_up_time(Duration::from_secs(3));
    g.measurement_time(Duration::from_secs(30));
    g.throughput(Throughput::Bytes(len));

    // Enumerate every message without decoding entities (cheapest full scan).
    g.bench_function("messages", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                black_box(p.messages().unwrap().len());
            },
            BatchSize::LargeInput,
        );
    });

    // Decode all game events (user messages: kills, damage, objectives, …).
    g.bench_function("events", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                black_box(p.events(None).unwrap().len());
            },
            BatchSize::LargeInput,
        );
    });

    // Full entity decode: every networked class, every tick. Worst case.
    g.bench_function("run_to_end", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                let mut ticks = 0u32;
                p.run_to_end(|_| ticks += 1).unwrap();
                black_box(ticks);
            },
            BatchSize::LargeInput,
        );
    });

    // Class-filtered decode: a single class (the player pawn).
    let pawn: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();
    g.bench_function("run_to_end_filtered_pawn", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                let mut ticks = 0u32;
                p.run_to_end_filtered(&pawn, |_| ticks += 1).unwrap();
                black_box(ticks);
            },
            BatchSize::LargeInput,
        );
    });

    // Class-filtered decode: every ability class (what `ability_ticks` pays).
    let abilities: HashSet<&str> = ability_classes.iter().map(String::as_str).collect();
    g.bench_function("run_to_end_filtered_abilities", |b| {
        b.iter_batched(
            || bytes.clone(),
            |bytes| {
                let p = boon::Parser::from_bytes(bytes);
                let mut ticks = 0u32;
                p.run_to_end_filtered(&abilities, |_| ticks += 1).unwrap();
                black_box(ticks);
            },
            BatchSize::LargeInput,
        );
    });

    g.finish();
}

criterion_group!(benches, bench_init, bench_decode);
criterion_main!(benches);
