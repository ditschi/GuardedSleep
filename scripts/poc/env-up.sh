#!/usr/bin/env bash
# Starts everything scripts/poc/gate.sh needs on a Linux host with KVM:
#   two headless emulators (baby :5554, parent :5556), the signaling server on :8787 and coturn
#   on :3478. Emulators sit behind separate NATs (both 10.0.2.15), so media relays through
#   TURN on the host; the core and pairing logic are unaffected.
# Env: API_LEVEL (35), SYSTEM_IMAGE, KEEP_RUNNING
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=tools/poc-images.env
source "$ROOT/tools/poc-images.env"
API_LEVEL="${API_LEVEL:-35}"
SYSTEM_IMAGE="${SYSTEM_IMAGE:-system-images;android-${API_LEVEL};google_apis;x86_64}"
OUT="${OUT:-$ROOT/target/poc-gate}"
mkdir -p "$OUT"

[[ -e /dev/kvm ]] || {
	echo "env-up: /dev/kvm missing — emulators need hardware acceleration" >&2
	exit 1
}

SDK="${ANDROID_HOME:?ANDROID_HOME must point at the Android SDK}"
yes | "$SDK/cmdline-tools/latest/bin/sdkmanager" --licenses >/dev/null || true
"$SDK/cmdline-tools/latest/bin/sdkmanager" --install emulator platform-tools "$SYSTEM_IMAGE" >/dev/null

for role in baby parent; do
	echo no | "$SDK/cmdline-tools/latest/bin/avdmanager" create avd --force -n "gs-$role" -k "$SYSTEM_IMAGE" -d pixel_6 >/dev/null
done

start_emulator() {
	local name=$1 port=$2
	nohup "$SDK/emulator/emulator" -avd "$name" -port "$port" -no-window -no-snapshot -no-boot-anim \
		-gpu swiftshader_indirect -memory 2048 -cores 2 >"$OUT/$name.log" 2>&1 &
}
start_emulator gs-baby 5554
start_emulator gs-parent 5556

# Signaling: plain WS for the emulator network only (never expose without TLS).
# STUN stays off: the coturn below owns UDP 3478 on the host.
"$ROOT/scripts/cargo.sh" build -q -p guardedsleep-server
STUN_DISABLED=1 PORT=8787 nohup "$ROOT/target/debug/guardedsleep-server" >"$OUT/signaling.log" 2>&1 &

# Test-only TURN credentials (poc/poc); loopback peers let both relay allocations reach each other.
docker rm -f gs-poc-turn >/dev/null 2>&1 || true
docker run -d --name gs-poc-turn --network host "$COTURN_IMAGE" \
	-n --log-file=stdout --no-cli --fingerprint --lt-cred-mech --user=poc:poc --realm=guardedsleep.poc \
	--listening-port=3478 --listening-ip=0.0.0.0 --relay-ip=127.0.0.1 --allow-loopback-peers \
	--min-port=49160 --max-port=49260 >/dev/null

for serial in emulator-5554 emulator-5556; do
	adb -s "$serial" wait-for-device
	for _ in $(seq 1 180); do
		[[ "$(adb -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]] && break
		sleep 2
	done
	adb -s "$serial" shell settings put global window_animation_scale 0
	adb -s "$serial" shell settings put global transition_animation_scale 0
	adb -s "$serial" shell settings put global animator_duration_scale 0
	echo "env-up: $serial booted"
done
curl -sf http://127.0.0.1:8787/healthz >/dev/null && echo "env-up: signaling healthy"
