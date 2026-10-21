<div align="center">

# boon-proto

[![crates.io](https://img.shields.io/crates/v/boon-proto.svg)](https://crates.io/crates/boon-proto)
[![docs.rs](https://docs.rs/boon-proto/badge.svg)](https://docs.rs/boon-proto)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/boon/blob/main/LICENSE)

</div>

```rust
use boon_proto::proto::CDemoFileHeader;

fn main() {
    let header = CDemoFileHeader::default();
    println!("Map: {:?}", header.map_name);
}
```

Rust types for Valve Deadlock Source 2 protobuf messages, generated with prost.
The [Boon](https://github.com/pnxenopoulos/boon) replay parser uses these types.

## Boon protobuf source

[`prost`](https://github.com/tokio-rs/prost) generates this Rust code from Valve's
`.proto` files. The repository contains `src/proto.rs`. Use these types without `protoc`.

## Install the boon-proto Rust crate

```toml
[dependencies]
boon-proto = "0.4"
```

## Regenerate Boon Rust protobuf types

Use the scripts in the [Boon repository](https://github.com/pnxenopoulos/boon)
when an upstream `.proto` file changes:

```bash
# Fetch latest protos from SteamTracking
./scripts/sync-protos.sh

# Regenerate src/proto.rs
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```

The generator uses a bundled `protoc`. It removes unsupported C++ annotations
from temporary inputs. The source `.proto` files stay unchanged.

## Check Boon protobufs against SteamTracking

From the repository root, run:

```bash
./scripts/sync-protos.sh --check
```

This command compares the files in `proto/allowlist.txt` with
[SteamTracking/GameTracking-Deadlock](https://github.com/SteamTracking/GameTracking-Deadlock/tree/master/Protobufs).
It reports changed or missing files and returns a nonzero exit code on a mismatch
or download error. It does not change local files or package versions.
Line endings and changes to `steam.inf` alone do not cause a mismatch.
Set `DEADLOCK_REF` to compare a specific upstream commit, branch, or tag.

CI runs this check on pull requests and pushes to `main`. The job summary shows
the result. A failed check adds a warning but does not block `CI Check` or releases.

## Read boon-proto version numbers

The crate version records the upstream build as
`MAJOR.MINOR.SourceRevision+ServerVersion`. `MAJOR.MINOR` identifies the
protobuf API compatibility line. The Deadlock source revision is the patch
version. SemVer build metadata contains the server build.
`scripts/sync-protos.sh` updates the version.

## License

MIT — see [LICENSE](https://github.com/pnxenopoulos/boon/blob/main/LICENSE) for details.
