# Proposal

## Why

The web client is the frozen SvelteKit + Capacitor-era `apps/mobile` with hand-written protocol types. ADR-019 makes the web client permanent and full in both roles (a laptop with its internal microphone is a supported baby unit, owner 2026-10-20), and ADR-020 §5 decides the rebuild on Vite 8 + Svelte 5 on the WASM core. Tier T0, wave 3.

## What Changes

- New `apps/web` (Vite 8, Svelte 5, vite-plugin-pwa, TypeScript strict, no SvelteKit), a greenfield build on the WASM `Link`, as baby unit and parent unit; plain CSS with own components, History API routing.
- New capability spec `web-client` with the browser-specific behaviour (keep-awake setup, alert sound unlock, night screen, background-loss reporting, silence status "unknown", copy code without sharing, memory-only pairing material, CSP).
- The v3 invite codec moves into the core and is used by both clients through their bindings; Android's Kotlin parser is deleted.
- Web clients use the relay's built-in STUN by default, as Android does.
- Web checks: `svelte-check --fail-on-warnings`, Vitest for logic, Playwright with fake media, axe and overflow checks, a storage test for pairing material, pnpm 11 settings.
- `apps/mobile` is deleted at parity (`packages/protocol` is already gone with `add-rust-signaling-server`); the web image builds `apps/web`.

## Capabilities

### New Capabilities

- `web-client`: the browser client in both roles and how it stays trustworthy without native background guarantees.

### Modified Capabilities

- `dev-toolchain`: web checks.
- `pairing`: one invite codec in the core.
- `signaling-relay`: client ICE configuration covers the web client.

## Impact

`crates/guardedsleep-core` (invite codec, vectors), `crates/guardedsleep-ffi`, `crates/guardedsleep-wasm`, `packages/core-wasm` (API snapshots), Android `platform` pairing, `apps/web` (new), `apps/mobile` (deleted), `apps/e2e`, `docker/mobile-web` (renamed to `docker/web`), `docker-web.yml`, `scripts/dev.sh`, `pnpm-workspace.yaml`, `locales/en.json` (new web keys), `docs/ci-gates.md`, `docs/backlog.md`.
