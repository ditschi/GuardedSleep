# ADR-007: Browser E2E test strategy (Playwright)

- Status: DECIDED
- Date: 2026-05-13
- Deciders: Technical Architect, Security/Privacy Architect, Critical Reviewer (synchronous review in implementation PR)

> **Partially superseded (2026-10-20):** the hybrid (Capacitor) part → [ADR-020](ADR-020-toolchain-and-development-setup.md) (hybrid E2E removed in wave 1); browser E2E scope narrowed per [ADR-016](ADR-016-test-strategy-local-virtual-physical.md). In the web rebuild Playwright adds fake media devices, `@axe-core/playwright` and an overflow check ([ADR-020](ADR-020-toolchain-and-development-setup.md) §5).

## Context

GuardedSleep needs automated confidence that two browser clients can pair (with or without session PSK), establish WebRTC (signaling + DTLS-SRTP + DataChannel), exchange control messages (`listen`, heartbeats, `noise_alert`), and move audio baby → parent. Vitest covers parsing and small units; it does not exercise real browsers, `getUserMedia`, or ICE.

## Options Considered

### Option A: Playwright (Chromium-first)

Two `BrowserContext` instances against `pnpm run dev:local` (Vite + signaling). Chromium flags fake microphone input (`--use-fake-device-for-media-stream`, `--use-file-for-fake-audio-capture`). Assertions use `RTCPeerConnection.getStats()` and a **dev-only** `window.__guardedsleep_test` bridge (stripped from production static builds via `import.meta.env.DEV`).

- **Pros**: Mature multi-page API, built-in `webServer`, trace/video on failure, TypeScript, good CI story.
- **Cons**: Chromium-only in phase 1; WebRTC can be flaky in CI without careful timeouts; dev hook must not weaken production.

### Option B: Cypress / WebdriverIO

- **Pros**: Familiar to some teams; WebdriverIO can drive multiple browsers.
- **Cons**: Multi-context WebRTC ergonomics weaker than Playwright for this repo; more boilerplate for two-role flows.

### Option C: No browser E2E (signaling smoke + manual only)

- **Pros**: Zero flake, no dev hooks.
- **Cons**: Regressions in WebRTC / UI wiring would only surface manually; PoC gate in ADR-001 stays harder to automate.

## Decision

Adopt **Option A (Playwright)** in a dedicated workspace `apps/e2e` (`@guardedsleep/e2e`), **Chromium only** for the first iteration. Tests start the local stack via Playwright `webServer`. Pairing coverage is split:

1. **Deep-link smoke** (`/baby` + `/parent` with same `room` + `signal`): no session PSK → `PeerSession` runs **without** `pair_proof` (pairing optional when `options.pairing` is absent).
2. **Paste-invite flow** (PoC-acceptable per ADR-001): parent pastes compact invite JSON; both sides share PSK in `sessionStorage` and complete `pair_proof`.

Add **dev-only** `window.__guardedsleep_test` (guarded by `import.meta.env.DEV`) to expose `PeerSession` probes (`getE2ePeerConnection`, `sendListen`, pairing state) and baby-side noise-gate helpers. Production `vite build` must not ship this surface (DCE on `import.meta.env.DEV` blocks).

## Critical review (gate)

| Finding (severity)                                                        | Resolution                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CRITICAL**: Dev hook could leak into production                         | Mitigation: all hook registration lives in `import.meta.env.DEV` branches in routes; `PeerSession` probe methods return `null` / `false` when not `DEV`. Verified by `pnpm --filter @guardedsleep/mobile build` + inspection that production bundle contains no `__guardedsleep_test` string (spot-check). |
| **IMPORTANT**: Playwright traces may capture PSK in screenshots / storage | Mitigation: `trace: 'retain-on-failure'` only; document no trace upload for secrets-heavy local runs; pairing tests use ephemeral rooms.                                                                                                                                                                   |
| **IMPORTANT**: CI stability (WebRTC, fake audio)                          | Mitigation: generous timeouts, looped short WAV fixtures, no hard latency SLA in phase 1 (metrics logged only).                                                                                                                                                                                            |
| **SUGGESTION**: Add Firefox/WebKit later                                  | Deferred: track as follow-up after Chromium baseline is green.                                                                                                                                                                                                                                             |

Expert response: **Accepted** all mitigations above. Critical reviewer: **second pass not required** (no unresolved disputes).

## Consequences

### Positive

- Automated regression signal for signaling + WebRTC + DataChannel + audio gate.
- Clear home for PoC-style scenarios (paste JSON) aligned with ADR-001 PoC allowances.

### Negative

- CI runtime and maintenance cost (browser install, flake triage).
- Dev-only surface area in mobile routes (small but real).
- Playwright starts signaling on **port 18787** by default (via `PORT` in `webServer.env`) so it does not collide with a developer `dev:local` on **8787**; tests and `localStorage` default must match (`E2E_SIGNALING_URL` in `playwright.config.ts`).

### Risks

- **Flake**: WebRTC timing — mitigate with retries only where justified, stable selectors (`data-testid`), and log stats on failure.
- **False confidence**: Browser E2E does **not** replace Android/Capacitor plugin tests (superseded, see ADR-016/ADR-020: native emulator gate replaces them) — document scope in `apps/e2e/README.md`.

## Discarded alternatives

- **Cypress primary driver** — Rejected: weaker multi-context story for two-role WebRTC (see Option B).
- **QR bitmap pipeline in CI** — Rejected for phase 1: brittle; paste-invite covers PSK + `pair_proof` path.
