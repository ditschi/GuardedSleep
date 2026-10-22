#!/usr/bin/env bash
# Lists leftovers of earlier runs (AGENTS.md section 10.2); with --stop it ends them.
#   scripts/lib/reap.sh          # list only
#   scripts/lib/reap.sh --stop   # TERM, KILL after 5 s, docker stop
#   scripts/lib/reap.sh --worktrees         # list the other worktrees as removable or kept (and why)
#   scripts/lib/reap.sh --prune-worktrees   # remove exactly the removable ones
# Candidates:
#   - processes running `just dev`, scripts/dev.sh, vite, guardedsleep-server or tsx whose working
#     directory or command line is inside this repository (the main checkout or any worktree of it);
#   - stuck agent polls: shells started from ~/.claude/shell-snapshots whose command line is an
#     until/while loop with sleep, older than 30 minutes;
#   - Docker containers whose name or image contains "guardedsleep", older than 2 hours.
# Nothing outside this repository and no unrelated container is ever touched.
#
# Worktrees (design of add-affected-tests-and-timings, section 4): every worktree except the main
# checkout is `removable` when
#   - `git status --porcelain` is empty,
#   - its HEAD is an ancestor of some origin/* ref (the first such ref is reported; no git fetch is
#     run, so the answer reflects the last fetch), and
#   - it is unlocked, or locked by a pid (`pid NNN` in the lock reason) that no longer runs.
# Otherwise it is `kept` with the reason (dirty, not pushed, in use by pid N; a lock without a pid
# counts as in use). Pruning unlocks stale locks, runs `git worktree remove` (never --force), deletes
# the branch with `git branch -d` only for worktree-agent-* branches, then `git worktree prune`.
set -euo pipefail

stop=0
worktrees=0
prune=0
case "${1:-}" in
  "") ;;
  --stop) stop=1 ;;
  --worktrees) worktrees=1 ;;
  --prune-worktrees) worktrees=1 prune=1 ;;
  *)
    echo "usage: reap.sh [--stop | --worktrees | --prune-worktrees]" >&2
    exit 2
    ;;
esac

here="$(cd "$(dirname "$0")/../.." && pwd)"
common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
repo="${common%/.git}"
[ -n "$repo" ] && [ "$repo" != "$common" ] || repo="$here"

# Prints one line per worktree (not the main checkout) and, with prune=1, removes the removable ones.
worktree_report() {
  local path="" head="" branch="" locked="" first=1 count=0 line
  local -a entries=()
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      "worktree "*) path="${line#worktree }" head="" branch="" locked="" ;;
      "HEAD "*) head="${line#HEAD }" ;;
      "branch "*) branch="${line#branch refs/heads/}" ;;
      "locked"*) locked=1 ;;
      "")
        if [ -n "$path" ]; then
          if [ "$first" -eq 1 ]; then first=0; else entries+=("$path|$head|$branch|$locked"); fi
          path=""
        fi
        ;;
    esac
  done < <(git -C "$repo" worktree list --porcelain; echo)

  local entry w h b l gitdir lockfile pid contains reason state
  for entry in "${entries[@]}"; do
    IFS='|' read -r w h b l <<<"$entry"
    count=$((count + 1))
    state=removable
    reason=""
    stale_lock=""
    if [ -n "$l" ]; then
      gitdir="$(git -C "$w" rev-parse --path-format=absolute --git-dir 2>/dev/null || true)"
      lockfile="$gitdir/locked"
      pid=""
      [ -f "$lockfile" ] && pid="$(grep -oE 'pid [0-9]+' "$lockfile" | head -n1 | cut -d' ' -f2 || true)"
      if [ -z "$pid" ]; then
        state=kept reason="in use (locked without a pid)"
      elif kill -0 "$pid" 2>/dev/null; then
        state=kept reason="in use by pid $pid"
      else
        stale_lock="pid $pid is gone"
      fi
    fi
    if [ "$state" = removable ] && [ -n "$(git -C "$w" status --porcelain 2>/dev/null)" ]; then
      state=kept reason="dirty"
    fi
    if [ "$state" = removable ]; then
      contains="$(git -C "$repo" for-each-ref --contains "$h" --format='%(refname:short)' refs/remotes/origin | head -n1)"
      if [ -z "$contains" ]; then
        state=kept reason="not pushed (HEAD ${h:0:8} is in no origin/* ref)"
      else
        reason="contained in $contains"
        [ -z "$stale_lock" ] || reason+="; stale lock ($stale_lock)"
      fi
    fi
    echo "$state: $w [${b:-detached}] $reason"
    if [ "$prune" -eq 1 ] && [ "$state" = removable ]; then
      [ -z "$l" ] || git -C "$repo" worktree unlock "$w"
      if git -C "$repo" worktree remove "$w"; then
        case "$b" in
          worktree-agent-*) git -C "$repo" branch -d "$b" || echo "  branch $b kept (git branch -d refused)" ;;
        esac
        echo "  removed $w"
      else
        echo "  could not remove $w" >&2
      fi
    fi
  done
  [ "$count" -gt 0 ] || echo "no other worktrees"
  if [ "$prune" -eq 1 ]; then git -C "$repo" worktree prune; fi
  if [ "$prune" -eq 0 ] && [ "$count" -gt 0 ]; then echo "listed only; run 'just reap --prune-worktrees' to remove the removable ones"; fi
}

if [ "$worktrees" -eq 1 ]; then
  worktree_report
  exit 0
fi

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
