#!/usr/bin/env bash
# `just ci`: lint, test, build and e2e in order (the same verdict as the plain dependency chain: the first
# failing step stops the run and sets the exit status), then a summary of every "SKIPPED: ..." line the
# steps printed. The summary also appears when a step fails. GS_STRICT and the slots are untouched: the
# steps are ordinary `just` recipes. The emitters (scripts/lib/require.sh, e2e.sh, ktfmt.sh) append to
# $GS_SKIP_LOG through scripts/lib/skip-log.sh.
#   scripts/lib/ci.sh [just-executable]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
# shellcheck source=skip-log.sh
source scripts/lib/skip-log.sh

just_bin="${1:-just}"
GS_SKIP_LOG="$(mktemp "${TMPDIR:-/tmp}/gs-skip-log.XXXXXX")"
export GS_SKIP_LOG

finish() {
  local rc=$?
  trap - EXIT
  gs_skip_summary "$GS_SKIP_LOG"
  rm -f "$GS_SKIP_LOG"
  exit "$rc"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

for step in lint test build e2e; do
  "$just_bin" "$step" || exit $?
done
