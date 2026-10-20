#!/usr/bin/env bash
# One-command local browser E2E orchestration (Docker-first Playwright cache).
# Hybrid Android + browser E2E was removed in wave 1 (ADR-020); the native emulator gate lives in scripts/poc/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log() {
  printf '\n==> %s\n' "$1"
}

run_step() {
  local label="$1"
  shift
  log "$label"
  "$@"
}

cd "$ROOT"

export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_PATH:-$ROOT/.cache/ms-playwright}"
if [[ "${GUARDEDSLEEP_E2E_IN_DOCKER:-0}" != "1" ]] && ! compgen -G "${PLAYWRIGHT_BROWSERS_PATH}/chromium_headless_shell-*" >/dev/null; then
  run_step "Install Playwright Chromium (workspace cache)" \
    pnpm --filter @guardedsleep/e2e exec playwright install chromium --with-deps
fi

run_step "Browser baseline E2E" pnpm run e2e
run_step "Browser LAN E2E (ws)" pnpm run e2e:lan

log "Done. Local automation run completed."
