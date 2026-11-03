#!/usr/bin/env bash
# Machine-wide counting semaphore for load classes (AGENTS.md section 10.1).
#   scripts/lib/slot.sh <light|medium|heavy> -- <cmd...>
#
# Slots are lock files /tmp/guardedsleep-slots/<class>.<i>.lock held with flock; the number of slots
# per class is GS_MEDIUM_SLOTS (default 2) and GS_HEAVY_SLOTS (default 1), positive integers; set
# GS_MEDIUM_SLOTS=1 on machines below 8 cores or 12 GB (docs/contributing/development.md, "Load
# slots"). All worktrees and sessions on the machine share the same pools. The lock is released
# when the command ends or this script is killed (the flock fd closes).
#
# Behaviour: try every slot of the class without blocking. If none is free, print
# "waiting for a <class> slot (N in use)" once and block on one random slot for at most
# GS_SLOT_WAIT seconds (default 3600); on timeout exit with status 75 and an error message.
# `light` never waits: the command is just executed.
#
# Run deadline: if GS_RUN_TIMEOUT is set (any GNU `timeout` duration, e.g. 900 or 15m), the command
# runs under `timeout --kill-after=30s` and the clock starts only AFTER the slot is acquired, so
# queueing time is not counted. Exit 124 means "ran too long", exit 75 means "waited too long for a
# slot" (GS_SLOT_WAIT). GS_RUN_TIMEOUT is cleared for the child so a nested acquisition does not
# start a second deadline. Light and re-entrant calls apply no deadline.
#
# Re-entrancy: GS_SLOT_HELD names the heaviest class this process tree already holds. A nested call
# for the same or a lighter class runs the command without acquiring. A nested heavy call inside a
# medium slot acquires a heavy slot as well: medium and heavy are separate pools, so a medium holder
# waiting for heavy cannot block other medium work and no cycle can form (heavy never waits for
# medium, because a nested medium inside heavy is free).
set -euo pipefail

usage() {
  echo "usage: slot.sh <light|medium|heavy> -- <cmd...>" >&2
  exit 2
}

[ $# -ge 3 ] && [ "$2" = "--" ] || usage
class="$1"
shift 2

rank() {
  case "$1" in
    light) echo 0 ;;
    medium) echo 1 ;;
    heavy) echo 2 ;;
    *) usage ;;
  esac
}
want="$(rank "$class")"

if [ "$want" -eq 0 ] || [ "$want" -le "$(rank "${GS_SLOT_HELD:-light}")" ]; then
  exec "$@"
fi

GS_MEDIUM_SLOTS="${GS_MEDIUM_SLOTS:-2}"
GS_HEAVY_SLOTS="${GS_HEAVY_SLOTS:-1}"
for v in GS_MEDIUM_SLOTS GS_HEAVY_SLOTS; do
  case "${!v}" in
    '' | *[!0-9]* | 0*)
      echo "error: $v must be a positive integer (got '${!v}')" >&2
      exit 2
      ;;
  esac
done

if [ "$class" = medium ]; then slots="$GS_MEDIUM_SLOTS"; else slots="$GS_HEAVY_SLOTS"; fi
dir=/tmp/guardedsleep-slots
mkdir -p "$dir"
chmod 1777 "$dir" 2>/dev/null || true

acquired=
for ((i = 0; i < slots; i++)); do
  exec 9>"$dir/$class.$i.lock"
  if flock -n 9; then
    acquired=1
    break
  fi
done

if [ -z "$acquired" ]; then
  echo "waiting for a $class slot ($slots in use)" >&2
  i=$((RANDOM % slots))
  exec 9>"$dir/$class.$i.lock"
  if ! flock -w "${GS_SLOT_WAIT:-3600}" 9; then
    echo "error: no $class slot became free within ${GS_SLOT_WAIT:-3600}s (GS_SLOT_WAIT); is a job stuck? try 'just reap'" >&2
    exit 75
  fi
fi

export GS_SLOT_HELD="$class"
run_timeout="${GS_RUN_TIMEOUT:-}"
unset GS_RUN_TIMEOUT
if [ -n "$run_timeout" ]; then
  timeout --kill-after=30s "$run_timeout" "$@" <&0 &
else
  "$@" <&0 &
fi
child=$!
# shellcheck disable=SC2329 # invoked through the trap
forward() { kill -TERM "$child" 2>/dev/null || true; }
trap forward TERM INT HUP
status=0
wait "$child" || status=$?
if kill -0 "$child" 2>/dev/null; then
  # wait was interrupted by a trapped signal; the child is still shutting down.
  wait "$child" || status=$?
fi
if [ "$status" -eq 124 ] && [ -n "$run_timeout" ]; then
  echo "error: $class run exceeded GS_RUN_TIMEOUT=$run_timeout (slot wait not counted)" >&2
fi
exit "$status"
