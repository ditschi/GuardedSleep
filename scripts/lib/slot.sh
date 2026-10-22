#!/usr/bin/env bash
# Machine-wide counting semaphore for load classes (AGENTS.md section 10.1).
#   scripts/lib/slot.sh <light|medium|heavy> -- <cmd...>
#
# Slots are lock files /tmp/guardedsleep-slots/<class>.<i>.lock held with flock; the number of slots
# per class comes from .cache/machine-profile.env (GS_MEDIUM_SLOTS, GS_HEAVY_SLOTS), generated on
# demand by machine-profile.sh. Environment variables override the file. All worktrees and sessions
# on the machine share the same pools. The lock is released when the command ends or this script is
# killed (the flock fd closes).
#
# Behaviour: try every slot of the class without blocking. If none is free, print
# "waiting for a <class> slot (N in use)" once and block on one random slot for at most
# GS_SLOT_WAIT seconds (default 3600); on timeout exit with status 75 and an error message.
# `light` never waits: the command is just executed.
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

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PROFILE="$ROOT/.cache/machine-profile.env"
[ -f "$PROFILE" ] || "$ROOT/scripts/lib/machine-profile.sh"
overrides="$(for v in GS_MEDIUM_SLOTS GS_HEAVY_SLOTS; do
  if [ -n "${!v:-}" ]; then printf 'export %s=%q\n' "$v" "${!v}"; fi
done)"
# shellcheck source=/dev/null
source "$PROFILE"
eval "$overrides"

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
"$@" <&0 &
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
exit "$status"
