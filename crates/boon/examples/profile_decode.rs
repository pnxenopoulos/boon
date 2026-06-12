//! Profiling harness for the entity-decode hot path.
//!
//! Loops `run_to_end` on a demo so an external sampler (`sample <pid>`,
//! `samply`, Instruments) has a long, steady window of decode work to attribute.
//! Not a benchmark — there is no timing here; it just keeps the decoder busy.
//!
//! ```sh
//! cargo build --release --example profile_decode -p boon-deadlock
//! BOON_BENCH_DEMO=/path/to.dem ./target/release/examples/profile_decode &
//! sample $! 15 -file /tmp/sample.txt   # 15s call-tree sample
//! ```
//!
//! Defaults to the largest fixture (longest per-iteration window).

use std::path::PathBuf;

fn main() {
    let path = std::env::var("BOON_BENCH_DEMO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("crates/boon-python/tests/fixtures/84133142.dem"));

    if !path.exists() {
        eprintln!("profile_decode: demo not found at {}", path.display());
        eprintln!("set BOON_BENCH_DEMO=/path/to.dem");
        std::process::exit(1);
    }

    let iters: u32 = std::env::var("BOON_PROFILE_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(u32::MAX);

    for _ in 0..iters {
        let parser = boon::Parser::from_file(&path).unwrap();
        let mut tick_count = 0u64;
        parser.run_to_end(|_| tick_count += 1).unwrap();
        std::hint::black_box(tick_count);
    }
}
