#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${DATA_DIR:-$SCRIPT_DIR/../data}"
LAST_SEQUENCE_FILE="$SCRIPT_DIR/last_sequence.txt"
STATE_URL="https://download.geofabrik.de/asia/japan-updates/state.txt"
BASE_URL="https://download.geofabrik.de/asia/japan-updates"
PBF_PATH="$DATA_DIR/japan-vending.osm.pbf"
NEXT_PBF_PATH="$DATA_DIR/japan-vending-new.osm.pbf"
IMPORT_SCRIPT="$SCRIPT_DIR/import_pbf.py"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

read_sequence_from_state() {
  curl -fsSL "$1" | awk -F= '/^sequenceNumber=/{print $2}'
}

format_sequence_path() {
  local sequence padded
  sequence="$1"
  padded="$(printf "%09d" "$sequence")"
  printf "%s/%s/%s.osc.gz" "${padded:0:3}" "${padded:3:3}" "${padded:6:3}"
}

require_command curl
require_command osmium
require_command python3

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL must be set" >&2
  exit 1
fi

mkdir -p "$DATA_DIR"

if [[ ! -f "$PBF_PATH" ]]; then
  echo "Base PBF not found: $PBF_PATH" >&2
  exit 1
fi

echo "Fetching current Geofabrik sequence from $STATE_URL"
CURRENT_SEQUENCE="$(read_sequence_from_state "$STATE_URL")"
if [[ -z "$CURRENT_SEQUENCE" ]]; then
  echo "Failed to read current sequence number" >&2
  exit 1
fi

if [[ -f "$LAST_SEQUENCE_FILE" ]]; then
  LAST_SEQUENCE="$(tr -d '[:space:]' < "$LAST_SEQUENCE_FILE")"
  echo "Resuming from saved sequence: $LAST_SEQUENCE"
else
  LAST_SEQUENCE="$((CURRENT_SEQUENCE - 1))"
  echo "No saved sequence found. Initializing from current sequence minus one: $LAST_SEQUENCE"
fi

if (( LAST_SEQUENCE >= CURRENT_SEQUENCE )); then
  echo "No diffs to apply. Current sequence is $CURRENT_SEQUENCE"
else
  for ((sequence = LAST_SEQUENCE + 1; sequence <= CURRENT_SEQUENCE; sequence++)); do
    RELATIVE_PATH="$(format_sequence_path "$sequence")"
    DIFF_URL="$BASE_URL/$RELATIVE_PATH"
    DIFF_PATH="$DATA_DIR/$(basename "$RELATIVE_PATH")"

    echo "Downloading diff $sequence from $DIFF_URL"
    curl -fsSL "$DIFF_URL" -o "$DIFF_PATH"

    echo "Applying diff $sequence to $PBF_PATH"
    osmium apply-changes "$PBF_PATH" "$DIFF_PATH" -o "$NEXT_PBF_PATH" --overwrite

    mv "$NEXT_PBF_PATH" "$PBF_PATH"
    rm -f "$DIFF_PATH"
    echo "Applied diff $sequence"
  done
fi

echo "Importing $PBF_PATH into PostgreSQL"
python3 "$IMPORT_SCRIPT" --pbf "$PBF_PATH" --db "$DATABASE_URL"

printf "%s\n" "$CURRENT_SEQUENCE" > "$LAST_SEQUENCE_FILE"
echo "Saved current sequence to $LAST_SEQUENCE_FILE"
echo "Sync complete"
