#!/usr/bin/env bash
# ADR-015/018 alpha gate, device part (AC-1 … AC-4, AC-10) on two Android emulators.
#
# Prerequisites (see scripts/poc/README.md; the CI workflow android-native.yml sets these up):
#   - two booted emulators (BABY_SERIAL, PARENT_SERIAL), debug APK at APK
#   - signaling server on host :8787, TURN (coturn) on host :3478 with user poc/poc
#     (two emulators cannot reach each other's 10.0.2.15 directly; media relays via the host)
#   - maestro on PATH (UI assertions); without it only the log-based assertions run
#
# Env: DOZE_MINUTES (30), ALERT_MAX_MS (5000), HB_INTERVAL_MS (5000), ALARM_AFTER_MS (60000)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BABY="${BABY_SERIAL:-emulator-5554}"
PARENT="${PARENT_SERIAL:-emulator-5556}"
APK="${APK:-$ROOT/apps/android/app/build/outputs/apk/debug/app-debug.apk}"
PKG=org.guardedsleep.app.alpha
DEBUG_LAUNCH=org.guardedsleep.app.debug.DebugLaunchActivity
DEBUG_INJECT=org.guardedsleep.app.debug.DebugInjectReceiver
INJECT_ACTION=org.guardedsleep.app.alpha.debug.INJECT_AUDIO
FLOWS="$ROOT/apps/android/maestro"
OUT="${OUT:-$ROOT/target/poc-gate}"
DOZE_MINUTES="${DOZE_MINUTES:-30}"
# Used in shell arithmetic below, which would evaluate an injected expression: integers only.
[[ "$DOZE_MINUTES" =~ ^[0-9]{1,3}$ ]] || {
	echo "gate: DOZE_MINUTES must be an integer (got '$DOZE_MINUTES')" >&2
	exit 2
}
ALERT_MAX_MS="${ALERT_MAX_MS:-5000}"
HB_INTERVAL_MS="${HB_INTERVAL_MS:-5000}"
ALARM_AFTER_MS="${ALARM_AFTER_MS:-60000}"
SIGNAL="${SIGNAL:-ws://10.0.2.2:8787}"
TURN_URL="${TURN_URL:-turn:10.0.2.2:3478}"

mkdir -p "$OUT"
: >"$OUT/summary.txt"

new_psk() { head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n'; }
log() { echo "[gate] $*"; }
pass() { echo "PASS $1 — $2" | tee -a "$OUT/summary.txt"; }
fail() {
	echo "FAIL $1 — $2" | tee -a "$OUT/summary.txt"
	dump_logs
	exit 1
}
dump_logs() {
	adb -s "$BABY" logcat -d >"$OUT/logcat-baby.txt" 2>&1 || true
	adb -s "$PARENT" logcat -d >"$OUT/logcat-parent.txt" 2>&1 || true
}

# Epoch ms of the first GS_POC line matching a pattern, or empty.
event_time() {
	adb -s "$1" logcat -d -s GS_POC:I | grep -E "$2" | head -1 | grep -oE 't=[0-9]+' | head -1 | cut -d= -f2 || true
}

last_value() {
	adb -s "$1" logcat -d -s GS_POC:I | grep -E "$2" | tail -1 | grep -oE "$3=[0-9]+" | cut -d= -f2 || true
}

wait_event() {
	local serial=$1 pattern=$2 timeout_s=$3
	for _ in $(seq 1 "$timeout_s"); do
		[[ -n "$(event_time "$serial" "$pattern")" ]] && return 0
		sleep 1
	done
	return 1
}

# Emulator clocks follow the host clock; Android's toybox date has no %N, so read it here.
device_now_ms() { date +%s%3N; }

maestro_assert() {
	local serial=$1 flow=$2
	if ! command -v maestro >/dev/null; then
		log "maestro not installed — skipping UI assertion $flow"
		return 0
	fi
	maestro --device "$serial" test --format junit --output "$OUT/maestro-$(basename "$flow" .yaml)-$serial.xml" "$FLOWS/$flow"
}

# Starts monitoring as if paired (debug-only activity, DUMP permission: adb shell only).
launch() {
	local serial=$1 role=$2 psk=$3
	adb -s "$serial" shell am start -n "$PKG/$DEBUG_LAUNCH" \
		--es role "$role" --es psk_b64url "$psk" --es signal_urls "$SIGNAL" \
		--es turn_url "$TURN_URL" --es turn_user poc --es turn_pass poc --es relay_only true \
		--es auto_listen true --es heartbeat_interval_ms "$HB_INTERVAL_MS" --es alarm_after_ms "$ALARM_AFTER_MS" --es noise_hold_ms "${NOISE_HOLD_MS:-500}" >/dev/null
}

# First event time (epoch ms) matching a pattern with t > $3, or empty.
event_after() {
	adb -s "$1" logcat -d -s GS_POC:I | grep -E "$2" | grep -oE 't=[0-9]+' | cut -d= -f2 | awk -v t="$3" '$1 > t { print; exit }' || true
}

wait_event_after() {
	local serial=$1 pattern=$2 since=$3 timeout_s=$4
	for _ in $(seq 1 "$timeout_s"); do
		[[ -n "$(event_after "$serial" "$pattern" "$since")" ]] && return 0
		sleep 1
	done
	return 1
}

reset_app() {
	adb -s "$1" shell am force-stop "$PKG"
	adb -s "$1" logcat -c
}

restore_power() {
	for s in "$BABY" "$PARENT"; do
		adb -s "$s" shell dumpsys deviceidle unforce >/dev/null 2>&1 || true
		adb -s "$s" shell dumpsys battery reset >/dev/null 2>&1 || true
	done
}
trap 'restore_power; dump_logs' EXIT

log "installing $APK"
for s in "$BABY" "$PARENT"; do
	# A reused emulator (KEEP_ENV=1) may hold a build signed with another debug key.
	adb -s "$s" uninstall "$PKG" >/dev/null 2>&1 || true
	adb -s "$s" install -r -g "$APK" >/dev/null
	# Android 14+: full-screen intents need explicit consent; the test device grants it via appops.
	adb -s "$s" shell appops set "$PKG" USE_FULL_SCREEN_INTENT allow || true
	adb -s "$s" shell settings put global stay_on_while_plugged_in 0
	adb -s "$s" shell input keyevent KEYCODE_WAKEUP
	adb -s "$s" shell wm dismiss-keyguard || true
	reset_app "$s"
done

# --- AC-1: pairing through the core; a different PSK never meets the peer (v3: PSK-derived rooms) ---
PSK="$(new_psk)"
log "AC-1a different PSKs derive different rooms"
launch "$BABY" baby "$PSK"
launch "$PARENT" parent "$(new_psk)"
sleep 60
if adb -s "$BABY" logcat -d -s GS_POC:I | grep -q 'pairing_verified' || adb -s "$PARENT" logcat -d -s GS_POC:I | grep -q 'pairing_verified'; then
	fail AC-1 "devices with different PSKs verified"
fi
wait_event "$PARENT" 'state .*cause=peer_absent' 60 || fail AC-1 "parent did not report cause peer_absent"
maestro_assert "$PARENT" assert-peer-absent.yaml || fail AC-1 "peer_absent not visible in parent UI"
reset_app "$BABY"
reset_app "$PARENT"

log "AC-1b matching PSK"
launch "$BABY" baby "$PSK"
launch "$PARENT" parent "$PSK"
wait_event "$BABY" 'pairing_verified' 90 || fail AC-1 "baby did not verify pairing"
wait_event "$PARENT" 'pairing_verified' 30 || fail AC-1 "parent did not verify pairing"
maestro_assert "$BABY" assert-verified-baby.yaml || fail AC-1 "baby UI not verified"
maestro_assert "$PARENT" assert-verified.yaml || fail AC-1 "parent UI not verified"
pass AC-1 "pairing verified with shared PSK; different PSKs never meet (parent reports peer_absent)"

# --- AC-2: 30 min forced Doze, screen off, unplugged -----------------------------------------
sleep 12 # let a few stats samples arrive
P0="$(last_value "$PARENT" 'stats ' packets_received)"
[[ -n "$P0" && "$P0" -gt 0 ]] || fail AC-2 "no audio packets before Doze (listen on?)"
log "AC-2 packets before Doze: $P0; dozing ${DOZE_MINUTES} min"
for s in "$BABY" "$PARENT"; do
	adb -s "$s" shell input keyevent KEYCODE_SLEEP
	adb -s "$s" shell dumpsys battery unplug
	adb -s "$s" shell dumpsys deviceidle force-idle >/dev/null
done
sleep $((DOZE_MINUTES * 60))
for s in "$BABY" "$PARENT"; do
	state="$(adb -s "$s" shell dumpsys deviceidle get deep | tr -d '\r')"
	log "$s deep idle state after wait: $state"
	[[ "$state" == "IDLE" ]] || fail AC-2 "$s left Doze during the test ($state)"
done
adb -s "$BABY" shell dumpsys activity services "$PKG" | grep -q 'MonitorService' || fail AC-2 "baby foreground service gone"
adb -s "$BABY" shell dumpsys activity services "$PKG" | grep -q 'isForeground=true' || fail AC-2 "baby service not foreground"
P1="$(last_value "$PARENT" 'stats ' packets_received)"
STATS_T="$(adb -s "$PARENT" logcat -d -s GS_POC:I | grep 'stats ' | tail -1 | grep -oE 't=[0-9]+' | cut -d= -f2)"
AGE=$(($(device_now_ms "$PARENT") - STATS_T))
MIN_GAIN=$((DOZE_MINUTES * 60 * 20)) # Opus sends 50 pkt/s; demand at least 40 %
log "AC-2 packets after: $P1 (gain $((P1 - P0)), need $MIN_GAIN), last stats ${AGE} ms ago"
[[ $((P1 - P0)) -ge $MIN_GAIN ]] || fail AC-2 "audio did not keep flowing during Doze"
[[ $AGE -lt 30000 ]] || fail AC-2 "parent stopped reporting stats"
if adb -s "$PARENT" logcat -d -s GS_POC:I | grep -q 'alert .*kind=link_lost'; then
	fail AC-2 "parent raised link_lost during Doze"
fi
pass AC-2 "${DOZE_MINUTES} min forced Doze: baby FGS alive, parent received $((P1 - P0)) audio packets"

# --- AC-3: injected cry → parent alert within ALERT_MAX_MS (devices still in Doze) -----------
adb -s "$BABY" shell am broadcast -a "$INJECT_ACTION" \
	-n "$PKG/$DEBUG_INJECT" --ef rms 0.6 --el duration_ms 3000 >/dev/null
wait_event "$PARENT" 'alert .*kind=noise' 30 || fail AC-3 "no noise alert on parent"
T_INJ="$(event_time "$BABY" '^.*inject ')"
T_ALERT="$(event_time "$PARENT" 'alert .*kind=noise')"
DELTA=$((T_ALERT - T_INJ))
log "AC-3 inject→alert ${DELTA} ms (limit ${ALERT_MAX_MS})"
[[ $DELTA -le $ALERT_MAX_MS ]] || fail AC-3 "alert took ${DELTA} ms"
maestro_assert "$PARENT" assert-noise-alert.yaml || fail AC-3 "alert not visible on parent"
pass AC-3 "cry alert visible on parent ${DELTA} ms after injection (in Doze)"

# --- AC-10: Wi-Fi flaps on each side (ADR-017 §12) ----------------------------------------------
restore_power
for s in "$BABY" "$PARENT"; do
	adb -s "$s" shell input keyevent KEYCODE_WAKEUP
done
wifi() { # serial on|off — mobile data too, so the emulator really loses its network
	adb -s "$1" shell svc wifi "$( [[ $2 == on ]] && echo enable || echo disable )" >/dev/null
	adb -s "$1" shell svc data "$( [[ $2 == on ]] && echo enable || echo disable )" >/dev/null 2>&1 || true
}
trap 'wifi "$BABY" on; wifi "$PARENT" on; restore_power; dump_logs' EXIT

ac10_side() {
	local side=$1 serial=$2 label=$3
	local t0 t_lost
	log "AC-10 $label: 20 s outage"
	t0="$(device_now_ms "$serial")"
	wifi "$serial" off
	sleep 20
	wifi "$serial" on
	wait_event_after "$PARENT" 'pairing_verified' "$t0" 60 || fail AC-10 "$label: no re-verification after 20 s outage"
	if [[ -n "$(event_after "$PARENT" 'alert .*kind=link_lost' "$t0")" ]]; then
		fail AC-10 "$label: link_lost after a 20 s outage"
	fi
	log "AC-10 $label: 90 s outage"
	t0="$(device_now_ms "$serial")"
	wifi "$serial" off
	wait_event_after "$PARENT" 'alert .*kind=link_lost' "$t0" $(((ALARM_AFTER_MS + 15000) / 1000)) || {
		wifi "$serial" on
		fail AC-10 "$label: no link_lost during a 90 s outage"
	}
	t_lost="$(event_after "$PARENT" 'alert .*kind=link_lost' "$t0")"
	[[ $((t_lost - t0)) -le $((ALARM_AFTER_MS + 2000)) ]] || {
		wifi "$serial" on
		fail AC-10 "$label: link_lost after $((t_lost - t0)) ms (limit $((ALARM_AFTER_MS + 2000)))"
	}
	local elapsed=$(($(device_now_ms "$serial") - t0))
	[[ $elapsed -ge 90000 ]] || sleep $(((90000 - elapsed) / 1000))
	wifi "$serial" on
	wait_event_after "$PARENT" 'pairing_verified' "$t_lost" 90 || fail AC-10 "$label: no re-verification after the 90 s outage"
	wait_event_after "$PARENT" 'link_restored' "$t_lost" 30 || fail AC-10 "$label: parent did not report link_restored"
	pass "AC-10$side" "$label: 20 s outage silent and recovered; 90 s outage alarmed in $((t_lost - t0)) ms and recovered"
}
ac10_side a "$BABY" "baby Wi-Fi flap"
ac10_side b "$PARENT" "parent Wi-Fi flap"

# --- AC-4: killing the baby process raises "connection lost" ---------------------------------
T_KILL="$(device_now_ms "$BABY")"
adb -s "$BABY" shell am force-stop "$PKG"
wait_event_after "$PARENT" 'alert .*kind=link_lost' "$T_KILL" $(((ALARM_AFTER_MS + 15000) / 1000)) || fail AC-4 "no link_lost alert"
T_LOST="$(event_after "$PARENT" 'alert .*kind=link_lost' "$T_KILL")"
LIMIT=$((ALARM_AFTER_MS + 2000))
log "AC-4 kill→link_lost $((T_LOST - T_KILL)) ms (limit ${LIMIT})"
[[ $((T_LOST - T_KILL)) -le $LIMIT ]] || fail AC-4 "link_lost after $((T_LOST - T_KILL)) ms"
maestro_assert "$PARENT" assert-link-lost.yaml || fail AC-4 "link_lost not visible on parent"
pass AC-4 "parent alarmed $((T_LOST - T_KILL)) ms after the baby process was killed"

log "all device criteria passed"
cat "$OUT/summary.txt"
