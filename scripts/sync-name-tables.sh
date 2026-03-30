#!/usr/bin/env bash
set -euo pipefail

# Fetch abilities.vdata and modifiers.vdata from SteamDatabase/GameTracking-Deadlock
# and regenerate the name lookup tables in crates/boon/src/.
#
# What it does:
# 1) Clones SteamDatabase/GameTracking-Deadlock (sparse checkout if available)
# 2) Copies abilities.vdata and modifiers.vdata to the repo root
# 3) Runs the generate-name-tables script to regenerate abilities.rs and modifiers.rs
# 4) Cleans up the temporary vdata files
#
# Environment:
#   DEADLOCK_REF=<ref>   optional: branch/tag/commit to checkout

REPO_URL="https://github.com/SteamDatabase/GameTracking-Deadlock.git"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/.."

VDATA_DIR="game/citadel/pak01_dir/scripts"
VDATA_FILES=(abilities.vdata modifiers.vdata)

DEADLOCK_REF="${DEADLOCK_REF:-}"

die() { echo "ERROR: $*" >&2; exit 1; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }

need_cmd git
need_cmd cargo

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"; rm -f "$ROOT_DIR/abilities.vdata" "$ROOT_DIR/modifiers.vdata"' EXIT

REPO_DIR="$TMP_DIR/deadlock"

has_sparse_checkout() {
  git help -a 2>/dev/null | grep -qE '^\s*sparse-checkout\s*$'
}

clone_repo() {
  echo "Cloning GameTracking-Deadlock..."
  if git clone --filter=blob:none --no-checkout "$REPO_URL" "$REPO_DIR" >/dev/null 2>&1; then
    :
  else
    git clone --no-checkout "$REPO_URL" "$REPO_DIR"
  fi

  cd "$REPO_DIR"

  if has_sparse_checkout; then
    git sparse-checkout init --cone >/dev/null 2>&1 || true
    git sparse-checkout set "$VDATA_DIR" >/dev/null 2>&1 || true
  fi

  if [[ -n "$DEADLOCK_REF" ]]; then
    git checkout -f "$DEADLOCK_REF" >/dev/null 2>&1 || die "Failed to checkout DEADLOCK_REF=$DEADLOCK_REF"
  else
    git checkout -f >/dev/null 2>&1 || die "Failed to checkout repo"
  fi
}

copy_vdata() {
  for file in "${VDATA_FILES[@]}"; do
    local src="$REPO_DIR/$VDATA_DIR/$file"
    [[ -f "$src" ]] || die "Missing vdata file in upstream: $src"
    cp -f "$src" "$ROOT_DIR/"
    echo "Copied $file to repo root"
  done
}

generate_tables() {
  echo "Generating name tables..."
  cd "$ROOT_DIR"
  cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
}

main() {
  clone_repo
  copy_vdata
  generate_tables
  echo "Done. Updated crates/boon/src/abilities.rs and crates/boon/src/modifiers.rs"
}

main "$@"
