#!/usr/bin/env bash
# Workbench one-shot setup: clone -> ./setup.sh -> ./run.sh and you are live.
# Builds the Rust services and the workbench TUI. No Node build is required:
# frontend/ is kept only for DESIGN.md (the upstream React app is retired
# here). Everything runs fully local by default (SQLite file DB, keyless AI
# via any agent CLI on PATH); .env.example documents every optional knob.
set -euo pipefail
cd "$(dirname "$0")"

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

say "Checking prerequisites"
if command -v cargo >/dev/null 2>&1; then
  printf '  ok  cargo (%s)\n' "$(command -v cargo)"
else
  printf '  MISSING  cargo\n\nInstall Rust (https://rustup.rs), then re-run.\n'
  exit 1
fi

say "Detecting local agent CLIs (the keyless AI engine)"
found_agent=0
for agent in claude gemini codex opencode hermes antigravity openclaw; do
  if command -v "$agent" >/dev/null 2>&1; then
    printf '  ok  %s\n' "$agent"
    found_agent=1
  fi
done
if [ "$found_agent" = 0 ]; then
  echo "  none found - AI features will be disabled until you install one"
  echo "  (Claude Code, Gemini CLI, OpenCode, ...) or set ANTHROPIC_API_KEY."
fi

say "Building Rust services (lifeos-api + lifeos-drain, release)"
(cd services && cargo build --release -p lifeos-api -p lifeos-drain)

say "Building the workbench (release)"
(cd workbench && cargo build --release)

say "Done"
cat <<'EOF'

Next steps:
  ./run.sh                # start the workbench (lifeos-api linked in-process)
  ./run.sh --with-drain   # also start the heavy-lane job worker

The API auto-creates and migrates lifeos.db on first boot - no manual DB
setup. Optional integrations (Turso sync, Telegram bot, Nango OAuth, ...)
are documented in .env.example and docs/MANUAL-SETUP.md.
EOF
