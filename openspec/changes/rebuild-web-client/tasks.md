# Tasks

Greenfield build of `apps/web` (design.md). `apps/mobile` is read-only reference until group 9 deletes it. Visual reference: the approved mockup (design.md, Context). Layer rules: AGENTS.md §5.1; copy only in `locales/en.json` (§5.2a); test ids `snake_case` as in `alpha-ux-flows.md` §4. Run `just test-affected` after each task, commit after each task.

## 1. Invite codec in the core

- [x] 1.1 Add `invite` to `guardedsleep-core`: `build_invite(psk, urls, expires_at_epoch_s)` and `parse_invite(text, now_epoch_s)` returning the PSK bytes, URLs and expiry or an `InviteError` (`NotOurCode`, `IncompletePaste`, `Expired`), following the v3 rules of the pairing spec (32-byte unpadded base64url PSK, non-empty `ws://`/`wss://` URLs with a host, positive expiry, trimmed input); no `Debug` output of the PSK; parser-module lint denies (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`); unit tests and a proptest that `parse(build(x)) == x`. Verify: `scripts/cargo.sh test -p guardedsleep-core`.
- [x] 1.2 Add `parse_invite` and `build_invite` conformance vectors (valid, wrong version, short PSK, bad URL, missing expiry, expired, cut-off JSON, not JSON, surrounding whitespace) to `vectors/conformance.json` and `conformance_eval`. Verify: core tests and `just test-wasm` pass with the new cases.
- [x] 1.3 Export the codec from `guardedsleep-ffi` and `guardedsleep-wasm` and the `packages/core-wasm` wrapper (typed result, no raw JSON in callers); `just api-snapshot`, review the diff. Verify: `just api-check`, `pnpm --filter @guardedsleep/core-wasm test`.
- [x] 1.4 Switch Android to the core codec: `LinkController` and the debug launcher call the binding; delete the parser in `PairingPayload.kt` (keep `PairingError` mapping in `platform`); port `PairingTest` cases to the binding. Verify: `just test-android` green, `rg -n 'JSONObject' apps/android/platform/src/main/kotlin/org/guardedsleep/app/pairing` finds no invite parsing.
- [x] 1.5 Remove the "Move the QR invite codec … into the core" items from `docs/backlog.md`; add the `IDEA` "Post-pairing key change so a shared code is single-use; then a Web Share action for the invite" under pairing. Verify: `just lint-text`.

## 2. Scaffold apps/web

- [x] 2.1 Create `apps/web` (`@guardedsleep/web`): Vite 8, Svelte 5 (runes), vite-plugin-pwa (`navigateFallback`, no inline register script), TypeScript strict, `svelte-check --fail-on-warnings`, Vitest; scripts `dev`, `build`, `check`, `test`, `i18n:gen` (i18n-gen `web` into `src/lib/i18n/generated`, git-ignored); add dependencies through pnpm only (`qrcode`, `@zxing/browser`), justify them in the PR. Vitest 5 (design.md); in the same step move `packages/core-wasm` to Vitest 5, and `apps/mobile` only if its tests pass without changes (otherwise it stays on 4.x until 9.2). Verify: `pnpm --filter @guardedsleep/web build` and `check` pass on an empty shell app; `pnpm --filter @guardedsleep/core-wasm test` green on Vitest 5; the PR states whether `apps/mobile` moved.
- [x] 2.2 Wire the workspace: root `test`/`build`/`check` pick up `apps/web`; `scripts/dev.sh` starts `apps/web` (port 5173) with the Rust server; `.github/path-filters.yml`, `.gitignore`, `.prettierignore` and `scripts/lib/affected*` know `apps/web`. Verify: `just dev` serves the shell (stopped by its trap), `node scripts/lib/affected.mjs --dry-run` maps `apps/web/**`.

## 3. App shell: router, theme, i18n, stores

- [x] 3.1 History API router (`/`, `/baby/pair`, `/baby`, `/parent/pair`, `/parent`, `/settings`) with focus on the screen heading and `document.title` per route; Vitest for matching and fallback to `/`. Verify: Vitest.
- [x] 3.2 `theme.css`: custom properties for dark (default), light and the dim baby theme, cascade layers, system font stack; external `theme-boot.js` applies the theme before first paint; base components (button, card, status bar, chip, banner, meter, switch, dialog, list row) per the mockup. Verify: `check` passes, a Vitest/DOM test renders each component with its label.
- [x] 3.3 Typed i18n: a `t(key, args)` over the generated web catalogue with ICU formatting and English fallback for non-critical keys; new web copy keys added to `locales/en.json` only, using "baby device" / "parent device"; `scripts/lint-ui-literals.sh` covers `apps/web/src`. Verify: Vitest for formatting, `just lint-text` (literal lint) green.
- [x] 3.4 Contract stores mirroring `UiContract.kt` (`AppState`, `LinkUi`, `ParentMonitorState`, `BabyMonitorState`, `PairingState`, `SettingsState`) and an `AppController` interface; UI imports only these. Verify: Vitest; `rg -n "core-wasm" apps/web/src/lib/ui apps/web/src/screens` is empty.

## 4. Platform: link driver on the WASM core

- [x] 4.1 Link driver: monotonic clock, `WasmLink` input/action loop, signaling WebSocket, one `RTCPeerConnection` per attempt, DTLS fingerprint from `getStats()` (`attempt_dtls_fingerprint`), ICE servers = the relay's built-in STUN only (no sipgate/linphone/Google defaults). Fake-environment tests for attempt lifecycle, reconnect and fingerprint report. Verify: Vitest.
- [x] 4.2 Audio: baby microphone capture with RMS reporting (`audio_level`) and mic enable/disable; parent playback for listen and alerts; alert tones after the sound-unlock gesture. Verify: Vitest with fake media.
- [x] 4.3 Map core status/events onto the contract stores (link phase, level, cause, last contact, peer status, noise alert, missed events, listen, snooze, probe). Verify: Vitest table tests per event.

## 5. Pairing screens

- [x] 5.1 S01 role screen and S04 baby pair: fresh 32-byte PSK from `crypto.getRandomValues`, invite from the core codec, QR (level M, inverted, ≤ 50 % brightness, "Make brighter"), countdown and "Show new code", "Copy code" with the no-chat hint, no share action. Verify: Vitest for the countdown; Playwright test ids `baby_pair_*`.
- [x] 5.2 S05 parent pair: camera only after "Allow camera", `BarcodeDetector` with `@zxing/browser` fallback, paste field, error copy per `PairingError`, connecting state. Verify: Vitest for error mapping; Playwright paste flow.
- [ ] 5.3 Pairing material in memory only; a Playwright storage test pairs, reloads and asserts `localStorage`, `sessionStorage`, IndexedDB and Cache API hold no PSK, invite or room id. Verify: `just e2e` storage test.

## 6. Monitoring screens

- [ ] 6.1 S07 baby monitor (status, mic level, battery from the Battery API where present, silence `unknown`, keep-awake card) and S09 parent monitor (ambient bar with colour + icon + text, headline, meter, chips, Listen ≥ 72 px, Snooze, Test alert, More, Stop in More) per `alpha-ux-flows.md` §4–§5. Verify: Playwright with fake media; axe has no violations.
- [ ] 6.2 S10 noise alert and S11 link alarm in page, priority per §5 (one full-screen at a time), document title flash, Notification API when granted. Verify: Playwright `05`-style noise-alert and link-loss tests.
- [ ] 6.3 S12 settings (signaling URL list, theme, sensitivity on the baby device, forget pairing). Verify: Playwright settings flow.

## 7. Browser-specific behaviour

- [ ] 7.1 Screen Wake Lock while monitoring (re-acquired on visibility return), keep-awake setup on the baby device, "Tap to turn on alert sound" gate on the parent device. Verify: Playwright with a stubbed `navigator.wakeLock`.
- [ ] 7.2 Night screen on both roles: black overlay, drifting dim dot (static under `prefers-reduced-motion`), first tap shows status, second tap closes, parent alerts replace it. Verify: Playwright.
- [ ] 7.3 Background-loss reporting: notification on `visibilitychange` → hidden while monitoring (when allowed), `beforeunload` confirmation while monitoring, warning on wake-lock release, background warning with missed events on return, resume-from-sleep warning (time gap). Verify: Playwright emulating visibility changes; Vitest for the time-gap detector.
- [ ] 7.4 CSP header (nginx, `--serve-web`) and `<meta>` fallback as in design.md, Trusted Types policy, no inline script. Verify: Playwright asserts no CSP violation on every screen and that an injected cross-origin script is blocked.

## 8. Web checks

- [ ] 8.1 Playwright suite against `apps/web` with fake media devices: rewrite `apps/e2e` tests to the new test ids (pairing, audio flow, noise alert and no-alert cases, listen, reconnect, wrong PSK, signaling unreachable, recovery, storage); delete tests for legacy-only behaviour (deep-link invites, v1 paste). Verify: `just e2e` green.
- [ ] 8.2 axe on every screen and a text-overflow check at 320 px width and 200 % text size. Verify: `just e2e`.
- [ ] 8.3 pnpm settings per the `Web checks` requirement (allow-list, minimum release age, exotic sub-deps blocked, frozen lockfile in CI) are in force for the new dependencies. Verify: `pnpm install --frozen-lockfile` and `just lint`.
- [ ] 8.4 `docs/ci-gates.md` rows for svelte-check, the storage test, the CSP test and axe (owner, runbook, last catch). Verify: `just lint-text`.

## 9. Switch over and delete the legacy client

- [ ] 9.1 Rebuild the web image on `apps/web` (rename `docker/mobile-web` to `docker/web`, CSP header in nginx, update `docker-web.yml`, `docker/stack/*`, `just stack-*`). Verify: `just stack-lan` builds (or prints `SKIPPED` without Docker; report it).
- [ ] 9.2 Delete `apps/mobile` and every reference (AGENTS.md §2/§3, README, CONTRIBUTING, `docs/contributing/*`, `renovate.json`, `release-please-config.json`, `.release-please-manifest.json`, `.betterleaks.toml`, `mise.toml`, `.claude/agents/*`, `crates/i18n-gen` output paths, `scripts/*`); `git grep -n "apps/mobile\|@guardedsleep/mobile"` outside the archive is empty. Verify: the grep, `just lint`.
- [ ] 9.3 Integrated check on the wave branch: `GS_STRICT=1 just ci` green.

## Workflow follow-up

- Role reviews: UX (screens against the mockup), Security/Privacy (memory-only PSK, CSP, invite codec), Technical Architect (core codec, layers), then the Critical Reviewer gate (pairing spec and core change).
- refactor-scout cleanup pass in diff mode against the wave branch.
- `mise exec -- openspec archive rebuild-web-client --yes`, then `openspec validate --all --strict`.
