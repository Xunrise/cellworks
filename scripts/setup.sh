#!/usr/bin/env bash
# Checks the tools Cellworks uses and builds a release binary.
set -euo pipefail
cd "$(dirname "$0")/.."

missing=0
need() { # tool, why, required
  if command -v "$1" >/dev/null 2>&1; then
    printf '  ok       %-9s %s\n' "$1" "$2"
  elif [ "$3" = required ]; then
    printf '  MISSING  %-9s %s\n' "$1" "$2"; missing=1
  else
    printf '  optional %-9s %s (not found)\n' "$1" "$2"
  fi
}

echo "Tools:"
need cargo   "build (install via https://rustup.rs)" required
need git     "git state, branches, worktrees" required
need nvim    "embedded editor (brew install neovim)" required
need claude  "servitor sessions (Claude Code CLI)" optional
need kubectl "live workloads in otc: views" optional
need openssl "Atlassian sign-in for jira: views" optional
need curl    "Atlassian sign-in for jira: views" optional
need herdr   "terminals as Herdr tabs" optional

if [ "$missing" = 1 ]; then
  echo "Install the missing tools above, then run this again." >&2
  exit 1
fi

echo
echo "Building (first build takes a few minutes)…"
cargo build --release

cat <<'MSG'

Done. Start it with:

  ./target/release/cellworks              # opens your most recent project
  ./target/release/cellworks ~/some/repo  # opens a specific one

In the app: ⌘K searches everything and adds projects, Esc steps back out, and the View bar at the
bottom controls what is drawn. See README.md for the rest.
MSG
