#!/usr/bin/env bash
# Local-only warning when git hooks are off (AGENTS.md section 10.3). Called first by `just lint-text`.
#   scripts/lib/hooks-check.sh
#
# Warns (stderr) if the hooks path (core.hooksPath and linked worktrees respected) points at a stale
# Husky directory, or if it has no executable pre-commit hook that mentions lefthook. Not a gate:
# always exits 0, and stays silent in CI (CI is non-empty) or outside a git work tree.
set -euo pipefail

[ -z "${CI:-}" ] || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

hooks="$(git rev-parse --path-format=absolute --git-path hooks)"
tail_msg="Commits skip formatting, typos and the secret scan until then."

if [[ "$hooks" == *".husky"* ]]; then
  echo "warning: git hooks are off: core.hooksPath points at a stale Husky directory ($hooks); run 'just setup'. $tail_msg" >&2
elif [ ! -x "$hooks/pre-commit" ] || ! grep -q lefthook "$hooks/pre-commit"; then
  echo "warning: git hooks are off: no lefthook pre-commit hook in $hooks; run 'just setup'. $tail_msg" >&2
fi
exit 0
