# Contributing to Boon

Sync Deadlock protobuf files with

```bash
./scripts/sync-protos.sh
```

Build generated code with

```bash
cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
```