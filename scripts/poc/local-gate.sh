#!/usr/bin/env bash
# Runs the ADR-015 device gate (AC-1 … AC-4) on this machine: two KVM emulators, the signaling
# image and coturn as Docker containers on one shared network namespace, and scripts/poc/gate.sh
# in a runner container in that same namespace (inside the emulator image, so adb matches).
#   scripts/poc/local-gate.sh            # 30 min Doze
#   DOZE_MINUTES=2 scripts/poc/local-gate.sh
#
# Why everything runs in containers: with rootless Docker, `--network host` is rootlesskit's
# namespace, not the host's, so the host cannot reach the emulators' adb ports. All containers share
# /root/.android so the runner's adb key is the one the emulators trust.
# Needs: read/write /dev/kvm, Docker, a debug APK (scripts/core-build.sh android + Gradle).
# KEEP_ENV=1 leaves emulators, signaling and TURN running afterwards and reuses them when present
# (fast loop; scripts/poc/adb.sh talks to them, `docker rm -f gs-gate-baby gs-gate-parent gs-gate-signaling gs-gate-turn` ends it).
# Audio stays enabled in the emulators: AC-2 measures received audio packets.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EMU_IMAGE="${EMULATOR_IMAGE:-guardedsleep-android-emulator:local}"
SIG_IMAGE="${SIGNALING_IMAGE:-guardedsleep-signaling:local}"
OUT="${OUT:-$ROOT/target/poc-gate}"
APK="${APK:-$ROOT/apps/android/app/build/outputs/apk/debug/app-debug.apk}"
# Not 8787: a dev stack (docker/stack) may already own that port, and a stale server answers every hello.
SIGNAL_PORT="${SIGNAL_PORT:-18787}"
ANDROID_HOME_DIR="$OUT/android-home"
mkdir -p "$OUT" "$ANDROID_HOME_DIR"

[[ -r /dev/kvm && -w /dev/kvm ]] || {
	echo "local-gate: /dev/kvm missing or not accessible (run scripts/check-kvm.sh)" >&2
	exit 1
}
[[ -f "$APK" ]] || {
	echo "local-gate: APK not found: $APK" >&2
	exit 1
}
case "$APK" in "$ROOT"/*) ;; *)
	echo "local-gate: APK must be inside the repository (mounted at /workspace)" >&2
	exit 1
	;;
esac
case "$OUT" in "$ROOT"/*) ;; *)
	echo "local-gate: OUT must be inside the repository (mounted at /workspace)" >&2
	exit 1
	;;
esac
docker image inspect "$EMU_IMAGE" >/dev/null 2>&1 || docker build -t "$EMU_IMAGE" "$ROOT/docker/android-emulator"
# Always rebuild (cached): a stale server image speaks an older protocol and fails every hello.
docker build -q -t "$SIG_IMAGE" -f "$ROOT/docker/signaling-server/Dockerfile" "$ROOT" >/dev/null

CONTAINERS=(gs-gate-baby gs-gate-parent gs-gate-signaling gs-gate-turn)
cleanup() {
	for c in gs-gate-baby gs-gate-parent; do docker logs "$c" >"$OUT/$c.log" 2>&1 || true; done
	docker rm -f "${CONTAINERS[@]}" >/dev/null 2>&1 || true
}
KEEP_ENV="${KEEP_ENV:-0}"
running() { [[ "$(docker inspect -f '{{.State.Running}}' "$1" 2>/dev/null)" == "true" ]]; }
if [[ "$KEEP_ENV" == "1" ]]; then
	trap 'for c in gs-gate-baby gs-gate-parent; do docker logs "$c" >"$OUT/$c.log" 2>&1 || true; done' EXIT
else
	trap cleanup EXIT
fi
REUSE=0
if [[ "$KEEP_ENV" == "1" ]] && running gs-gate-baby && running gs-gate-parent && running gs-gate-signaling && running gs-gate-turn; then
	REUSE=1
	# Pick up the current server image if it changed; emulators and TURN stay.
	docker rm -f gs-gate-signaling >/dev/null 2>&1 || true
else
	docker rm -f "${CONTAINERS[@]}" >/dev/null 2>&1 || true
fi

start_emulator() {
	local name=$1 port=$2 grpc=$3
	docker run -d --name "$name" --network host --device /dev/kvm --shm-size 2g \
		-v "$ANDROID_HOME_DIR:/root/.android" \
		--entrypoint /opt/android-sdk/emulator/emulator "$EMU_IMAGE" \
		-avd guardedsleep_e2e -port "$port" -grpc "$grpc" -no-window -no-snapshot -no-boot-anim \
		-gpu swiftshader_indirect -accel on -memory 2048 -cores 2 >/dev/null
}
if [[ $REUSE == 0 ]]; then
	start_emulator gs-gate-baby 5554 8554
	start_emulator gs-gate-parent 5556 8556
fi

docker run -d --name gs-gate-signaling --network host \
	-e NODE_ENV=development -e PORT=$SIGNAL_PORT -e SIGNALING_ALLOWED_ORIGINS=http://localhost "$SIG_IMAGE" >/dev/null
sleep 2
running gs-gate-signaling || {
	echo "local-gate: signaling container exited (port $SIGNAL_PORT busy?):" >&2
	docker logs gs-gate-signaling >&2 || true
	exit 1
}
if [[ $REUSE == 0 ]]; then
# Test-only TURN credentials (poc/poc); loopback peers let both relay allocations reach each other.
docker run -d --name gs-gate-turn --network host coturn/coturn:4.18.0 \
	-n --log-file=stdout --no-cli --fingerprint --lt-cred-mech --user=poc:poc --realm=guardedsleep.poc \
	--listening-port=3478 --listening-ip=0.0.0.0 --relay-ip=127.0.0.1 --allow-loopback-peers \
	--min-port=49160 --max-port=49260 >/dev/null
fi

APK_IN="/workspace/${APK#"$ROOT"/}"
OUT_IN="/workspace/${OUT#"$ROOT"/}"
docker run --rm --network host -v "$ROOT:/workspace" -v "$ANDROID_HOME_DIR:/root/.android" \
	-e DOZE_MINUTES -e ALERT_MAX_MS -e HB_INTERVAL_MS -e ALARM_AFTER_MS \
	-e SIGNAL="ws://10.0.2.2:$SIGNAL_PORT" -e SIGNAL_PORT="$SIGNAL_PORT" -e APK="$APK_IN" -e OUT="$OUT_IN" -e BABY_SERIAL=emulator-5554 -e PARENT_SERIAL=emulator-5556 \
	--entrypoint bash "$EMU_IMAGE" -c '
		set -euo pipefail
		for serial in emulator-5554 emulator-5556; do
			timeout 300 adb -s "$serial" wait-for-device
			for _ in $(seq 1 150); do
				[[ "$(adb -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" == "1" ]] && break
				sleep 2
			done
			for s in window_animation_scale transition_animation_scale animator_duration_scale; do
				adb -s "$serial" shell settings put global "$s" 0
			done
			echo "local-gate: $serial booted"
		done
		curl -sf http://127.0.0.1:$SIGNAL_PORT/healthz >/dev/null && echo "local-gate: signaling healthy"
		/workspace/scripts/poc/gate.sh
	'
