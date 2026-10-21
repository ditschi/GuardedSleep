#!/usr/bin/env bash
# Every workspace crate must inherit the workspace lints (unsafe_code = "deny" etc., ADR-020 section 3),
# otherwise a new crate silently opts out of them.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

status=0
for manifest in crates/*/Cargo.toml; do
  if ! grep -Pzq '\[lints\]\s*\nworkspace\s*=\s*true' "$manifest"; then
    echo "workspace-lints: $manifest: add [lints] workspace = true" >&2
    status=1
  fi
done
exit "$status"
