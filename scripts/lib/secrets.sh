#!/usr/bin/env bash
# Secret scan with Betterleaks (.betterleaks.toml). Live validation stays off: never pass --validation.
#   scripts/lib/secrets.sh            # full history of the current branch (HEAD); other local branches and agent worktrees are not scanned
#   scripts/lib/secrets.sh --staged   # staged changes only (lefthook pre-commit)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=require.sh
source scripts/lib/require.sh

if ! command -v betterleaks >/dev/null 2>&1; then
  echo "betterleaks not found: run 'just setup' (mise install)" >&2
  exit 1
fi
common=(--config .betterleaks.toml --no-banner --redact --log-level warn)

if [ "${1:-}" = "--staged" ]; then
  exec betterleaks git "${common[@]}" --staged .
fi
betterleaks git "${common[@]}" --log-opts HEAD .
