#!/usr/bin/env bash
# Local development stack: the signaling server (legacy Node, wave 2 swaps in the Rust server) and the
# Vite dev server for apps/mobile, started together and stopped together (Ctrl-C, or when either exits).
# Used by `just dev` and by the Playwright webServer (apps/e2e/playwright.config.ts).
#   PORT=18787 just dev      # the signaling server reads PORT (Vite stays on 5173)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/require.sh
source scripts/lib/require.sh

pnpm --filter @guardedsleep/protocol build

# Everything stays in this script's process group on purpose: Playwright stops its webServer by killing
# the whole group (SIGKILL, so no trap runs), and `kill 0` below does the same on a normal exit.
trap 'trap - EXIT INT TERM; kill 0' EXIT INT TERM

pnpm --filter @guardedsleep/signaling-server dev &
pnpm --filter @guardedsleep/mobile dev &

wait -n
