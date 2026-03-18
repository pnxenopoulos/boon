#!/usr/bin/env bash
# Prints the minimum Rust version required by the workspace's dependency tree.
# Compare this against the rust-version field in the root Cargo.toml.

set -euo pipefail

declared=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | python3 -c "
import json, sys
meta = json.load(sys.stdin)
for pkg in meta['packages']:
    rv = pkg.get('rust_version')
    if rv:
        print(rv)
        break
")

required=$(cargo metadata --format-version 1 2>/dev/null \
  | python3 -c "
import json, sys
meta = json.load(sys.stdin)
max_msrv = '0.0.0'
max_pkg = ''
for pkg in meta['packages']:
    msrv = pkg.get('rust_version')
    if msrv and msrv > max_msrv:
        max_msrv = msrv
        max_pkg = f'{pkg[\"name\"]} {pkg[\"version\"]}'
print(f'{max_msrv} (from {max_pkg})')
")

echo "Declared MSRV:  ${declared:-<not set>}"
echo "Required MSRV:  ${required}"
