#!/usr/bin/env bash
# Summarises the run timings that slot.sh appends (AGENTS.md section 10.1).
#   scripts/lib/timings.sh [days]     # default 7
# File: ${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}/timings.tsv, tab separated, no header:
#   time (ISO UTC), worktree, class, command, wait_s, run_s, exit, nested|top
# Output per command, sorted by total run time: runs, median/p90/max run seconds, total wait
# seconds, failures (exit status other than 0).
set -euo pipefail

days="${1:-7}"
case "$days" in '' | *[!0-9]*)
  echo "usage: timings.sh [days]" >&2
  exit 2
  ;;
esac
file="${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}/timings.tsv"
if [ ! -s "$file" ]; then
  echo "no timings yet ($file)"
  exit 0
fi
cutoff="$(date -u -d "$days days ago" +%Y-%m-%dT%H:%M:%SZ)"

printf 'runs\tmedian_s\tp90_s\tmax_s\ttotal_run_s\twait_s\tfails\tcommand\n'
awk -F'\t' -v cutoff="$cutoff" '
  NF >= 8 && $1 >= cutoff {
    c = $4; n[c]++; run[c, n[c]] = $6; total[c] += $6; wait[c] += $5
    if ($7 != 0) fails[c]++
  }
  END {
    for (c in n) {
      k = n[c]
      for (i = 1; i <= k; i++) a[i] = run[c, i] + 0
      for (i = 2; i <= k; i++) { v = a[i]; j = i - 1; while (j > 0 && a[j] > v) { a[j + 1] = a[j]; j-- } a[j + 1] = v }
      med = (k % 2) ? a[(k + 1) / 2] : (a[k / 2] + a[k / 2 + 1]) / 2
      p = int(0.9 * k); if (p < 0.9 * k) p++; if (p < 1) p = 1
      printf "%d\t%g\t%d\t%d\t%d\t%d\t%d\t%s\n", k, med, a[p], a[k], total[c], wait[c], fails[c], c
    }
  }' "$file" | sort -t$'\t' -k5,5nr
