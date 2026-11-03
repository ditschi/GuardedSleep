#!/usr/bin/env bash
# Runs adb inside the emulator image on the host network, sharing the gate's adb key. With rootless
# Docker the host cannot reach the emulators' adb ports (see local-gate.sh), so use this instead.
#   scripts/poc/adb.sh devices
#   scripts/poc/adb.sh -s emulator-5554 logcat -d -s GS_POC:I
# Uses the shared ~/.android of the running gate (KEEP_ENV=1): GATE_ANDROID_HOME, default target/poc-android-home.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GATE_ANDROID_HOME="${GATE_ANDROID_HOME:-$ROOT/target/poc-android-home}"
EMU_IMAGE="${EMULATOR_IMAGE:-guardedsleep-android-emulator:local}"
exec docker run --rm --network host -v "$ROOT:/workspace" -v "$GATE_ANDROID_HOME:/root/.android" \
	-w /workspace --entrypoint adb "$EMU_IMAGE" "$@"
