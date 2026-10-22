# Proposal

## Why

The web client is the frozen SvelteKit + Capacitor-era `apps/mobile` with hand-written protocol types. ADR-019 makes the web client permanent and full in both roles (a laptop with its internal microphone is a supported baby unit, owner 2026-10-20), and ADR-020 §5 decides the rebuild on Vite 8 + Svelte 5 on the WASM core. Tier T0, wave 3.

## What Changes

- New `apps/web` (Vite 8, Svelte 5, vite-plugin-pwa, TypeScript strict, no SvelteKit) on the WASM `Link`, as baby unit and parent unit.
- New capability spec `web-client` with the browser-specific behaviour (keep-awake setup, alert sound unlock, background warning, silence status "unknown", memory-only pairing material, CSP).
- Web checks: `svelte-check --fail-on-warnings`, Vitest for logic, Playwright with fake media, axe and overflow checks, a storage test for pairing material, pnpm 11 settings.
- `apps/mobile` and `packages/protocol` are deleted at parity; the web image builds `apps/web`.

## Capabilities

### New Capabilities

- `web-client`: the browser client in both roles and how it stays trustworthy without native background guarantees.

### Modified Capabilities

- `dev-toolchain`: web checks.

## Impact

`apps/web` (new), `apps/mobile` and `packages/protocol` (deleted), `apps/e2e`, `docker/mobile-web` (renamed or rebuilt), `docker-web.yml`, `pnpm-workspace.yaml`, `docs/ci-gates.md`.
