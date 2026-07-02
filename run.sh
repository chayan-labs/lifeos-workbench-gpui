#!/usr/bin/env bash
# Start Life OS locally: the Rust API (auto-migrating SQLite DB) + the React
# dev server, and optionally the heavy-lane drain worker (--with-drain).
# Run ./setup.sh once first. Ctrl-C stops everything.
set -euo pipefail
cd "$(dirname "$0")"

# Load .env if present (all vars optional - see .env.example).
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

# Prefer the release build from setup.sh; fall back to a dev (debug) build.
pick_bin() {
  for candidate in "services/target/release/$1" "services/target/debug/$1"; do
    if [ -x "$candidate" ]; then echo "$candidate"; return; fi
  done
}
API_BIN="$(pick_bin lifeos-api)"
DRAIN_BIN="$(pick_bin lifeos-drain)"
if [ -z "$API_BIN" ]; then
  echo "lifeos-api not built yet - run ./setup.sh first." >&2
  exit 1
fi

pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT INT TERM

echo "== lifeos-api on ${LIFEOS_BIND_ADDR:-127.0.0.1:8080}"
"$API_BIN" &
pids+=("$!")

if [ "${1:-}" = "--with-drain" ]; then
  if [ -n "$DRAIN_BIN" ]; then
    echo "== lifeos-drain (heavy-lane job worker)"
    "$DRAIN_BIN" &
    pids+=("$!")
  else
    echo "lifeos-drain not built - run ./setup.sh first." >&2
  fi
fi

echo "== frontend dev server (Vite)"
(cd frontend && npm run dev) &
pids+=("$!")

wait
