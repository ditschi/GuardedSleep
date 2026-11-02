#!/usr/bin/env bash
# Shared handling of "SKIPPED: ..." lines (AGENTS.md section 4): a skip is not a pass, so `just ci` lists
# every skip of its run at the end. Source it:
#   source scripts/lib/skip-log.sh
#   gs_skip "e2e needs playwright-browsers"     # prints "SKIPPED: e2e needs ..." and, when
#                                               # $GS_SKIP_LOG is set, appends the line to that file
#   gs_skip_summary "$GS_SKIP_LOG"              # prints the distinct lines, or "No tasks skipped."
# scripts/lib/ci.sh creates and exports GS_SKIP_LOG; outside `just ci` the variable is unset and
# gs_skip only prints.

gs_skip() {
  echo "SKIPPED: $*"
  if [ -n "${GS_SKIP_LOG:-}" ]; then echo "SKIPPED: $*" >>"$GS_SKIP_LOG" 2>/dev/null || true; fi
  return 0
}

gs_skip_summary() {
  local log="${1:-}"
  if [ -n "$log" ] && [ -s "$log" ]; then
    echo ""
    echo "Skipped tasks (not a pass):"
    awk '!seen[$0]++ { print "  " $0 }' "$log"
  else
    echo ""
    echo "No tasks skipped."
  fi
}
