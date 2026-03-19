use std::collections::HashSet;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};

/// Path to a demo file for benchmarking. Uses the smaller demo in the repo.
const DEMO_PATH: &str = "../../crates/boon-python/55353734.dem";

fn bench_events(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench_events: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("events_full", |b| {
        b.iter(|| {
            let parser = boon::Parser::from_file(path).unwrap();
            let events = parser.events(None).unwrap();
            std::hint::black_box(events.len());
        });
    });
}

fn bench_run_to_end(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench_run_to_end: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("run_to_end", |b| {
        b.iter(|| {
            let parser = boon::Parser::from_file(path).unwrap();
            let mut tick_count = 0u32;
            parser
                .run_to_end(|_ctx| {
                    tick_count += 1;
                })
                .unwrap();
            std::hint::black_box(tick_count);
        });
    });
}

fn bench_run_to_end_filtered(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!(
            "skipping bench_run_to_end_filtered: {} not found",
            DEMO_PATH
        );
        return;
    }

    let class_filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();

    c.bench_function("run_to_end_filtered_pawn", |b| {
        b.iter(|| {
            let parser = boon::Parser::from_file(path).unwrap();
            let mut tick_count = 0u32;
            parser
                .run_to_end_filtered(&class_filter, |_ctx| {
                    tick_count += 1;
                })
                .unwrap();
            std::hint::black_box(tick_count);
        });
    });
}

fn bench_run_to_end_with_events(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    let class_filter: HashSet<&str> = ["CCitadelPlayerPawn"].into_iter().collect();

    c.bench_function("run_to_end_with_events_filtered_pawn", |b| {
        b.iter(|| {
            let parser = boon::Parser::from_file(path).unwrap();
            let mut tick_count = 0u32;
            let mut event_count = 0u32;
            parser
                .run_to_end_with_events_filtered(&class_filter, |_ctx, events| {
                    tick_count += 1;
                    event_count += events.len() as u32;
                })
                .unwrap();
            std::hint::black_box((tick_count, event_count));
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_events, bench_run_to_end, bench_run_to_end_filtered, bench_run_to_end_with_events
}
criterion_main!(benches);
