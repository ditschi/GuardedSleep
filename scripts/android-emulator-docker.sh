#!/usr/bin/env bash
# Start/stop/wait for the headless Android emulator in Docker.
# Usage (repo root): scripts/android-emulator-docker.sh up|wait|down|status
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT/docker/android-emulator/compose.yaml"
ADB_PORT="${EMULATOR_ADB_PORT:-5555}"
WAIT_TIMEOUT_SEC="${EMULATOR_WAIT_TIMEOUT_SEC:-300}"

compose() {
	docker compose -f "$COMPOSE_FILE" "$@"
}

require_kvm() {
	if [[ -e /dev/kvm ]]; then
		return 0
	fi
	if [[ "${EMULATOR_ALLOW_NO_KVM:-0}" == "1" ]]; then
		echo "WARN: /dev/kvm not available — using software emulation (very slow; boot may take 10+ min)." >&2
		return 0
	fi
	echo "ERROR: /dev/kvm not found. Enable KVM (BIOS virtualization + modprobe kvm)." >&2
	echo "       Or set EMULATOR_ALLOW_NO_KVM=1 for software emulation." >&2
	exit 1
}

cmd_up() {
	require_kvm
	compose build
	compose up -d
	echo "Emulator container started. Run: pnpm run emulator:wait"
}

cmd_wait() {
	local start_ts
	start_ts="$(date +%s)"
	echo "Waiting for emulator adb (port ${ADB_PORT})..."
	while true; do
		if compose exec -T emulator adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; then
			echo "Emulator boot completed."
			compose exec -T emulator adb devices
			return 0
		fi
		if (( $(date +%s) - start_ts > WAIT_TIMEOUT_SEC )); then
			echo "ERROR: Timed out after ${WAIT_TIMEOUT_SEC}s waiting for emulator boot." >&2
			compose logs --tail 80 emulator >&2 || true
			exit 1
		fi
		sleep 3
	done
}

cmd_down() {
	compose down --remove-orphans
}

cmd_status() {
	compose ps
	if compose ps --status running -q emulator 2>/dev/null | grep -q .; then
		compose exec -T emulator adb devices 2>/dev/null || true
	fi
}

usage() {
	cat <<'EOF'
Usage: scripts/android-emulator-docker.sh <up|wait|down|status>

  up      Build and start the headless emulator container (requires /dev/kvm)
  wait    Block until sys.boot_completed=1 inside the container
  down    Stop and remove the emulator container
  status  Show compose status and adb devices inside the container
EOF
}

case "${1:-}" in
	up) cmd_up ;;
	wait) cmd_wait ;;
	down) cmd_down ;;
	status) cmd_status ;;
	-h | --help | "") usage; exit 0 ;;
	*)
		echo "Unknown command: $1" >&2
		usage >&2
		exit 1
		;;
esac
