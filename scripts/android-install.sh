#!/usr/bin/env bash
# Install the locally built debug APK on all connected Android phones (USB or Wi-Fi adb) and launch it.
# Usage (repo root): just install-android [--build] [--no-launch]
#   --build       run `just build-android` first (otherwise a missing APK is an error)
#   --no-launch   do not start the app after a successful install
# Environment:
#   GS_DEVICES  comma- or space-separated adb serials and/or host:port (default: every device in state "device").
#               host:port entries not yet listed by `adb devices` are connected first.
#   GS_APK      APK path (default: apps/android/app/build/outputs/apk/debug/guardedsleep-debug.apk)
# The same phone seen over USB and TCP is installed once (USB serial preferred). Never uninstalls anything.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/require.sh
source scripts/lib/require.sh

PKG="org.guardedsleep.app.alpha"
APK="${GS_APK:-$ROOT/apps/android/app/build/outputs/apk/debug/guardedsleep-debug.apk}"
BUILD=0
LAUNCH=1
for arg in "$@"; do
  case "$arg" in
    --build) BUILD=1 ;;
    --no-launch) LAUNCH=0 ;;
    *) echo "usage: $0 [--build] [--no-launch]" >&2; exit 2 ;;
  esac
done

require install-android adb

if [ "$BUILD" = "1" ]; then
  just build-android
fi
if [ ! -f "$APK" ]; then
  echo "APK not found: $APK" >&2
  echo "Build it first: just build-android (or pass --build)" >&2
  exit 1
fi

adbq() { timeout 15 adb "$@"; }

# Serials currently in state "device".
list_ready() { adbq devices 2>/dev/null | awk 'NR>1 && $2=="device" {print $1}'; }

declare -a candidates=()
if [ -n "${GS_DEVICES:-}" ]; then
  read -r -a wanted <<<"${GS_DEVICES//,/ }"
  for d in "${wanted[@]}"; do
    if [[ "$d" == *:* ]]; then
      # "connected to" also covers "already connected to"; failures read "failed to connect to".
      out="$(timeout 15 adb connect "$d" 2>&1 || true)"
      [[ "$out" == *"connected to"* ]] || echo "warning: adb connect $d failed: $out" >&2
    fi
    candidates+=("$d")
  done
else
  while IFS= read -r d; do [ -n "$d" ] && candidates+=("$d"); done < <(list_ready)
fi

[ "${#candidates[@]}" -gt 0 ] || gs_unmet install-android "a connected adb device"

# Deduplicate by hardware serial; USB serials (no ':') sort before host:port entries.
declare -a sorted=()
while IFS= read -r d; do sorted+=("$d"); done < <(
  for d in "${candidates[@]}"; do [[ "$d" == *:* ]] || echo "$d"; done
  for d in "${candidates[@]}"; do [[ "$d" == *:* ]] && echo "$d"; done
)
declare -a devices=()
seen=" "
for d in "${sorted[@]}"; do
  hw="$(adbq -s "$d" shell getprop ro.serialno 2>/dev/null | tr -d '\r\n' || true)"
  key="${hw:-$d}"
  if [[ "$seen" == *" $key "* ]]; then
    echo "skip $d: same phone as an already selected device ($key)"
    continue
  fi
  seen="$seen$key "
  devices+=("$d")
done

failed=0
for d in "${devices[@]}"; do
  model="$(adbq -s "$d" shell getprop ro.product.model 2>/dev/null | tr -d '\r\n' || true)"
  out="$(timeout 180 adb -s "$d" install -r "$APK" 2>&1 || true)"
  if [[ "$out" == *Success* ]]; then
    result="installed"
    if [ "$LAUNCH" = "1" ]; then
      adbq -s "$d" shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 \
        || result="installed (launch failed)"
    fi
  else
    failed=1
    # adb prints "Performing Streamed Install" after the failure line, so pick the reason explicitly.
    reason="$(printf '%s\n' "$out" | grep -m1 -E 'Failure|error|failed' || printf '%s' "$out" | tail -n 1)"
    result="FAILED: $reason"
    if [[ "$out" == *INSTALL_FAILED_UPDATE_INCOMPATIBLE* ]]; then
      echo "hint: $d has $PKG installed with a different signature (e.g. the GitHub dev APK)." >&2
      echo "hint: uninstall it with 'adb -s $d uninstall $PKG' (this deletes the pairings), then retry. Not done automatically." >&2
    fi
  fi
  echo "$d  ${model:-unknown model}  $result"
done
exit "$failed"
