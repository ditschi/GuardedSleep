#!/usr/bin/env bash
set -euo pipefail

AVD_NAME="${EMULATOR_AVD:-guardedsleep_e2e}"
EMULATOR_PORT="${EMULATOR_PORT:-5554}"
ADB_PORT="${ADB_PORT:-5555}"

ACCEL_FLAG="-accel on"
if [[ ! -e /dev/kvm ]]; then
	echo "WARN: /dev/kvm not available — emulator will run without hardware acceleration (very slow)." >&2
	ACCEL_FLAG="-accel off"
fi

echo "Starting AVD ${AVD_NAME} (adb port ${ADB_PORT})..."
"${ANDROID_HOME}/emulator/emulator" \
	-avd "${AVD_NAME}" \
	-no-window \
	-no-audio \
	-no-boot-anim \
	-gpu swiftshader_indirect \
	${ACCEL_FLAG} \
	-ports "${EMULATOR_PORT},${ADB_PORT}" \
	>&2 &

EMULATOR_PID=$!

cleanup() {
	kill "${EMULATOR_PID}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

adb wait-for-device
echo "Waiting for Android boot to complete..."
for _ in $(seq 1 120); do
	boot="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)"
	if [[ "$boot" == "1" ]]; then
		break
	fi
	sleep 2
done

boot="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)"
if [[ "$boot" != "1" ]]; then
	echo "ERROR: Emulator did not finish booting within the timeout." >&2
	exit 1
fi

adb shell input keyevent 82 >/dev/null 2>&1 || true
echo "Emulator ready (adb devices: $(adb devices | tr '\n' ' '))"

wait "${EMULATOR_PID}"
