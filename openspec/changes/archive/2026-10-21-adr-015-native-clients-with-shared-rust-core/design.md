# ADR-015: Native Clients with a Shared Rust Core

- Status: DECIDED (owner, 2026-10-21; PoC gate AC-1…AC-4 passed 2026-10-20, AC-10 next)
- Date: 2026-10-19
- Deciders: Project owner (pending), Technical Architect, Security/Privacy Architect, UX, Product Owner, Critical Reviewer (perspectives applied by agent)
- Supersedes: ADR-002 (client platform and signaling server stack) — **only once this ADR is DECIDED** and the PoC gate below passes
- Related: [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md) (test strategy; owns all test details, this ADR keeps the PoC gate). **ADR-015 and ADR-016 are decided together.**
- Owner decisions 2026-10-19: English, German, French and Spanish are officially supported from Phase 1 (see Localization; superseded, see ADR-021 §2: about 20 AI-translated locales); link recovery is designed first and then **gated in the PoC as AC-10** (automatic reconnect after Wi-Fi loss/network change, on both roles; see Link recovery)

> **Partially superseded (2026-10-20):** the web row (SvelteKit → Vite + Svelte 5), the signaling row (Node stays until parity → Rust axum now), the WebRTC vendor (`stream-webrtc-android` → `io.github.webrtc-sdk:android` behind a `MediaEngine`) and the four-locale owner decision → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §4–§6 and [ADR-021](../../../../docs/decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §2. Status stays PROPOSED until its gate passes.

## Context

ADR-002 chose a single SvelteKit web app wrapped by Capacitor for Android (and later iOS). Since then, the scaffold has shown where that approach strains:

- The Android shell already carries ~1,200 lines of native Java (`GuardedSleepNativePlugin`, `LanSignalingWebSocketServer`, `NativeSignalingWebSocket`, …) plus a foreground-service plugin, NSD discovery, alert audio routing, full-screen notifications, keep-awake and immersive mode. Much of the reliability-critical behaviour is already native, connected through a JS bridge.
- Media capture and WebRTC still run **inside the WebView**. Whether that survives Doze, screen-off, and OEM battery managers is outside our control, and it is the component whose failure defeats the product's core promise (the alert must arrive).
- **iOS:** WKWebView suspends `getUserMedia` / WebRTC when the app is backgrounded. A parent unit that stays awake requires a native `AVAudioSession` with the `audio` background mode. Push via APNs would route per-event alerts through a third party, violating privacy guardrail #1. A Capacitor plugin solving this would amount to a native media path anyway.
- **Testability:** the web UI tests well in Playwright, but the risks that matter (background survival, Doze, audio routing, notifications, bridge lifecycle) are invisible to browser tests. Testing the packaged app needs Appium with `WEBVIEW` ↔ `NATIVE_APP` context switching, which is slow and flaky.

The project owner asked to re-evaluate the client architecture from scratch, explicitly considering that GenAI lowers the cost of writing per-platform UIs.

## Options Considered

### Option A: Keep web app + Capacitor (status quo, ADR-002)

- **Pros**: single UI codebase; most existing work reused; fast iteration; Playwright coverage.
- **Cons**: WebView owns the media path; background reliability uncertain on Android and practically unachievable on iOS; growing native plugin surface with a three-layer debug story (JS ↔ bridge ↔ native); end-to-end tests on device are hard.

### Option B: Fully independent native apps (Android, iOS) + separate web app

Each client implements protocol, pairing, state machine and alert logic in its own language; GenAI writes them in parallel.

- **Pros**: best OS integration; no bridge; idiomatic UX per platform.
- **Cons**: security-critical logic (pairing, PSK handling, fingerprint binding, heartbeat/watchdog, alarm thresholds) exists **three times**. Drift between implementations becomes the dominant bug source; review, test and audit effort triples. GenAI lowers the cost of _writing_, not of _verifying_.

### Option C: Shared Rust core + native UI per platform

A `sans-IO` Rust crate owns all logic that must behave identically everywhere. Platforms supply I/O (sockets, timers, WebRTC, audio) and render UI.

- **Pros**: one implementation and one test suite for security/protocol logic; UniFFI generates Kotlin and Swift bindings, `wasm-bindgen` + `tsify` generate TypeScript for the web; the same crate can back the signaling server and the on-device LAN signaling server (today Java-only); strong crypto ecosystem (RustCrypto, `snow`, SPAKE2); deterministic, fast, fuzzable tests.
- **Cons**: more complex build (cargo-ndk, XCFramework, WASM); FFI boundary design needs care; Rust learning curve; three UIs to maintain.

### Option D: Kotlin Multiplatform shared core + Compose / SwiftUI

- **Pros**: mature; one language on Android; good iOS interop; `webrtc-kmp` exists.
- **Cons**: web target (Kotlin/Wasm or JS) is weaker than Rust → WASM; the signaling server cannot share the core as naturally; thinner crypto ecosystem.

### Option E: Flutter or React Native

- **Pros**: one UI codebase; `flutter_webrtc` is mature.
- **Cons**: background audio and foreground services again depend on plugins — Option A's problem in a different shape; weaker OS integration than native.

## Decision

**Proposed: Option C**, gated by the PoC below. If the Rust build/FFI cost proves prohibitive in the PoC, fall back to **Option D** with the same layering.

Rationale: the UI is small (~7 screens) and highly platform-specific, so native UI is the right tool and is cheap to produce. The logic is security-critical and must be identical everywhere, so it lives exactly once. Rust is chosen for **portability** (Android, iOS, browser via WASM, server), not for raw performance — audio encoding happens inside libwebrtc regardless — and not primarily for memory safety, which Kotlin, Swift and TypeScript also provide.

### Target stack

| Area                | Choice                                                                                                                                                                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Core                | Rust, **sans-IO** (no internal networking or async runtime; platform feeds events in, core returns actions)                                                                                                                            |
| Core scope          | Protocol types (serde), pairing / PSK / fingerprint binding, session state machine, heartbeat/watchdog, alert thresholds, LAN signaling logic                                                                                          |
| Bindings            | UniFFI → Kotlin + Swift; `wasm-bindgen` + `tsify` → TypeScript                                                                                                                                                                         |
| Media               | Platform libwebrtc (Android: maintained prebuilt such as `stream-webrtc-android` or LiveKit builds; superseded, see ADR-020 §4: `io.github.webrtc-sdk:android`; iOS: WebRTC.xcframework; web: browser). **Not** `webrtc-rs` for media. |
| Android             | Kotlin + Jetpack Compose; foreground service type `microphone`; `NsdManager`; Android Keystore                                                                                                                                         |
| iOS (after Android) | Swift + SwiftUI; `AVAudioSession` background `audio`; Network.framework / Bonjour; Keychain / Secure Enclave                                                                                                                           |
| Web                 | SvelteKit (superseded, see ADR-020 §5: Vite + Svelte 5) (existing UI becomes the web client) + core via WASM + browser WebRTC                                                                                                          |
| Signaling server    | Rust (`axum` + `tokio-tungstenite`), sharing the protocol crate. Node implementation stays until parity is proven (superseded, see ADR-020 §6: Rust server now, Node deleted).                                                         |
| Localization        | One platform-neutral catalogue (`locales/`), generated into native resources per target; the core emits message keys, never prose (see "Localization" below)                                                                           |
| Tests               | Local → virtual → physical, device-agnostic physical runner; see [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md)                                                                                       |

### Invariant: signaling server deployment

Regardless of implementation language, the signaling server keeps its current deployment shape: **one plain HTTP port inside the container**, WebSocket via HTTP `Upgrade` on any path, TLS terminated by the operator's reverse proxy on 443 (domain- or path-based routing, Traefik labels supported). No extra public ports are introduced for signaling. A self-hosted TURN server, if used, is a separate optional service with its own ports (see ADR-003).

### Localization: one source of truth

**Supported languages (owner decision 2026-10-19; superseded by ADR-021 §2, about 20 AI-translated locales): English (source and fallback), German, French, Spanish — officially supported from Phase 1**, in every client and for all alert, connection-loss and notification copy. Today the app has these four locales in `apps/mobile/src/lib/i18n/messages/*.json` (`en` is the master, `{{name}}` placeholders) with a key-presence parity test (there is **no** plural handling yet). With three UIs (Compose, SwiftUI, Svelte) and possible later desktop targets (macOS, Windows, Ubuntu), per-platform copies would drift exactly as duplicated protocol logic would. Translations are therefore managed **once** and reused in every deployment.

- **Single catalogue:** `locales/<tag>.json` at the repository root, `en` as master. It is platform- and framework-neutral and is the only place where end-user copy is written or translated. Moving it out of `apps/mobile` is part of the migration.
- **Message syntax:** named arguments plus plural and select forms (ICU MessageFormat subset). The subset is fixed in the PoC under one constraint: every message must map losslessly to Android `strings.xml` (incl. `<plurals>`), Apple String Catalogs (`.xcstrings`) and the web runtime. Rules that keep the mapping lossless: no HTML or markup in messages (the generator escapes XML/Apple special characters, `%`, `'`, `@` and literals like `<PC-LAN-IP>`); `select` (gender) is **not** in the subset — use separate keys; the generator reorders arguments into Android's positional form (`%1$s`) per locale and sorts them by name. The PoC publishes a table of ICU feature → Android / Apple / web mapping with a golden-file test. Existing `{{name}}` messages are converted mechanically; messages that need plurals (e.g. retry counters with a count) are listed and converted explicitly.
- **Generator, not copies:** a build step (`tools/i18n-gen`, language chosen in the PoC) emits per-target resources: Android `res/values-*/strings.xml`, Apple `.xcstrings`, web JSON bundle, and later desktop formats. Generated files are build artifacts and are not committed; CI fails if generation is not reproducible.
- **The core never emits prose.** User-visible outcomes (connection states, alert kinds, errors) are returned as `UserMessage { key, args }` or as enums with a stable key mapping owned by the core. UIs only call `t(key, args)`. Consequence for tests: conformance vectors and traces are locale-independent, and wording changes never touch core tests.
- **Formatting stays native:** locale selection (including per-app language: Android 13+ `LocaleManager` with a generated `locales_config.xml`, iOS per-app language), numbers, dates, units and relative times use platform APIs, not the core. The UI formats numbers and units _before_ passing them as `args`, so the core never returns untranslated number formats (e.g. the `×` suffix and decimal separators).
- **Headless copy:** the Android foreground service, the Doze alert and the "baby is crying" notification run without an Activity. A `Messages.resolve(UserMessage)` helper usable from a Service/`NotificationManager` context resolves text with the per-app locale (not the Activity context); notification channel names, the manifest label (`@string/app_name`) and accessibility `contentDescription`s are generated from the same catalogue. Alert keys are bundled with the app and never fetched lazily. Test: set the app locale to `de` and assert that notification text and channel names are German.
- **Store listings** (Play, App Store) are not generated from the catalogue; they live in per-locale metadata directories (e.g. fastlane) and are maintained manually (`STATUS: IDEA` to automate).
- **Right-to-left** languages are out of scope for now; UIs use start/end rather than left/right so that adding them stays cheap.
- **Fallback policy:** at runtime, missing non-alert strings may fall back to English; for **alert and connection-loss keys, CI fails** if any supported locale lacks the key.
- **Quality gates (all in CI):** key and placeholder parity of every locale against `en` (the current `parity.test.ts` logic moves into the generator check), and **plural-category completeness per locale according to CLDR** (not `en`'s categories: e.g. French and Spanish use `many` where English does not); English `typos` and native-dictionary CSpell as today; a generated **pseudo-locale** (accented, ~40 % longer) for overflow screenshots and for spotting hard-coded strings; a lint rule that forbids string literals in UI code (Android Lint `HardcodedText` is not assumed to cover Compose `Text("…")`; Detekt or a custom lint rule is named in the PoC; SwiftLint custom rule for iOS). Screenshot tests run in the pseudo-locale, at 200 % font scale and at narrow width.
- **Translator workflow:** plain JSON stays compatible with hosted translation tools (`STATUS: IDEA`, not decided here).
- **Tests do not select UI by copy:** UI elements carry stable IDs (see [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md)).

### Test strategy

Defined in [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md). In short:

1. **Local first:** the sans-IO core is the main test target — unit, property and fuzz tests, a deterministic two-peer system simulation with fault injection, scenario traces replayed by every binding (Kotlin, Swift, WASM). A device-side trace recorder is deferred (ADR-016 §9).
2. **Virtual second:** emulator matrix across API levels, an OEM-chaos `adb` set, Maestro flows by stable IDs, and the web client as interop peer and media harness (ADR-007, narrowed).
3. **Physical last and device-agnostic:** one runner executes capability-tagged scenarios on **any** attached phone and produces a local report; no scenario names a specific model. A **mandatory overnight soak (≥ 8 h) on at least one physical Android device** gates every milestone release; without it a release is labelled "not soak-tested" and not tagged stable (ADR-016 §5). Community reports and an in-app self-test with the same schema are deferred.
4. **Cloud device farms are optional**, never a gate: the app is free and has no telemetry, so unverified OEM behaviour is handled by disclosure in release notes, borrowed devices and user reports.

### Link recovery (open design item, PoC-gated by AC-10)

`STATUS: UNDER DISCUSSION` — owner decisions 2026-10-19: design this first (C1), and make it part of the PoC gate (**AC-10**). Reconnect after a lost link never worked in the web/Capacitor client either (no reconnect, backoff or ICE-restart logic in `peer-session.ts` / `ws-client.ts`; Playwright spec 09 only restarts both sides manually), so this is new behaviour, not a regression fix.

In the current core (`session.rs`), `PeerConnectionState` `failed`/`closed` ends the session permanently (`finished = true`); there is no reconnect, ICE restart or path switch, and a Wi-Fi flap on the baby unit ends in a terminal `Failed`. For a baby monitor, silently ending the session is a product-level problem (failure-model F-1…F-5, ADR-012). The design must decide: which side restarts, ICE restart vs new session, how pairing (`pair_proof`) is re-established, interaction with the LAN path selector and the parent's "connection lost" alarm timing (ADR-013), and what the Android service does while recovering. Output: a section in this ADR or a follow-up ADR, and simulation scenarios in ADR-016. The scenarios (below) are written **test-first** as an expected-failing specification before the design is implemented, so the current behaviour is documented and the design has concrete targets. Tracked as a ticket (owner and decision date to be set); the PoC cannot pass before AC-10 does.

Scenarios the design must cover (minimum): Wi-Fi off/on on the baby unit; Wi-Fi off/on on the parent unit; switching between access points or 2.4/5 GHz; Wi-Fi ↔ mobile data (internet-signaling path); router restart; signaling-server restart; short outage (< heartbeat stale time) vs long outage (> stale time, parent alarm must fire); process death and restart (`force-stop`) of either role; time jumps after Doze.

## PoC Gate (acceptance criteria)

Scope: Rust core (pairing state machine + session/heartbeat) → UniFFI → minimal Compose app (baby + parent roles) → native libwebrtc audio, talking to the existing signaling server and to the existing Svelte web client.

The PoC passes when **all** of the following hold in CI on two Android emulators (API 34+, `x86_64`), orchestrated by a shell script around Maestro. The emulators sit behind separate NATs, so the test stack connects them through a TURN relay (coturn) unless the emulator's virtual Wi-Fi link proves sufficient. The wider API-level matrix and the long soak belong to [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md), not to this gate.

| ID    | Criterion                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-1  | Pairing via the core completes; the wrong-PSK case is rejected with the expected error on both devices.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| AC-2  | Baby unit streaming, both emulators screen-off, battery unplugged (`dumpsys battery unplug`) and forced into Doze (`dumpsys deviceidle force-idle`) for **30 minutes** (PoC smoke; multi-cycle soak is ADR-016); the baby foreground service is still running and the parent still receives audio frames afterwards.                                                                                                                                                                                                                                                                               |
| AC-3  | After AC-2, a cry sample injected on the baby unit produces a visible parent alert (heads-up / full-screen notification) within **≤ 5 s** (placeholder; align with ADR-013 escalation timing).                                                                                                                                                                                                                                                                                                                                                                                                     |
| AC-4  | Killing the baby process (`am force-stop` or `kill -9`; `am kill` does not terminate a foreground-service process) makes the parent show the "connection lost" alert within heartbeat timeout + 2 s.                                                                                                                                                                                                                                                                                                                                                                                               |
| AC-5  | The same core conformance vectors pass on the JVM (Kotlin binding) and in Node (WASM binding).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| AC-6  | `cargo-ndk` builds `arm64-v8a` and `x86_64` libraries reproducibly in CI; the debug APK size increase stays under 15 MB.                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| AC-7  | The Svelte web client (core via WASM) can act as the parent for the native baby unit. Automated (headless Chromium with fake audio) preferred; a manual check is acceptable for the PoC.                                                                                                                                                                                                                                                                                                                                                                                                           |
| AC-8  | Localization: the generator produces the Android and web resources from `locales/`; key/placeholder/plural parity and pseudo-locale generation pass; no string literal in Compose UI code; plural-category check per CLDR locale passes; the notification locale test (headless copy) passes; Maestro flows reference stable IDs only.                                                                                                                                                                                                                                                             |
| AC-9  | Host-side system simulation (two cores, simulated signaling, seeded faults) reproduces the AC-1 and AC-4 logic in under 60 s (first the current state machine; the link-recovery scenarios of AC-10 are added as soon as the design is decided and are gating for the PoC). Committed `golden`/`simulated` traces replay identically through the Rust, Kotlin and WASM bindings; traces use a synthetic PSK, never real pairing material.                                                                                                                                                          |
| AC-10 | **Link recovery:** after Wi-Fi is switched off and on again on the baby unit, and separately on the parent unit (emulators: `svc wifi disable/enable`; at least one real-device run in Stage 3), the session re-establishes automatically, including `pair_proof`, without user action and without re-scanning the QR code (failure-model: no QR re-scan in F-1/F-3/F-5). Audio resumes. An outage longer than the heartbeat-stale time raises the parent's "connection lost" alarm; an outage shorter than it does not. The same scenarios exist as host simulations (AC-9) and pass there first. |

**Audio injection:** emulator microphones are unreliable in CI. Debug builds expose a test-only audio source (e.g. WAV playback triggered by `adb shell am broadcast -a org.guardedsleep.app.debug.INJECT_AUDIO`), mirroring the dev-only `window.__guardedsleep_test` hook from ADR-007. It only proves the `AudioRecord` → core wiring, foreground service and notifications; noise/cry detection itself is tested in the core with WAV vectors. The receiver must be compiled out of release builds, and a CI step inspects the release APK to enforce it (ADR-016).

### PoC implementation map

| Criterion | Where                                                                                                                                                                                                                                                                                                                        |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-1…AC-4 | `scripts/poc/env-up.sh` (two emulators, signaling, coturn) + `scripts/poc/gate.sh`; Maestro flows in `apps/android/maestro/` select by stable ID (`status_pairing_verified`, `status_failed_<key>`, `alert_baby_crying`, `alert_link_lost`). Latencies are measured from `GS_POC` debug log events, not from Maestro timing. |
| AC-5      | Golden vectors `crates/guardedsleep-core/vectors/conformance.json`, replayed by `cargo test`, `apps/android/core` JVM test (UniFFI) and `crates/guardedsleep-core/js` (WASM, `node --test`). The same vectors are also checked against today's TypeScript pairing/parsing code (`core-conformance.test.ts`).                 |
| AC-6      | `scripts/core-build.sh android` + `scripts/poc/size-check.sh`                                                                                                                                                                                                                                                                |
| AC-7      | Native baby shows the compact invite JSON; the web client pastes it as parent. Manual for now.                                                                                                                                                                                                                               |
| ADR-016   | `scripts/poc/release-check.sh` fails if the injection receiver or its action is in the release APK.                                                                                                                                                                                                                          |

Workflow: `.github/workflows/android-native.yml` (core + Android on every change; the emulator gate manually and nightly).

**Deviation (open):** the debug injection overrides the RMS level fed to the core (`InjectedAudio`), so it proves FGS, core, DataChannel and notification wiring, but not `AudioRecord` capture itself, which ADR-016 asks for. Closing this needs PCM injection below libwebrtc's audio device module.

### Dependency versions

`STATUS: DECIDED` (project owner, 2026-10-19): new native code starts on the **latest stable** toolchain and libraries (at PoC time: AGP 9.4, Gradle 9.8, Kotlin 2.4, NDK 30, Rust 1.99, UniFFI 0.32), never alpha/beta/RC. Rationale: long-term support and maintainability; avoids a forced AGP 8 → 9 migration right after the start. Keeping versions current afterwards needs automated update PRs (Renovate or Dependabot, `STATUS: IDEA`).

iOS is **not** part of this gate; a separate real-device gate for background audio follows before iOS work starts.

## Consequences

### Positive

- Background and alert reliability are owned by native code we control, on both platforms.
- Security/protocol logic exists once and is tested deeply (property and fuzz tests, conformance vectors).
- The LAN signaling server moves from Android-only Java into the shared core and becomes available on iOS.
- Device-level behaviour (Doze, notifications, permissions) becomes automatable with Maestro / UI Automator.
- The existing Svelte UI keeps its value as the web client.
- Translations are maintained once and reused by every UI; the core is locale-independent, which also keeps tests stable.
- Most failure scenarios are testable on the host in seconds and replayable from device traces (ADR-016).

### Negative

- Three UI codebases (Compose, SwiftUI, Svelte) to maintain, plus a Rust toolchain.
- More complex CI (NDK, XCFramework, WASM targets).
- Existing Capacitor-specific code (`lib/platform/*`, Java plugins) is retired after migration; the Java logic must be ported to Kotlin and Rust.
- A localization generator and the catalogue move (`apps/mobile/src/lib/i18n/messages/` → `locales/`) must be built and the existing `{{name}}` messages converted.
- AGENTS.md (incl. the i18n row of §9, which no longer matches the existing four locales), ADR-001, ADR-002, ADR-007 and ADR-008 need updates once this is DECIDED.

### Risks

- **Rust mobile build friction** — mitigated by the PoC gate (AC-6) and the fallback to Option D.
- **UI drift between platforms** — mitigated by keeping all behaviour in the core and treating UIs as thin renderers; conformance vectors in CI.
- **Maintenance load for a small team** — sequence the work: core + Android + web first, iOS only after its own gate.
- **libwebrtc prebuilt maintenance** — prefer actively maintained builds; pin versions and track security releases.
- **Emulator ≠ OEM reality** — the project is free and has no telemetry, so no specific hardware is mandated. Mitigation (ADR-016): OEM-chaos set on emulators, a device-agnostic physical runner for any available phone (including borrowed ones), disclosure of tested and untested device classes in release notes, user-submitted redacted reports, and an **optional** cloud device farm for smoke only.
