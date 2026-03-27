#!/usr/bin/env bash
set -euo pipefail

# Sync Deadlock protos + version info into ../crates/boon-proto
#
# What it does:
# 1) Clones SteamDatabase/GameTracking-Deadlock (sparse checkout if available)
# 2) Copies ONLY the allowlisted Protobufs/*.proto into ../crates/boon-proto/proto/
# 3) Reads game/citadel/steam.inf and updates ../crates/boon-proto/Cargo.toml:
#    - Sets [package].version to MAJOR.MINOR.PATCH (Cargo-safe SemVer)
#    - Sets [package.metadata.boon-proto].version to:
#        MAJOR.MINOR.PATCH.ClientVersion.ServerVersion.SourceRevision
#
# Environment:
#   CLEAN_DEST=1         delete existing *.proto in DEST_DIR before copying
#   DEADLOCK_REF=<ref>   optional: branch/tag/commit to checkout

REPO_URL="https://github.com/SteamDatabase/GameTracking-Deadlock.git"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Destination for synced protos (relative to this script)
DEST_DIR="$SCRIPT_DIR/../crates/boon-proto/proto"

# Cargo.toml to update (relative to this script)
CARGO_TOML="$SCRIPT_DIR/../crates/boon-proto/Cargo.toml"

# Single source of truth for which protos to sync (relative to this script)
MANIFEST="$SCRIPT_DIR/../crates/boon-proto/proto/allowlist.txt"

CLEAN_DEST="${CLEAN_DEST:-0}"           # 1 to delete existing *.proto in DEST_DIR before copying
DEADLOCK_REF="${DEADLOCK_REF:-}"        # optional: branch/tag/commit to checkout

die() { echo "ERROR: $*" >&2; exit 1; }

need_file() { [[ -f "$1" ]] || die "Missing file: $1"; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }

need_cmd git
need_file "$CARGO_TOML"
need_file "$MANIFEST"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

REPO_DIR="$TMP_DIR/deadlock"
INF_PATH="game/citadel/steam.inf"

has_sparse_checkout() {
  git help -a 2>/dev/null | grep -qE '^\s*sparse-checkout\s*$'
}

clone_repo() {
  if git clone --filter=blob:none --no-checkout "$REPO_URL" "$REPO_DIR" >/dev/null 2>&1; then
    :
  else
    git clone --no-checkout "$REPO_URL" "$REPO_DIR"
  fi

  cd "$REPO_DIR"

  if has_sparse_checkout; then
    git sparse-checkout init --cone >/dev/null 2>&1 || true
    git sparse-checkout set "Protobufs" "$INF_PATH" >/dev/null 2>&1 || true
  fi

  if [[ -n "$DEADLOCK_REF" ]]; then
    git checkout -f "$DEADLOCK_REF" >/dev/null 2>&1 || die "Failed to checkout DEADLOCK_REF=$DEADLOCK_REF"
  else
    git checkout -f >/dev/null 2>&1 || die "Failed to checkout repo"
  fi
}

copy_protos() {
  mkdir -p "$DEST_DIR"

  if [[ "$CLEAN_DEST" == "1" ]]; then
    find "$DEST_DIR" -maxdepth 1 -type f -name '*.proto' -delete
  fi

  local copied=0
  while IFS= read -r raw || [[ -n "$raw" ]]; do
    raw="${raw%$'\r'}"              # strip CR for CRLF files

    local line="${raw%%#*}"
    line="$(echo -n "$line" | xargs || true)"
    [[ -z "$line" ]] && continue

    [[ "$line" == *.proto ]] || die "Manifest entry is not a .proto: '$line'"
    [[ "$line" != */* ]] || die "Manifest entry must be a basename (no '/'): '$line'"

    local src="$REPO_DIR/Protobufs/$line"
    [[ -f "$src" ]] || die "Missing proto in upstream: $src"

    cp -f "$src" "$DEST_DIR/"
    copied=$((copied + 1))
  done < "$MANIFEST"

  (( copied > 0 )) || die "Manifest produced 0 files"
  echo "Copied $copied proto files to: $DEST_DIR"
}

parse_steam_inf() {
  local inf_file="$REPO_DIR/$INF_PATH"
  need_file "$inf_file"

  local client server rev
  client="$(grep -E '^ClientVersion=' "$inf_file" | head -n1 | cut -d= -f2 | tr -d '\r')"
  server="$(grep -E '^ServerVersion=' "$inf_file" | head -n1 | cut -d= -f2 | tr -d '\r')"
  rev="$(grep -E '^SourceRevision=' "$inf_file" | head -n1 | cut -d= -f2 | tr -d '\r')"

  [[ "$client" =~ ^[0-9]+$ ]] || die "Invalid ClientVersion: '$client'"
  [[ "$server" =~ ^[0-9]+$ ]] || die "Invalid ServerVersion: '$server'"
  [[ "$rev"    =~ ^[0-9]+$ ]] || die "Invalid SourceRevision: '$rev'"

  echo "$client" "$server" "$rev"
}

extract_version_in_section() {
  local header="$1" toml="$2"
  awk -v header="$header" '
    BEGIN { in_section=0 }
    $0 == "[" header "]" { in_section=1; next }
    $0 ~ /^\[/ {
      if (in_section) exit
    }
    in_section && $0 ~ /^[[:space:]]*version[[:space:]]*=/ {
      if (match($0, /"[^"]+"/)) {
        s = substr($0, RSTART+1, RLENGTH-2)
        print s
        exit
      }
    }
  ' "$toml"
}

update_version_in_section() {
  local header="$1" toml="$2" new_version="$3"
  local tmp
  tmp="$(mktemp)"

  awk -v header="$header" -v new="$new_version" '
    BEGIN { in_section=0; done=0 }
    $0 == "[" header "]" { in_section=1 }
    $0 ~ /^\[/ {
      if (in_section && $0 != "[" header "]") in_section=0
    }
    {
      if (!done && in_section && $0 ~ /^[[:space:]]*version[[:space:]]*=/) {
        if ($0 ~ /"[^"]*"/) {
          sub(/"[^"]*"/, "\"" new "\"")
          done=1
        }
      }
      print
    }
    END {
      if (!done) exit 3
    }
  ' "$toml" > "$tmp" || {
    rm -f "$tmp"
    die "Could not update version in section [$header] in $toml (expected version = \"...\")"
  }

  mv -f "$tmp" "$toml"
}

upsert_metadata_version() {
  # Upsert:
  #   [package.metadata.boon-proto]
  #   version = "<full>"
  #
  # Args:
  #   $1 = toml file path
  #   $2 = full version string
  local toml="$1" full="$2"
  local hdr="[package.metadata.boon-proto]"
  local tmp
  tmp="$(mktemp)"

  awk -v hdr="$hdr" -v full="$full" '
    BEGIN { in_meta=0; found=0; wrote=0 }
    $0 == hdr { in_meta=1; found=1; print; next }
    $0 ~ /^\[/ {
      if (in_meta) {
        if (!wrote) { print "version = \"" full "\""; wrote=1 }
        in_meta=0
      }
      print
      next
    }
    in_meta && $0 ~ /^[[:space:]]*version[[:space:]]*=/ {
      if ($0 ~ /"[^"]*"/) {
        sub(/"[^"]*"/, "\"" full "\"")
      } else {
        $0 = "version = \"" full "\""
      }
      wrote=1
      print
      next
    }
    { print }
    END {
      if (in_meta && !wrote) {
        print "version = \"" full "\""
        wrote=1
      }
      if (!found) {
        print ""
        print hdr
        print "version = \"" full "\""
      }
    }
  ' "$toml" > "$tmp" || {
    rm -f "$tmp"
    die "Failed to update $hdr in $toml"
  }

  mv -f "$tmp" "$toml"
}

update_cargo_toml() {
  local client="$1" server="$2" rev="$3"

  # boon-proto uses version.workspace = true, so read from the workspace root
  local workspace_toml="$SCRIPT_DIR/../Cargo.toml"
  need_file "$workspace_toml"

  local current_version
  current_version="$(extract_version_in_section "workspace.package" "$workspace_toml")"
  [[ -n "$current_version" ]] || die "Could not find quoted version in [workspace.package] in $workspace_toml"

  if [[ ! "$current_version" =~ ([0-9]+)\.([0-9]+)\.([0-9]+) ]]; then
    die "Existing version '$current_version' does not contain MAJOR.MINOR.PATCH"
  fi

  local major="${BASH_REMATCH[1]}"
  local minor="${BASH_REMATCH[2]}"
  local patch="${BASH_REMATCH[3]}"

  local full_version="${major}.${minor}.${patch}.${client}.${server}.${rev}"

  upsert_metadata_version "$CARGO_TOML" "$full_version"

  echo "Updated $CARGO_TOML ([package.metadata.boon-proto].version): -> $full_version"
}

main() {
  clone_repo
  copy_protos

  read -r client server rev < <(parse_steam_inf)
  update_cargo_toml "$client" "$server" "$rev"
}

main "$@"
