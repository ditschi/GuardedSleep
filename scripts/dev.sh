#!/usr/bin/env bash
# Local development stack: the Rust signaling server (crates/guardedsleep-server) and the Vite dev
# server for apps/mobile, started together and stopped together (Ctrl-C, or when either exits).
# Used by `just dev` and by the Playwright webServer (apps/e2e/playwright.config.ts).
#   PORT=18787 just dev      # the signaling server reads PORT (Vite stays on 5173)
#   STUN_DISABLED=0 just dev # also start the STUN responder (UDP 3478); off by default in dev
# The server is built with scripts/cargo.sh (host cargo or the build image) and the binary runs on
# the host, so ports are reachable without Docker networking.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/require.sh
source scripts/lib/require.sh

# `wait -n` needs bash 4.3 or newer (stock macOS bash is 3.2).
if (( BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 3) )); then
  echo "dev.sh needs bash >= 4.3 (found ${BASH_VERSION}); install a newer bash." >&2
  exit 2
fi

# Everything stays in this script's process group on purpose: Playwright stops its webServer by killing
# the whole group (SIGKILL, so no trap runs), and `kill 0` below does the same on a normal exit.
# The trap keeps the real exit status (no always-143) and ignores TERM while it tears the group down.
# shellcheck disable=SC2154 # rc is assigned inside the trap string itself
trap 'rc=$?; trap "" TERM; trap - EXIT INT; kill 0 2>/dev/null; exit "$rc"' EXIT INT TERM

scripts/cargo.sh build -q -p guardedsleep-server
STUN_DISABLED="${STUN_DISABLED:-1}" target/debug/guardedsleep-server &
pnpm --filter @guardedsleep/mobile dev &

wait -n
