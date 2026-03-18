use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};

/// Path to a demo file for benchmarking.
const DEMO_PATH: &str = "../../crates/boon-python/55353734.dem";

fn bench_troopers(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("cli_troopers", |b| {
        b.iter(|| {
            boon_cli::commands::troopers(path, None, false, Some(0), None, None, false).unwrap();
        });
    });
}

fn bench_objectives(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("cli_objectives", |b| {
        b.iter(|| {
            boon_cli::commands::objectives(path, None, false, Some(0), None, None, false).unwrap();
        });
    });
}

fn bench_abilities(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("cli_abilities", |b| {
        b.iter(|| {
            boon_cli::commands::abilities(path, None, false, Some(0), None, None, false).unwrap();
        });
    });
}

fn bench_stat_modifiers(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("cli_stat_modifiers", |b| {
        b.iter(|| {
            boon_cli::commands::stat_modifiers(path, None, false, Some(0), None, None, false)
                .unwrap();
        });
    });
}

fn bench_active_modifiers(c: &mut Criterion) {
    let path = Path::new(DEMO_PATH);
    if !path.exists() {
        eprintln!("skipping bench: {} not found", DEMO_PATH);
        return;
    }

    c.bench_function("cli_active_modifiers", |b| {
        b.iter(|| {
            boon_cli::commands::active_modifiers(path, None, false, Some(0), None, None, false)
                .unwrap();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_troopers, bench_objectives, bench_abilities, bench_stat_modifiers, bench_active_modifiers
}
criterion_main!(benches);
