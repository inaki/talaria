#!/bin/sh
# Diagnose Hermes Python, then run the no-model protocol dump.
# Does not spawn tui_gateway.entry as a probe — cargo dump_gateway uses
# import-only discovery.
set -e
cd "$(dirname "$0")/.."

echo "=== diagnose ==="
if command -v hermes >/dev/null 2>&1; then
  echo "hermes: $(command -v hermes)"
  echo "shebang: $(head -1 "$(command -v hermes)" 2>/dev/null || true)"
else
  echo "hermes: not on PATH"
fi
HOME_HERMES="${HERMES_HOME:-$HOME/.hermes}"
echo "HERMES_HOME: $HOME_HERMES"
if [ -x "$HOME_HERMES/hermes-agent/venv/bin/python" ]; then
  echo "venv python: $HOME_HERMES/hermes-agent/venv/bin/python"
elif [ -x "$HOME_HERMES/hermes-agent/.venv/bin/python" ]; then
  echo "venv python: $HOME_HERMES/hermes-agent/.venv/bin/python"
else
  echo "venv python: missing (no $HOME_HERMES/hermes-agent/venv)"
fi
if [ -n "${HERMES_PYTHON:-}" ]; then
  echo "HERMES_PYTHON=$HERMES_PYTHON"
fi

if ! command -v hermes >/dev/null 2>&1 && [ ! -d "$HOME_HERMES/hermes-agent" ]; then
  cat <<EOF

Hermes is not installed in this environment.

Install (then reload the shell):
  curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash

If hermes already works in another terminal, copy that interpreter:
  command -v hermes
  head -1 "\$(command -v hermes)"
  export HERMES_PYTHON=...   # the #! path, not /usr/bin/env
  ./scripts/live-check.sh
EOF
  exit 2
fi

echo
echo "=== dump_gateway (Rust discovery, import probe only) ==="
exec cargo run --example dump_gateway
