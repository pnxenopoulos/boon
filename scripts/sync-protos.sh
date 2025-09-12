#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------
# Requirements
#   git
#   sha256sum or shasum
# ---------------------------------------

# ---------------------------------------
# Usage
#   ./scripts/sync-protos.sh \
#     --workspace-root /path/to/workspace/root \
#     --manifest /path/to/workspace/root/crates/boon-proto/proto/manifest.txt \
#     --local-dir /path/to/workspace/root/crates/boon-proto/proto
#
#  or just
#  
#    ./scripts/sync-protos.sh
# ---------------------------------------

# ---------------------------------------
# Locate workspace root (assumes this file lives in <workspace>/scripts/)
# ---------------------------------------
SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
# Default workspace root is parent of scripts/
DEFAULT_WS_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Allow override via env/flag
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$DEFAULT_WS_ROOT}"

# ---------------------------------------
# Config (override via env or flags)
# ---------------------------------------
REPO_URL="${REPO_URL:-https://github.com/SteamDatabase/GameTracking-Deadlock.git}"

# New defaults pointing at crates/boon-proto/proto/
MANIFEST_PATH="${MANIFEST_PATH:-$WORKSPACE_ROOT/crates/boon-proto/proto/manifest.txt}"  # local manifest path
LOCAL_PROTO_DIR="${LOCAL_PROTO_DIR:-$WORKSPACE_ROOT/crates/boon-proto/proto}"           # local *.proto path

# Flags (optional):
#   --manifest <path>        override MANIFEST_PATH
#   --local-dir <path>       override LOCAL_PROTO_DIR
#   --repo-url <url>         override REPO_URL
#   --workspace-root <path>  set WORKSPACE_ROOT (affects default paths)
while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest) MANIFEST_PATH="$2"; shift 2 ;;
    --local-dir) LOCAL_PROTO_DIR="$2"; shift 2 ;;
    --repo-url) REPO_URL="$2"; shift 2 ;;
    --workspace-root) WORKSPACE_ROOT="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

# ---------------------------------------
# Utilities
# ---------------------------------------
hash_file() {
  local f="$1"
  if [[ ! -f "$f" ]]; then
    echo "MISSING"
    return 0
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    echo "Error: neither sha256sum nor shasum found in PATH" >&2
    exit 1
  fi
}

trim() { awk '{$1=$1;print}'; }

# ---------------------------------------
# Pre-flight checks
# ---------------------------------------
if [[ ! -f "$MANIFEST_PATH" ]]; then
  echo "Error: manifest not found at '$MANIFEST_PATH'." >&2
  exit 1
fi

# ---------------------------------------
# Clone repo into a temp dir
# ---------------------------------------
WORKDIR="$(mktemp -d)"
cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

echo "Cloning $REPO_URL ..."
git clone --depth=1 "$REPO_URL" "$WORKDIR/repo" >/dev/null 2>&1

REPO_PROTO_DIR="$WORKDIR/repo/Protobufs"
if [[ ! -d "$REPO_PROTO_DIR" ]]; then
  echo "Error: expected directory '$REPO_PROTO_DIR' not found in repo." >&2
  exit 1
fi

# ---------------------------------------
# Process manifest
# ---------------------------------------
echo "Reading manifest: $MANIFEST_PATH"
echo "Local proto dir: $LOCAL_PROTO_DIR"
echo "Repo proto dir : $REPO_PROTO_DIR"
echo

UPDATED=0
ADDED=0
UNCHANGED=0
MISSING_IN_REPO=0

declare -a CHANGES
declare -a ADDS
declare -a UNCHANGED_LIST
declare -a MISSING_LIST

while IFS= read -r line || [[ -n "$line" ]]; do
  entry="$(echo "$line" | sed 's/\r//g' | trim)"
  [[ -z "$entry" ]] && continue
  [[ "$entry" =~ ^# ]] && continue

  repo_file="$REPO_PROTO_DIR/$entry"
  local_file="$LOCAL_PROTO_DIR/$entry"

  repo_hash="$(hash_file "$repo_file")"
  if [[ "$repo_hash" == "MISSING" ]]; then
    MISSING_IN_REPO=$((MISSING_IN_REPO+1))
    MISSING_LIST+=("$entry  [missing in upstream repo]")
    echo "SKIP  $entry  (missing in upstream repo)"
    continue
  fi

  local_hash="$(hash_file "$local_file")"
  if [[ "$local_hash" == "MISSING" ]]; then
    mkdir -p "$(dirname "$local_file")"
    cp -f "$repo_file" "$local_file"
    ADDED=$((ADDED+1))
    ADDS+=("$entry")
    echo "ADD   $entry"
    continue
  fi

  if [[ "$repo_hash" != "$local_hash" ]]; then
    mkdir -p "$(dirname "$local_file")"
    cp -f "$repo_file" "$local_file"
    UPDATED=$((UPDATED+1))
    CHANGES+=("$entry")
    echo "UPDATE $entry"
  else
    UNCHANGED=$((UNCHANGED+1))
    UNCHANGED_LIST+=("$entry")
    echo "OK    $entry"
  fi
done < "$MANIFEST_PATH"

# ---------------------------------------
# Summary
# ---------------------------------------
echo
echo "Summary:"
echo "  Updated : $UPDATED"
echo "  Added   : $ADDED"
echo "  Unchanged: $UNCHANGED"
echo "  Missing in upstream: $MISSING_IN_REPO"
echo

if (( UPDATED > 0 )); then
  echo "Updated files:"
  for f in "${CHANGES[@]}"; do echo "  - $f"; done
  echo
fi

if (( ADDED > 0 )); then
  echo "Added files:"
  for f in "${ADDS[@]}"; do echo "  - $f"; done
  echo
fi

if (( MISSING_IN_REPO > 0 )); then
  echo "Missing in upstream (skipped):"
  for f in "${MISSING_LIST[@]}"; do echo "  - $f"; done
  echo
fi
