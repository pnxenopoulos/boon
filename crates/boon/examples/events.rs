//! Game Events — extracts and prints game events from a demo file.
//!
//! Usage:
//!   cargo run -p boon-deadlock --example events -- <demo.dem>
//!   cargo run -p boon-deadlock --example events -- <demo.dem> Damage

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: events <demo.dem> [filter]");
    let filter = args.get(2);

    let parser = boon::Parser::from_file(Path::new(path)).expect("failed to open demo");
    let events = parser.events(None).expect("failed to parse events");

    let mut count = 0;
    for event in &events {
        // If a filter is given, skip events whose name doesn't contain it
        if let Some(f) = filter
            && !event.name.contains(f.as_str())
        {
            continue;
        }

        print!("[tick {:>6}] {}", event.tick, event.name);

        // Print key-value pairs for legacy game events
        if !event.keys.is_empty() {
            let pairs: Vec<String> = event
                .keys
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            print!("  {{ {} }}", pairs.join(", "));
        }

        // Try to decode the protobuf payload for richer output
        if let Some(decoded) = boon::decode_event_payload(event.msg_type, &event.payload) {
            // Print just the first line to keep output compact
            if let Some(first_line) = decoded.lines().next() {
                print!("  -> {}", first_line);
            }
        }

        println!();
        count += 1;
    }

    eprintln!("\n{} events printed (of {} total)", count, events.len());
}
