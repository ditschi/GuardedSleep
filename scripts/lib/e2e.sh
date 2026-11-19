#!/usr/bin/env bash
# Browser E2E (Playwright, apps/e2e). Prints "SKIPPED: e2e needs playwright-browsers" when Chromium is
# not installed, unless GS_STRICT=1 (CI), where it installs the browser with its system dependencies.
# Install by hand: pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium
# https://playwright.dev/docs/browsers
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=require.sh
source scripts/lib/require.sh
# shellcheck source=playwright-ready.sh
source scripts/lib/playwright-ready.sh

playwright() { pnpm --filter @guardedsleep/e2e exec playwright "$@"; }

if ! gs_playwright_ready; then
  if [ "${GS_STRICT:-0}" = "1" ]; then
    playwright install ${CI:+--with-deps} chromium
  else
    gs_skip "e2e needs playwright-browsers (pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium)"
    exit 0
  fi
fi

# `just e2e -- --shard=1/3` hands the separator on as well; Playwright would read everything after a
# bare `--` as a file filter, so drop it.
if [ "${1:-}" = "--" ]; then shift; fi

pnpm run e2e "$@"
