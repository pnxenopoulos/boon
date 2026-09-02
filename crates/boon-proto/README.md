<div align="center">

# boon-proto

[![crates.io](https://img.shields.io/crates/v/boon-proto.svg)](https://crates.io/crates/boon-proto)
[![docs.rs](https://docs.rs/boon-proto/badge.svg)](https://docs.rs/boon-proto)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

Pre-generated Rust types for the Deadlock protobuf definitions. The [Boon](https://github.com/pnxenopoulos/boon) demo parser uses these types.

## Overview

[`prost`](https://github.com/tokio-rs/prost) generates this Rust code from Valve's
`.proto` files. The repository contains `src/proto.rs`. Users do not need `protoc`.

## Installation

```toml
[dependencies]
boon-proto = "0.3"
```

## Usage

```rust
use boon_proto::proto;

// Access Deadlock protobuf message types
let header = proto::CDemoFileHeader::default();
let event = proto::CCitadelUserMsgHeroKilled::default();
```

## Regenerating

Use the scripts in the [Boon repository](https://github.com/pnxenopoulos/boon)
when an upstream `.proto` file changes:

```bash
# Fetch latest protos from SteamDatabase
./scripts/sync-protos.sh

# Regenerate src/proto.rs
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

## Version tracking

The crate version records the upstream build as
`MAJOR.MINOR.SourceRevision+ServerVersion`. `MAJOR.MINOR` identifies the
protobuf API compatibility line. The Deadlock source revision is the patch
version. SemVer build metadata contains the server build.
`scripts/sync-protos.sh` updates the version.

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
