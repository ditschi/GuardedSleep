#!/usr/bin/env bash
# Lists leftovers of earlier runs (AGENTS.md section 10.2); with --stop it ends them.
#   scripts/lib/reap.sh          # list only
#   scripts/lib/reap.sh --stop   # TERM, KILL after 5 s, docker stop
# Candidates:
#   - processes running `just dev`, scripts/dev.sh, vite, guardedsleep-server or tsx whose working
#     directory or command line is inside this repository (the main checkout or any worktree of it);
#   - stuck agent polls: shells started from ~/.claude/shell-snapshots whose command line is an
#     until/while loop with sleep, older than 30 minutes;
#   - Docker containers whose name or image contains "guardedsleep", older than 2 hours.
# Nothing outside this repository and no unrelated container is ever touched.
set -euo pipefail

stop=0
case "${1:-}" in
  "") ;;
  --stop) stop=1 ;;
  *)
    echo "usage: reap.sh [--stop]" >&2
    exit 2
    ;;
esac

here="$(cd "$(dirname "$0")/../.." && pwd)"
common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
repo="${common%/.git}"
[ -n "$repo" ] && [ "$repo" != "$common" ] || repo="$here"

# This script and its ancestors (just, the shell) must survive.
protected=" $$ "
pid=$$
while [ "$pid" -gt 1 ]; do
  pid="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')" || break
  [ -n "$pid" ] || break
  protected+="$pid "
done

inside_repo() {
  local cwd
  cwd="$(readlink "/proc/$1/cwd" 2>/dev/null || true)"
  case "$cwd/" in "$repo"/*) return 0 ;; esac
  case "$2" in *"$repo"*) return 0 ;; esac
  return 1
}

dev_re='(just dev|scripts/dev\.sh|vite|guardedsleep-server|tsx)'
poll_re='(until|while).*sleep'
found_pids=()
found_containers=()

while read -r p age args; do
  case "$protected" in *" $p "*) continue ;; esac
  if [[ "$args" =~ $dev_re ]] && inside_repo "$p" "$args"; then
    echo "process $p (${age}s): $args"
    found_pids+=("$p")
  elif [[ "$args" == *shell-snapshots* ]] && [[ "$args" =~ $poll_re ]] && [ "$age" -gt 1800 ]; then
    echo "stuck poll $p (${age}s): ${args:0:160}"
    found_pids+=("$p")
  fi
done < <(ps -eo pid=,etimes=,args=)

if command -v docker >/dev/null 2>&1; then
  now="$(date +%s)"
  while IFS=$'\t' read -r id name image; do
    [ -n "$id" ] || continue
    created="$(docker inspect -f '{{.Created}}' "$id" 2>/dev/null || true)"
    [ -n "$created" ] || continue
    age=$((now - $(date -d "$created" +%s)))
    if [ "$age" -gt 7200 ]; then
      echo "container $id ($name, $image, ${age}s)"
      found_containers+=("$id")
    fi
  done < <(docker ps --filter name=guardedsleep --format '{{.ID}}\t{{.Names}}\t{{.Image}}' 2>/dev/null
    docker ps --filter ancestor=guardedsleep --format '{{.ID}}\t{{.Names}}\t{{.Image}}' 2>/dev/null
    docker ps --format '{{.ID}}\t{{.Names}}\t{{.Image}}' 2>/dev/null | grep -i guardedsleep || true)
fi

if [ ${#found_pids[@]} -eq 0 ] && [ ${#found_containers[@]} -eq 0 ]; then
  echo "nothing to reap"
  exit 0
fi
if [ "$stop" -eq 0 ]; then
  echo "listed only; run 'just reap --stop' to end them"
  exit 0
fi

if [ ${#found_pids[@]} -gt 0 ]; then
  kill -TERM "${found_pids[@]}" 2>/dev/null || true
  sleep 5
  kill -KILL "${found_pids[@]}" 2>/dev/null || true
fi
if [ ${#found_containers[@]} -gt 0 ]; then
  mapfile -t found_containers < <(printf '%s\n' "${found_containers[@]}" | sort -u)
  docker stop "${found_containers[@]}" >/dev/null
fi
echo "stopped"
