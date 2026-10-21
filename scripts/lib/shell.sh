#!/usr/bin/env bash
# Shell for single-line just recipes (`set shell := ["scripts/lib/shell.sh"]`): runs the recipe line
# with the tools pinned in mise.toml, or with plain bash and a hint when mise is not installed.
if command -v mise >/dev/null 2>&1; then
  exec mise exec -- bash -euo pipefail "$@"
fi
echo "hint: mise not found, using tools from PATH. Install mise (https://mise.jdx.dev) and run 'just setup' for the pinned versions." >&2
export GS_MISE_HINT_SHOWN=1
exec bash -euo pipefail "$@"
