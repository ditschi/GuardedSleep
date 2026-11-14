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
# Maestro (UI assertions and the AC-12 journey): the mise-pinned install is mounted read-only into the
# runner; without it the gate prints "SKIPPED: gate-ui needs maestro", and under GS_STRICT=1 it fails.
# GATE_ONLY=ui runs the install and the AC-12 journey only, GATE_ONLY=ac1 stops after AC-1.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=tools/poc-images.env
source "$ROOT/tools/poc-images.env"
EMU_IMAGE="${EMULATOR_IMAGE:-guardedsleep-android-emulator:local}"
SIG_IMAGE="${SIGNALING_IMAGE:-guardedsleep-signaling:local}"
OUT="${OUT:-$ROOT/target/poc-gate}"
APK="${APK:-$ROOT/apps/android/app/build/outputs/apk/debug/guardedsleep-debug.apk}"
# Not 8787: a dev stack (docker/stack) may already own that port, and a stale server answers every hello.
SIGNAL_PORT="${SIGNAL_PORT:-18787}"
# The emulators' shared ~/.android (adb key, AVD state) lives outside OUT: CI uploads OUT as an artifact,
# and the files the containers write there are unreadable for the runner (EACCES) and must not be published.
ANDROID_HOME_DIR="${GATE_ANDROID_HOME:-$ROOT/target/poc-android-home}"
[[ "${DOZE_MINUTES:-30}" =~ ^[0-9]{1,3}$ ]] || {
	echo "local-gate: DOZE_MINUTES must be an integer (got '${DOZE_MINUTES}')" >&2
	exit 2
}
# The AC-12 pairing code (a pairing secret of the throwaway emulators) lives here for the canary only:
# outside OUT, which CI uploads, and deleted on every exit path after the canary.
SECRET_DIR="${GATE_SECRET_DIR:-$ROOT/target/poc-gate-secret}"
case "$SECRET_DIR" in "$ROOT"/target/*) ;; *)
	echo "local-gate: GATE_SECRET_DIR must be inside $ROOT/target (mounted at /workspace)" >&2
	exit 2
	;;
esac
mkdir -p "$OUT" "$ANDROID_HOME_DIR"
rm -rf "$SECRET_DIR"

# Pinned Maestro from mise (docs: mise.toml). `mise where` prints the install dir of the pinned version.
MAESTRO_HOME="${MAESTRO_HOME:-}"
if [[ -z "$MAESTRO_HOME" ]] && command -v mise >/dev/null 2>&1; then
	MAESTRO_HOME="$(cd "$ROOT" && mise where maestro 2>/dev/null || true)"
fi
if [[ -n "$MAESTRO_HOME" && ! -x "$MAESTRO_HOME/bin/maestro" ]]; then MAESTRO_HOME=""; fi
if [[ -z "$MAESTRO_HOME" && "${GS_STRICT:-0}" == 1 ]]; then
	echo "local-gate: Maestro is not installed and GS_STRICT=1 (mise install maestro)" >&2
	exit 1
fi

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
# The canary scans these too: the signaling server must not log a key, room id or SDP.
save_logs() {
	for c in "${CONTAINERS[@]}"; do docker logs "$c" >"$OUT/$c.log" 2>&1 || true; done
}
drop_secret() { rm -rf "$SECRET_DIR"; }
cleanup() {
	save_logs
	drop_secret
	docker rm -f "${CONTAINERS[@]}" >/dev/null 2>&1 || true
}
keep_cleanup() {
	save_logs
	drop_secret
}
KEEP_ENV="${KEEP_ENV:-0}"
running() { [[ "$(docker inspect -f '{{.State.Running}}' "$1" 2>/dev/null)" == "true" ]]; }
if [[ "$KEEP_ENV" == "1" ]]; then
	trap keep_cleanup EXIT
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
	-e NODE_ENV=development -e STUN_DISABLED=1 -e GS_LOG_EVENTS=1 -e PORT="$SIGNAL_PORT" -e SIGNALING_ALLOWED_ORIGINS=http://localhost "$SIG_IMAGE" >/dev/null
sleep 2
running gs-gate-signaling || {
	echo "local-gate: signaling container exited (port $SIGNAL_PORT busy?):" >&2
	docker logs gs-gate-signaling >&2 || true
	exit 1
}
if [[ $REUSE == 0 ]]; then
# Test-only TURN credentials (poc/poc); loopback peers let both relay allocations reach each other.
docker run -d --name gs-gate-turn --network host "$COTURN_IMAGE" \
	-n --log-file=stdout --no-cli --fingerprint --lt-cred-mech --user=poc:poc --realm=guardedsleep.poc \
	--listening-port=3478 --listening-ip=0.0.0.0 --relay-ip=127.0.0.1 --allow-loopback-peers \
	--min-port=49160 --max-port=49260 >/dev/null
fi

APK_IN="/workspace/${APK#"$ROOT"/}"
OUT_IN="/workspace/${OUT#"$ROOT"/}"
MAESTRO_MOUNT=()
if [[ -n "$MAESTRO_HOME" ]]; then MAESTRO_MOUNT=(-v "$MAESTRO_HOME:/opt/maestro:ro"); fi
# Maestro: no analytics, no update check (AGENTS.md §7.5; the same names as mise.toml [env]); a 1 GB
# heap (gate-ui-journeys design §6). Its debug output stays inside this --rm container.
docker run --rm --network host -v "$ROOT:/workspace" -v "$ANDROID_HOME_DIR:/root/.android" "${MAESTRO_MOUNT[@]}" \
	-e GATE_ONLY -e GATE_UI_JOURNEY -e DOZE_MINUTES -e ALERT_MAX_MS -e HB_INTERVAL_MS -e ALARM_AFTER_MS -e GS_STRICT -e GATE_SECRET_DIR="/workspace/${SECRET_DIR#"$ROOT"/}" \
	-e MAESTRO_CLI_NO_ANALYTICS="${MAESTRO_CLI_NO_ANALYTICS:-1}" \
	-e MAESTRO_CLI_ANALYSIS_NOTIFICATION_DISABLED="${MAESTRO_CLI_ANALYSIS_NOTIFICATION_DISABLED:-true}" \
	-e MAESTRO_DISABLE_UPDATE_CHECK="${MAESTRO_DISABLE_UPDATE_CHECK:-true}" \
	-e MAESTRO_OPTS="${GATE_MAESTRO_OPTS:--Xmx1g}" \
	-e SIGNAL="ws://10.0.2.2:$SIGNAL_PORT" -e SIGNAL_PORT="$SIGNAL_PORT" -e APK="$APK_IN" -e OUT="$OUT_IN" -e BABY_SERIAL=emulator-5554 -e PARENT_SERIAL=emulator-5556 \
	--entrypoint bash "$EMU_IMAGE" -c '
		set -euo pipefail
		[[ -x /opt/maestro/bin/maestro ]] && export PATH="/opt/maestro/bin:$PATH"
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

# AC-12 hermetic check: the journey only counts if both devices met on THIS gate's signaling server in
# its time window (a silent fallback to another server would pair nothing here). The relay event log
# (GS_LOG_EVENTS) names roles only, never an address, a room or a key. docker owns the logs, so the
# check lives here and not in gate.sh.
if [[ -f "$OUT/ac12-window.txt" ]]; then
	read -r w_from w_to <"$OUT/ac12-window.txt"
	joined="$(docker logs --since "$w_from" --until "$((w_to + 1))" gs-gate-signaling 2>&1 | grep -c 'joined as' || true)"
	babies="$(docker logs --since "$w_from" --until "$((w_to + 1))" gs-gate-signaling 2>&1 | grep -c 'joined as baby' || true)"
	if [[ "$joined" -ge 2 && "$babies" -ge 1 ]]; then
		echo "PASS AC-12b — $joined members joined on the gate's own signaling server during the UI journey" | tee -a "$OUT/summary.txt"
	else
		echo "FAIL AC-12b — only $joined member join(s) on the gate's signaling server during the UI journey (did a device use another relay?)" | tee -a "$OUT/summary.txt" >&2
		exit 1
	fi
fi

# Log canary (AGENTS.md section 7.6): the gate paired with known keys; none of them, nothing derived
# from them and no plaintext SDP may be in any log of the run. Runs on the host (the runner container
# above has no Rust toolchain) over everything the gate wrote to OUT.
save_logs
log_dir="${OUT#"$ROOT"/}"
canary_extra=()
if [[ -f "$SECRET_DIR/code" ]]; then canary_extra=(--extra-canary-file "${SECRET_DIR#"$ROOT"/}/code"); fi
"$ROOT/scripts/poc/canary-scan.sh" "${canary_extra[@]}" "$log_dir" || {
	echo "local-gate: LOG CANARY FAILED: a secret or plaintext SDP is in a gate log (names above, files in $log_dir)" >&2
	exit 1
}
drop_secret
