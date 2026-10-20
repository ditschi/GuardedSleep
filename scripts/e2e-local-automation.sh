#!/usr/bin/env bash
# One-command local E2E orchestration (Docker-first, optional docker emulator).
# Runs browser tests always; hybrid Android when adb device or docker emulator is available.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STACK_FILE="$ROOT/docker/stack/compose.yaml"
STACK_ENV="$ROOT/docker/stack/.env"

RUN_WSS_STACK="${RUN_WSS_STACK:-1}"
USE_DOCKER_EMULATOR="${USE_DOCKER_EMULATOR:-auto}"

log() {
  printf '\n==> %s\n' "$1"
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

run_step() {
  local label="$1"
  shift
  log "$label"
  "$@"
}

adb_devices() {
  if [[ -n "${GUARDEDSLEEP_ADB:-}" ]]; then
    bash -c "${GUARDEDSLEEP_ADB} devices" 2>/dev/null || true
  else
    adb devices 2>/dev/null || true
  fi
}

has_adb_device() {
  adb_devices | awk 'NR > 1 && $2 == "device" { found = 1 } END { exit(found ? 0 : 1) }'
}

stack_started=0
emulator_started=0

cleanup() {
  if [[ "$stack_started" -eq 1 ]]; then
    log "Stopping local Docker stack"
    docker compose -f "$STACK_FILE" --env-file "$STACK_ENV" down >/dev/null
  fi
  if [[ "$emulator_started" -eq 1 ]]; then
    log "Stopping Docker emulator"
    bash "$ROOT/scripts/android-emulator-docker.sh" down >/dev/null
  fi
}
trap cleanup EXIT

start_docker_emulator() {
  if [[ ! -e /dev/kvm ]]; then
    if [[ "${EMULATOR_ALLOW_NO_KVM:-0}" == "1" ]]; then
      log "No /dev/kvm — starting Docker emulator in software mode (slow)."
    else
      log "Skipping Docker emulator: /dev/kvm not available (run scripts/check-kvm.sh)."
      return 1
    fi
  fi
  run_step "Start Docker Android emulator" bash "$ROOT/scripts/android-emulator-docker.sh" up
  run_step "Wait for emulator boot" bash "$ROOT/scripts/android-emulator-docker.sh" wait
  export GUARDEDSLEEP_ADB="$ROOT/docker/android-emulator/adb.sh"
  export GUARDEDSLEEP_DOCKER_GRADLE=1
  emulator_started=1
}

cd "$ROOT"

export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_PATH:-$ROOT/.cache/ms-playwright}"
if [[ "${GUARDEDSLEEP_E2E_IN_DOCKER:-0}" != "1" ]] && ! compgen -G "${PLAYWRIGHT_BROWSERS_PATH}/chromium_headless_shell-*" >/dev/null; then
  run_step "Install Playwright Chromium (workspace cache)" \
    pnpm --filter @guardedsleep/e2e exec playwright install chromium --with-deps
fi

run_step "Browser baseline E2E" pnpm run e2e
run_step "Browser LAN E2E (ws)" pnpm run e2e:lan

hybrid_ready=0
if has_adb_device; then
  hybrid_ready=1
elif [[ "$USE_DOCKER_EMULATOR" == "1" || "$USE_DOCKER_EMULATOR" == "auto" ]]; then
  if start_docker_emulator && has_adb_device; then
    hybrid_ready=1
  fi
fi

if [[ "$hybrid_ready" -eq 1 ]]; then
  run_step "Hybrid Android + browser LAN E2E (ws)" pnpm run e2e:hybrid:lan-ws

  if [[ "$RUN_WSS_STACK" == "1" ]]; then
    run_step "Generate LAN Docker env" bash "$ROOT/scripts/stack-lan-env.sh"
    run_step "Start Docker WSS stack (detached)" docker compose -f "$STACK_FILE" --env-file "$STACK_ENV" up -d --build
    stack_started=1
    run_step "Hybrid Android + browser LAN E2E (wss)" pnpm run e2e:hybrid:lan-wss
  else
    log "Skipping WSS stack/tests because RUN_WSS_STACK=0"
  fi
else
  log "Skipping hybrid Android tests: no adb device and docker emulator not started."
fi

log "Done. Local automation run completed."
