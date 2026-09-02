#!/usr/bin/env bash
set -euo pipefail

# Fetch name-table vdata from SteamDatabase/GameTracking-Deadlock
# and regenerate the name lookup tables in crates/boon/src/.
#
# What it does:
# 1) Clones SteamDatabase/GameTracking-Deadlock (sparse checkout if available)
# 2) Copies the required VData and English hero/item localization files to the
#    repo root
# 3) Runs the generator to refresh the name/stat tables in crates/boon/src
# 4) Runs `cargo fmt --all` so the regenerated tables are correctly formatted
# 5) Cleans up the temporary generator inputs
#
# The modifier table is built purely from these two vdata files: modifiers.vdata
# (top-level keys + nested `_my_subclass_name` values) plus the modifier
# subclasses nested in abilities.vdata. See scripts/generate-name-tables/main.rs.
#
# Environment:
#   DEADLOCK_REF=<ref>   optional: branch/tag/commit to checkout

REPO_URL="https://github.com/SteamDatabase/GameTracking-Deadlock.git"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/.."

VDATA_DIR="game/citadel/pak01_dir/scripts"
HERO_LOCALIZATION_DIR="game/citadel/resource/localization/citadel_heroes"
ITEM_LOCALIZATION_DIR="game/citadel/resource/localization/citadel_gc_mod_names"
# modifiers.vdata holds the generic modifiers; the bulk of gameplay modifiers are
# nested as modifier subclasses inside abilities.vdata (see
# scripts/generate-name-tables/main.rs).
VDATA_FILES=(abilities.vdata modifiers.vdata heroes.vdata misc.vdata)
LOCALIZATION_FILES=(citadel_heroes_english.txt citadel_gc_mod_names_english.txt)
LOCALIZATION_SOURCES=(
  "$HERO_LOCALIZATION_DIR/citadel_heroes_english.txt"
  "$ITEM_LOCALIZATION_DIR/citadel_gc_mod_names_english.txt"
)

DEADLOCK_REF="${DEADLOCK_REF:-}"

die() { echo "ERROR: $*" >&2; exit 1; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }

need_cmd git
need_cmd cargo

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
  for file in "${VDATA_FILES[@]}"; do
    rm -f "$ROOT_DIR/$file"
  done
  for file in "${LOCALIZATION_FILES[@]}"; do
    rm -f "$ROOT_DIR/$file"
  done
}
trap cleanup EXIT

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
    git sparse-checkout set "$VDATA_DIR" "$HERO_LOCALIZATION_DIR" "$ITEM_LOCALIZATION_DIR" >/dev/null 2>&1 || true
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
  for index in "${!LOCALIZATION_FILES[@]}"; do
    local file="${LOCALIZATION_FILES[$index]}"
    local src="$REPO_DIR/${LOCALIZATION_SOURCES[$index]}"
    [[ -f "$src" ]] || die "Missing localization file in upstream: $src"
    cp -f "$src" "$ROOT_DIR/$file"
    echo "Copied $file to repo root"
  done
}

generate_tables() {
  echo "Generating name tables..."
  cd "$ROOT_DIR"
  cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
}

format_tables() {
  echo "Formatting with cargo fmt..."
  cd "$ROOT_DIR"
  cargo fmt --all
}

main() {
  clone_repo
  copy_vdata
  generate_tables
  format_tables
  echo "Done. Updated abilities.rs, ability_display_names.rs, breakables.rs, modifiers.rs, resistances.rs, and stat_catalog.rs"
}

main "$@"
