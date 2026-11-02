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

playwright() { pnpm --filter @guardedsleep/e2e exec playwright "$@"; }

browser_ready() {
  pnpm --filter @guardedsleep/e2e exec node --input-type=module -e \
    "import { chromium } from '@playwright/test'; import { existsSync } from 'node:fs'; process.exit(existsSync(chromium.executablePath()) ? 0 : 1)" \
    >/dev/null 2>&1
}

if ! browser_ready; then
  if [ "${GS_STRICT:-0}" = "1" ]; then
    playwright install ${CI:+--with-deps} chromium
  else
    echo "SKIPPED: e2e needs playwright-browsers (pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium)"
    exit 0
  fi
fi

pnpm run e2e "$@"
