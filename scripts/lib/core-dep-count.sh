#!/usr/bin/env bash
# Dependency-count guard for guardedsleep-core (ADR-020 §3), counted on the all-features graph (the
# optional tsify feature included, see docs/ci-gates.md). The number of unique normal dependencies
# (including the crate itself) must equal the number committed in .core-dep-count. A new dependency
# therefore shows up as a one-line diff that has to be justified in review.
#   scripts/lib/core-dep-count.sh          # check
#   scripts/lib/core-dep-count.sh --bless  # rewrite .core-dep-count
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
actual="$(scripts/cargo.sh tree -p guardedsleep-core --all-features -e normal --prefix none | sort -u | wc -l | tr -d ' ')"
if [ "${1:-}" = "--bless" ]; then
  echo "$actual" > .core-dep-count
  echo "core-dep-count: wrote $actual"
  exit 0
fi
expected="$(tr -d ' \n' < .core-dep-count)"
if [ "$actual" != "$expected" ]; then
  echo "core-dep-count: guardedsleep-core has $actual unique dependencies, .core-dep-count says $expected." >&2
  echo "Justify the change in the PR, then run: scripts/lib/core-dep-count.sh --bless" >&2
  exit 1
fi
echo "core-dep-count: $actual (ok)"
