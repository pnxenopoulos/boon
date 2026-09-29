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
# Fetch latest protos from SteamTracking
./scripts/sync-protos.sh

# Regenerate src/proto.rs
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

## Check for upstream changes

From the repository root, run:

```bash
./scripts/sync-protos.sh --check
```

This command compares the files in `proto/allowlist.txt` with
[SteamTracking/GameTracking-Deadlock](https://github.com/SteamTracking/GameTracking-Deadlock/tree/master/Protobufs).
It reports changed or missing files and returns a nonzero exit code on a mismatch
or download error. It does not change local files or package versions.
Line endings and changes to `steam.inf` alone do not cause a mismatch.
Set `DEADLOCK_REF` to check a specific upstream commit, branch, or tag.

CI runs this check on pull requests and pushes to `main`. The job summary shows
the result. A failed check adds a warning but does not block `CI Check` or releases.

## Version tracking

The crate version records the upstream build as
`MAJOR.MINOR.SourceRevision+ServerVersion`. `MAJOR.MINOR` identifies the
protobuf API compatibility line. The Deadlock source revision is the patch
version. SemVer build metadata contains the server build.
`scripts/sync-protos.sh` updates the version.

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
