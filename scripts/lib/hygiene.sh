#!/usr/bin/env bash
# Text hygiene that the old pre-commit config covered: trailing whitespace, final newline, CRLF,
# merge-conflict markers, files over 800 KB and JSON validity. YAML validity is covered by
# `prettier --check`, which fails on syntax errors.
#   scripts/lib/hygiene.sh                # all tracked files
#   scripts/lib/hygiene.sh FILE...        # only the given files
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
MAX_BYTES=$((800 * 1024))

if [ $# -gt 0 ]; then
  files=("$@")
else
  mapfile -d '' files < <(git ls-files -z)
fi

status=0
fail() { echo "hygiene: $1: $2" >&2; status=1; }

for f in "${files[@]}"; do
  [ -f "$f" ] && [ ! -L "$f" ] || continue
  size="$(wc -c < "$f")"
  if [ "$size" -gt "$MAX_BYTES" ]; then
    fail "$f" "larger than 800 KB ($size bytes)"
  fi
  [ "$size" -gt 0 ] || continue
  # Binary files (png, jar, ...) have no further text checks.
  grep -Iq . "$f" || continue

  first="$(LC_ALL=C grep -nE '[[:blank:]]+$' "$f" | head -1 | cut -d: -f1 || true)"
  if [ -n "$first" ]; then
    fail "$f" "trailing whitespace (line $first)"
  fi
  if LC_ALL=C grep -q $'\r' "$f"; then
    fail "$f" "CRLF line ending (use LF)"
  fi
  if [ -n "$(tail -c1 "$f")" ]; then
    fail "$f" "no newline at end of file"
  elif [ "$(tail -c2 "$f" | od -An -c | tr -d ' ')" = '\n\n' ]; then
    fail "$f" "blank line at end of file"
  fi
  if grep -nE '^(<<<<<<<|>>>>>>>) ' "$f" >/dev/null; then
    fail "$f" "merge conflict marker"
  fi
  case "$f" in
    *.json)
      node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$f" 2>/dev/null \
        || fail "$f" "invalid JSON"
      ;;
  esac
done
exit "$status"
