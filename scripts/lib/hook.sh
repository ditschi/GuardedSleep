#!/usr/bin/env bash
# Runs one hook command with the mise-pinned tools on PATH (git hooks do not inherit `mise activate`).
#   scripts/lib/hook.sh typos FILE...
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=require.sh
source scripts/lib/require.sh
exec "$@"
