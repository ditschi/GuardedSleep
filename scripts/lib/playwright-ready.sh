#!/usr/bin/env bash
# Shared by scripts/lib/e2e.sh and scripts/doctor.sh: is the Playwright Chromium of apps/e2e installed?
# Source it (from the repo root), then call gs_playwright_ready.
# https://playwright.dev/docs/browsers

gs_playwright_ready() {
  pnpm --filter @guardedsleep/e2e exec node --input-type=module -e \
    "import { chromium } from '@playwright/test'; import { existsSync } from 'node:fs'; process.exit(existsSync(chromium.executablePath()) ? 0 : 1)" \
    >/dev/null 2>&1
}
