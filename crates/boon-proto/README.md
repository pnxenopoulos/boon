# boon-proto

Pre-generated Rust types for Deadlock's protobuf definitions, used by the [Boon](https://github.com/pnxenopoulos/boon) parser.

## Overview

This crate contains auto-generated Rust code produced by [`prost`](https://github.com/tokio-rs/prost) from Valve's `.proto` files shipped with Deadlock. The generated output lives in `src/proto.rs` and is checked into version control so that downstream crates can build without needing `protoc`.

The `proto/allowlist.txt` file controls which `.proto` files are compiled. Only protos listed there are included in the build.

## Regenerating

When upstream `.proto` files change (e.g. after a Deadlock update), regenerate `src/proto.rs` using the build script at [`scripts/build-protos/`](../../scripts/build-protos/):

```bash
# Run from the repository root
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

This reads `proto/allowlist.txt`, compiles the listed `.proto` files with `prost-build`, and writes the combined output to `src/proto.rs`.

## Crate structure

```
boon-proto/
  proto/              # Raw .proto files from Deadlock
    allowlist.txt     # Controls which protos are compiled
    demo.proto
    netmessages.proto
    ...
  src/
    lib.rs            # Re-exports the proto module
    proto.rs          # @generated — DO NOT EDIT
```

## Version tracking

The `[package.metadata.boon-proto]` section in `Cargo.toml` records the Deadlock client version the protos were extracted from.
