#!/usr/bin/env bash
# Start the Life OS Workbench: the workbench binary links lifeos-api
# in-process, so nothing else is needed for the app itself. Optionally also
# start the heavy-lane drain worker (--with-drain) and/or the 127.0.0.1 HTTP
# API for external consumers (--with-http). Run ./setup.sh once first.
set -euo pipefail
cd "$(dirname "$0")"

# Load .env if present (all vars optional - see .env.example).
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

pick_bin() {
  for candidate in "$2/target/release/$1" "$2/target/debug/$1"; do
    if [ -x "$candidate" ]; then echo "$candidate"; return; fi
  done
}
WORKBENCH_BIN="$(pick_bin workbench workbench)"
API_BIN="$(pick_bin lifeos-api services)"
DRAIN_BIN="$(pick_bin lifeos-drain services)"
if [ -z "$WORKBENCH_BIN" ]; then
  echo "workbench not built yet - run ./setup.sh first." >&2
  exit 1
fi

pids=()
cleanup() {
  for pid in "${pids[@]:-}"; do kill "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT INT TERM

for arg in "$@"; do
  case "$arg" in
    --with-drain)
      if [ -n "$DRAIN_BIN" ]; then
        echo "== lifeos-drain (heavy-lane job worker)"
        "$DRAIN_BIN" &
        pids+=("$!")
      else
        echo "lifeos-drain not built - run ./setup.sh first." >&2
      fi
      ;;
    --with-http)
      if [ -n "$API_BIN" ]; then
        echo "== lifeos-api HTTP server for external consumers on ${LIFEOS_BIND_ADDR:-127.0.0.1:8080}"
        "$API_BIN" &
        pids+=("$!")
      else
        echo "lifeos-api not built - run ./setup.sh first." >&2
      fi
      ;;
  esac
done

echo "== workbench (lifeos-api in-process)"
"$WORKBENCH_BIN"
