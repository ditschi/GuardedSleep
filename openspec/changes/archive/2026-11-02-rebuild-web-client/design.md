# Design

## Context

Sources: ADR-019 (permanent full web client, Rust as the only protocol source), ADR-020 §5 (stack, pnpm 11, lint and tests, browser security), ADR-018 §2 ("Web client" row) and §6 (laptop baby unit, "keep this unit awake"), AGENTS.md §7.2 (web: pairing material in memory only), `alpha-ux-flows.md` (web sections). Visual reference: the owner-approved mockup "GuardedSleep web mockup" (2026-10-23, seven screens S01, S04, S05, S07/S08, S09, S10, S11).

**Greenfield** (owner, 2026-10-23): the legacy web concept was never released. `apps/web` is a new build, not a port: no compatibility with the legacy `v:1` invite, its settings keys or its markup. `apps/mobile` is a read-only reference until it is deleted.

## Decisions

- **Plain Vite + Svelte 5, no SvelteKit** (owner D5): SvelteKit 3 had been GA for three weeks and agents write older patterns; the app has about five screens and no SSR. Revisit at beta.
- **Same contract models as Android**: Svelte stores mirror `UiContract.kt`; UI never imports WASM types.
- **Invite codec in the core** (owner, 2026-10-23): the v3 invite (`{"v":3,"k","s","x"}`) is parsed and built by `guardedsleep-core`, exposed through `guardedsleep-wasm` and `guardedsleep-ffi`, and both clients switch to it; Android's `PairingPayload.kt` parser is deleted. The core stays sans-IO: the caller passes the 32 random PSK bytes and the current epoch seconds. Errors are the existing `PairingError` kinds (`not_our_code`, `incomplete_paste`, `expired`). One parser for peer input, covered by conformance vectors through every binding. This closes the backlog item "Move the QR invite codec into the core".
- **Styling: modern plain CSS and own Svelte components** (owner, 2026-10-23). One `theme.css` with CSS custom properties for the dark, light and dim baby-unit themes, cascade layers, `light-dark()`, native `<dialog>`, `popover` and `role="switch"` checkboxes; about a dozen own components (button, card, status bar, chip, banner, meter, switch, dialog, list row, night screen). Material-3-like shapes so the web and Compose clients read as one product. System font stack, no web fonts. The custom properties are hand-written now and generated from `tokens.json` by `add-quality-gates`.
  - Pros: no styling dependency, tokens map 1:1, smallest bundle, simple CSP, native elements carry accessibility.
  - Cons: the components are ours to design and maintain.
- **Routing: History API** (owner, 2026-10-23). A small in-house router on `pushState`/`popstate` with clean paths (`/`, `/baby/pair`, `/baby`, `/parent/pair`, `/parent`, `/settings`). Both hosts already fall back to `index.html` (nginx `try_files`, the server's `--serve-web` `ServeDir` fallback), and vite-plugin-pwa uses `navigateFallback`. Pairing material is memory-only, so a reload of a paired screen returns to the start screen.
- **Device wording: "baby device" / "parent device"** (owner, 2026-10-23, decides open decision A8 of `alpha-ux-flows.md`): phones, tablets and laptops are all first-class. The catalogue-wide rename (Android copy, the UX doc) belongs to `expand-locales`; new web keys use the new wording from the start.
- **Copy code, no share** (owner, 2026-10-23): the baby device offers "Copy code" with the hint "Paste it on the parent device within 5 minutes. Don't send it in a chat." The invite carries the long-term PSK, not a one-time ticket, so a code sent through a messenger stays usable from chat history and backups until re-pairing. A share button waits for a post-pairing key change (backlog `IDEA`).
- **Night screen** (owner, 2026-10-23): a browser cannot dim or switch off the display, and a locked screen lets the browser freeze the tab. Both roles therefore offer a pure-black full-screen overlay while the page stays visible and holds the Screen Wake Lock: on AMOLED displays black pixels are off, a dim dot drifts slowly against burn-in, a first tap shows the status for a few seconds, and on the parent device an alert replaces the overlay at once.
- **Background-loss detection** (owner, 2026-10-23): a page cannot alert from the background, and a wake-up push would need a third-party push service (AGENTS.md §7.1). The client instead reduces and reports the loss: a system notification when the monitoring page becomes hidden (when the user granted notifications in parent setup), a leave-page confirmation while monitoring, a warning when the wake lock is released, and on return the background warning with the core's missed-events summary. Setup copy recommends the Android app for an all-night parent device.
- **Browser security**: a CSP delivered as a header by both hosts and as a `<meta>` fallback: `default-src 'self'`, `script-src 'self' 'wasm-unsafe-eval'`, `connect-src 'self'` plus the configured `ws:`/`wss:` signaling origins, `img-src 'self' data: blob:`, `object-src 'none'`, `base-uri 'none'`, `frame-ancestors 'none'`, `require-trusted-types-for 'script'` with a single named policy. No inline scripts (the theme bootstrap is an external script that runs before first paint), no third-party origins.
- **QR**: generation with `qrcode` (error correction level M, no logo), scanning with the native `BarcodeDetector` where available and `@zxing/browser` as fallback; the camera is requested only on the scan screen.
- **Vitest 5** (owner, 2026-10-23): `apps/web` starts on Vitest 5 (current `latest` 5.0.5), and `packages/core-wasm` (5 tests, no mocks) moves to it in the same step. `apps/mobile` moves only if its 237 tests pass unchanged; otherwise it stays on 4.x until group 9 deletes it. pnpm resolves a version per package, so the workspace can carry both for that period.
- **Image tags**: the web image keeps its tag rules until Release Please (T1) introduces the `app` component; a later change modifies `distribution`.

## Discarded alternatives

- **SvelteKit 3**: see the routing decision; revisit at beta.
- **Leptos or Dioxus** (Rust UI): smaller ecosystem for accessibility.
- **Compose for Web**: immature.
- **Biome or ESLint**: Prettier + `svelte-check` suffice at T0.
- **Invite codec as a TypeScript port of `PairingPayload.kt`**: a second hand-written parser for peer input (AGENTS.md §5.1).
- **Tailwind 4**: fast to build, but long class strings and a build dependency; its main advantage, reuse of the legacy markup, vanished with the greenfield decision.
- **shadcn-svelte** (Bits UI + Tailwind): polished at once, but the most dependencies, a generic look unlike the Compose app, and copied code with upstream churn.
- **Bits UI headless with plain CSS**: the five screens need no complex widgets that native elements do not cover.
- **Material Web components**: in maintenance mode.
- **Hash router**: works on any static host, but both of our hosts already provide the fallback, so clean paths cost nothing.
- **Logo in the QR code** (owner): denser code that is harder to scan when inverted at ≤ 50 % brightness.
- **Web Share button for the code**: exposes the long-term key in chat history; waits for a post-pairing key change.
- **Keep the tab alive with silent audio**: unreliable across browsers and drains the battery.
- **Start `apps/web` on Vitest 4.x**: the known version, but a planned major upgrade right after the rebuild.
- **Adapt `apps/mobile` tests to Vitest 5**: effort spent on a frozen client that group 9 deletes.
- **Web Push to wake a closed parent tab**: needs a third-party push service in the alert path.

## Risks / Trade-offs

- Browsers throttle and freeze background tabs: the client says so plainly, notifies when it is hidden and warns after the page was in the background.
- A browser parent device cannot be made as reliable as the Android app; the setup screen says so and recommends the app for all-night use.
- Moving the invite codec into the core touches the Android pairing path; the conformance vectors and the existing `PairingTest` cases guard the switch.
