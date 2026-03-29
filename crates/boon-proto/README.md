<div align="center">

# boon-proto

[![crates.io](https://img.shields.io/crates/v/boon-proto.svg)](https://crates.io/crates/boon-proto)
[![docs.rs](https://docs.rs/boon-proto/badge.svg)](https://docs.rs/boon-proto)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

Pre-generated Rust types for Deadlock's protobuf definitions, used by the [Boon](https://github.com/pnxenopoulos/boon) demo parser.

## Overview

This crate contains auto-generated Rust code produced by [`prost`](https://github.com/tokio-rs/prost) from Valve's `.proto` files shipped with Deadlock. The generated output lives in `src/proto.rs` and is checked into version control so that downstream crates can build without needing `protoc`.

## Installation

```toml
[dependencies]
boon-proto = "0.1"
```

## Usage

```rust
use boon_proto::proto;

// Access Deadlock protobuf message types
let header = proto::CDemoFileHeader::default();
let event = proto::CCitadelUserMsgHeroKilled::default();
```

## Regenerating

When upstream `.proto` files change (e.g. after a Deadlock update), regenerate using the scripts in the [Boon repository](https://github.com/pnxenopoulos/boon):

```bash
# Fetch latest protos from SteamDatabase
./scripts/sync-protos.sh

# Regenerate src/proto.rs
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

## Version tracking

The `[package.metadata.boon-proto]` section in `Cargo.toml` records the Deadlock client/server version the protos were extracted from.

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
