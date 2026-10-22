# ADR-016: Test strategy — local first, then virtual, then physical devices

- Status: DECIDED (owner, 2026-10-21, together with ADR-015)
- Date: 2026-10-19
- Deciders: Project owner, Technical Architect, Security/Privacy Architect, UX, Critical Reviewer (reviewed by four independent review agents, see "Critical review record")
- Depends on: [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md) (shared Rust core, native clients). ADR-015 delegates test details to this ADR, so **both must be decided together**.
- Amends / narrows (only once ADR-015 and this ADR are DECIDED, via a status note on each, never by silent edit): [ADR-001](../2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md) (WebView/Capacitor PoC gate and the "two OEMs" rule), [ADR-007](../2026-05-13-adr-007-browser-e2e-test-strategy/design.md) (scope narrowed to the web client). Replaces the test sections of ADR-015 by a pointer.

> **Partially superseded (2026-10-20):** test tooling details → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §3–§5: nextest, proptest, insta, negative vectors through every binding, two nightly fuzz targets, Roborazzi + accessibility check on 11 goldens (replacing the broader screenshot matrix), Playwright with `@axe-core/playwright`, `just` as the only entry point and a mandatory `ci-ok` gate. The layering of local → virtual → physical stands. Decided by the owner together with ADR-015 on 2026-10-21.

## Context

The project is a free, privacy-first, open-source app maintained by one to two people. It has no analytics, no crash reporting and (by guardrail) no telemetry, so field feedback arrives only when users choose to send it. Testing has to find as much as possible **before** release, cheaply, and must not depend on paid infrastructure or a large device fleet. At the same time the product promise — _the alert must arrive_ — is exactly what emulators and unit tests cannot prove, so a small, non-negotiable physical gate is needed (Decision §5).

Existing test decisions and assets (reviewed 2026-10-19):

| Source                                                                          | What it decided / provides                                                                                            | Status under ADR-015                                                                                |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| ADR-001                                                                         | PoC gate on real devices: 30 min screen-off, two OEMs, older Android (Persona C), call interruption                   | DECIDED and binding until ADR-015/016 are DECIDED. Goals stay valid; wording is WebView-specific    |
| ADR-007                                                                         | Playwright, Chromium only, dev-only `window.__guardedsleep_test` hook                                                 | Stays for the web client; role changes (Decision §2)                                                |
| ADR-009 / 010 / 011 / 013                                                       | Persona acceptance tests, min-API + self-test, call/audio-focus behaviour, state-transition rendering                 | Requirements stay; no single place defines how they are tested                                      |
| ADR-012                                                                         | Path-selector tests with two WS servers; real NSD / offline LAN deferred to "Android-instrumented tests"              | The deferred part has no plan (emulators sit behind separate NATs, no mDNS between them)            |
| `docs/contributing/manual-release-testing.md`                                   | Manual checklist, two-device matrix, "≥ two OEMs"; **rejects a mandatory two-emulator CI gate** (RAM/flake cost)      | Needs a rewrite (Decision §6); the rejection is addressed in Decision §3                            |
| `apps/e2e` (17 specs + 4 hybrid), `docker/e2e-local`, `docker/android-emulator` | Two-Chromium WebRTC E2E; emulator-baby + browser-parent via `adb reverse` and WebView DevTools                        | Hybrid specs are Capacitor-specific and are retired; the "emulator + browser peer" pattern survives |
| ADR-015 (draft) and `.github/workflows/android-native.yml`                      | Six test layers; PoC gate on two API 34+ emulators with Maestro and a 30-minute forced Doze, "manual or nightly only" | Good base; gaps below                                                                               |

### Gaps found when questioning those decisions

Each gap was re-checked against the sources during review; the evidence level is noted.

1. **The sans-IO core is under-used for testing.** ADR-015 plans unit, property and fuzz tests but not whole-system scenarios (two cores, simulated signaling and network) reproducible by seed. _Verified:_ `Session::handle(now, input)` is deterministic and takes time as an argument.
2. **A 30-minute forced Doze is likely too short and too mild.** `deviceidle force-idle` jumps straight to idle, whereas the maintenance-window cycle that hurts monitoring apps takes longer. _Hypothesis, UNVERIFIED against AOSP docs; to be confirmed in the first Stage-3 soak._
3. **Emulators can simulate part of OEM aggressiveness** (standby bucket `restricted`, `RUN_ANY_IN_BACKGROUND` denied, force-stop, trim-memory). ADR-015 does not use this.
4. **API 34+ as the only emulator level is too narrow.** Behaviour changes at API 26, 29/30, 31, 33 and 34. The support floor itself is still open (ADR-010: API 26 candidate; `variables.gradle` currently has `minSdkVersion = 23`).
5. **Two emulators cannot reach each other directly** (separate 10.0.2.x NATs), so emulator↔emulator pairing needs a TURN relay. Emulator↔host-browser reachability is UNVERIFIED.
6. **LAN/NSD (ADR-012) has no test plan.** mDNS between NAT-ed emulators does not work; this needs real Wi-Fi.
7. **Audio injection tests wiring, not detection.** Detection lives in the core and is testable on the host.
8. **Text-based UI selectors are fragile** with copy from i18n in three UIs.
9. **The existing tests are the only executable description of today's behaviour** and should be exported as vectors before the port. _Verified:_ the vectors directory contains only `conformance.json`.
10. **Policy, not a technical gap:** ADR-015 demanded "a Samsung and a Xiaomi device". A free project cannot make releases depend on specific hardware.
11. **The core cannot express link recovery.** _Verified (`session.rs`):_ `PeerConnectionState` `failed`/`closed` calls `fail()` and sets `finished`, after which all inputs are ignored; there is no reconnect or ICE restart. Wi-Fi-flap and process-restart scenarios (failure-model F-1…F-5, old Playwright spec 09) therefore cannot be modelled against `Session` today. See Decision §2 and ADR-015 "Link recovery".

## Options Considered

### Option A: Keep the ADR-015 test section as is

- **Pros**: already written; smallest effort; unit/property/fuzz/conformance already planned.
- **Cons**: gaps 2–6 and 11 remain; the hardware requirement (10) is unrealistic.

### Option B: Full programme (simulation, device trace recorder, community reports, full API matrix, iOS rules)

- **Pros**: strongest debugging and coverage story.
- **Cons**: large up-front work for one to two maintainers; a trace recorder in the safety-critical core is itself security-sensitive.

### Option B-lite: Lean core first, grow on evidence (chosen)

Deterministic simulation as a thin vertical slice, a small nightly emulator tier, a device-agnostic runner, a mandatory physical soak gate, a release checklist. Everything else is explicitly DEFERRED with a reason and target phase (Decision §9).

- **Pros**: delivers the safety-critical evidence early; cheap; keeps options open.
- **Cons**: weaker field debugging until the deferred parts are built.

### Option C: Mandatory cloud device farm

- **Pros**: Samsung/Xiaomi coverage without owning devices.
- **Cons**: recurring cost for a free app; third-party dependency; farm devices are plugged in on datacenter Wi-Fi so they cannot show battery-manager behaviour; two-device scenarios need public signaling/TURN. Kept as an **optional** tool only (Decision §7).

### Option D: Manual testing on physical devices only

- **Pros**: no automation to maintain.
- **Cons**: no regression signal; contradicts the intent of ADR-007.

## Decision

**Option B-lite**, with a mandatory physical soak gate. A cloud device farm is explicitly optional. The project owner confirmed on 2026-10-19: lean scope (A1), mandatory soak gate (B1), link recovery to be designed first with a ticket (C1) and then gated in the PoC as AC-10, relaxation of the "two OEMs" rule (D); and English, German, French and Spanish as officially supported languages (see ADR-015 "Localization").

### 1. Three stages, from cheap to expensive

| Stage            | Where                      | Cadence                                               | Purpose                                                                                                                 |
| ---------------- | -------------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| **1 – Local**    | Host / CI container, no OS | every PR (target ≤ 10 min, tracked, not enforced yet) | Logic, protocol, security, UI rendering, web client, localization                                                       |
| **2 – Virtual**  | Emulators                  | smoke on Android-touching PRs (optional); nightly     | Framework behaviour per API level, permissions, foreground service, notifications, process death, OEM-chaos set         |
| **3 – Physical** | Any attached device        | before milestone releases; ad hoc                     | Real microphone/AEC, audio routing, battery-manager behaviour, thermals, real Wi-Fi + NSD, **real-time overnight soak** |

Rule: a bug found in a later stage is first reproduced in the earliest stage that can express it and fixed with a regression test there.

### 2. Stage 1 — local

- **Core unit, property (`proptest`) and fuzz (`cargo-fuzz`) tests** with simulated time (ADR-015).
- **Time model:** the core receives a **monotonic** millisecond clock (the current doc comment in `session.rs` says "wall-clock" and must be corrected). Doze and suspend produce time jumps; the simulation includes a time-jump operation.
- **Deterministic system simulation, vertical slice first (AC-9):** two core instances plus a fake platform (SDP with a synthetic `a=fingerprint`, DataChannel-open timing, a model of the signaling server's `welcome`/`peer`/`members`) and a seeded fault injector. Scope for now: **the current state machine only** — signaling loss before verification, heartbeat loss, wrong PSK, delayed/duplicated/reordered messages, time jumps. The harness's signaling model is a third implementation next to the Node and Java servers; drift is mitigated by driving it from the same conformance vectors.
- **Link-recovery scenarios** (Wi-Fi flap on either role, AP / band / mobile-data switch, router and signaling-server restart, process restart, short vs long outage, Doze time jump) are **PoC-gating (ADR-015 AC-10)**. Because the core cannot recover today (gap 11), they are first written **test-first as an expected-failing specification** against the current state machine, then turned green by the link-recovery design (ADR-015 "Link recovery", ticket open). They run in all three stages: host simulation, emulator (`svc wifi disable/enable`, airplane mode, `force-stop`) and at least one real-device run (router off/on, AP switch, Wi-Fi ↔ mobile data).
- **Traces:** `golden` and `simulated` traces are committed JSON; see [test-schemas.md](../../../../docs/technical/test-schemas.md). The existing `session` conformance cases stay valid; a trace is a superset with a converter.
- **Characterization before porting:** export vectors from the current TypeScript protocol/pairing code and the Java LAN signaling server before they are replaced, so the Rust core is held to today's behaviour. Vitest suites that duplicate core logic are retired only once equivalent traces are green. Replaying the same vectors through UniFFI and WASM mainly tests marshalling (same Rust code); the independent implementation is the TypeScript code, so characterization vectors are the real cross-check.
- **Detection logic:** noise/cry thresholds are tested in the core with WAV-derived vectors, not on devices.
- **UI (JVM):** Compose tests with Robolectric and Roborazzi screenshots (including pseudo-locale, large font scale and narrow width; golden set narrowed to 11 baselines, see ADR-020 §4; golden updates only via an explicit command, with tolerance); Svelte component tests as today (superseded, see ADR-020 §5: Vitest for logic only, Playwright for UI behaviour).
- **Web client in Playwright (ADR-007, narrowed):** the only place with real WebRTC and real media on the host. Role: web-client regression and **interop peer / media harness** for native clients (headless Chromium with fake audio as parent for a native baby, automating AC-7). Existing specs are reallocated as follows.

| Existing specs                                                | Future home                                                                                                              |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| 11 wrong PSK, 12 signaling unreachable                        | core simulation (one Playwright smoke each may remain)                                                                   |
| 09 reconnect, 13 simulated LAN fallback                       | stay in Playwright until the link-recovery design and a Rust path selector exist (13 tests the TypeScript path selector) |
| 06 silence, 07 short blip                                     | core vectors (WAV)                                                                                                       |
| 05 noise alert, 10 parent playback                            | logic in core; media wiring stays in Playwright                                                                          |
| 03, 04, 08, 14, 15 (audio flow, latency log, listen off, RMS) | stay in Playwright (media harness)                                                                                       |
| 16 LAN-IP connect                                             | stays (environment smoke)                                                                                                |
| `tests/hybrid/*` (Capacitor emulator)                         | retired with the Capacitor shell; replaced by native emulator + web parent                                               |

### 3. Stage 2 — virtual (lean)

- **Matrix (initial, nightly, non-blocking):** API 26 and API 34 on the existing Docker emulator setup. Widen to API 30 (microphone while-in-use rules), then 31/33 and the latest stable level, as CI budget and the ADR-010 floor decision allow. API 31 (foreground-service background-start limits) and 33 (`POST_NOTIFICATIONS`) are only partly covered by the 34 row; this is accepted until widened. Gradle Managed Devices are _not_ evaluated now: they need KVM as well, ATD images do not cover old API levels (UNVERIFIED), and Docker already exists.
- **Relation to the earlier rejection of a mandatory two-emulator gate** (`manual-release-testing.md`): the reasons (RAM, flake) still hold, so the emulator tier is **nightly and non-blocking for PRs**. The release gate is "Stage 1 green on the release commit **plus the last green nightly**". A flaky nightly is triaged, not ignored; if the nightly cannot run on hosted runners, a self-hosted runner or a weekly cadence is the fallback.
- **Network:** the test TURN relay (coturn) is decided **per pairing type**: required for emulator↔emulator; for emulator↔host-browser it is used only if direct reachability is not demonstrated. Test TURN credentials are generated at stack start, never committed, and coturn binds to the Docker network only.
- **OEM-chaos set** (scripted `adb`, any debug build): standby bucket `restricted`, `RUN_ANY_IN_BACKGROUND` denied, `am force-stop` / `kill -9` (`am kill` only affects cached background processes and is _not_ a valid process-death test for a foreground-service process), trim-memory, Wi-Fi/airplane toggles. `adb emu network delay/speed` shapes latency and bandwidth only; packet loss and reordering come from host `tc` or a proxy. Call simulation via the emulator console (`gsm call`) is used if the image keeps telephony (UNVERIFIED).
- **Doze:** ADR-015 AC-2 (30 min forced idle) stays the PoC smoke. A nightly job may additionally walk the Doze state machine with `cmd deviceidle step`; this compresses time, so it exercises framework state transitions and **not** real-time heartbeat gaps. Deep idle may need enabling on emulator images (UNVERIFIED). **Real-time soak belongs to Stage 3.**
- **Driver:** Maestro for flows, `adb`/`dumpsys` for OS-state assertions. Maestro selects elements by **stable IDs** (Compose `testTag` with `testTagsAsResourceId` enabled explicitly), never by copy.
- **Test-only surfaces** (audio injection, trace tools) exist only in debug builds, which use a distinct `applicationIdSuffix`, set the injection receiver `exported="false"` (the adb shell can still reach it) and are never attached to releases. **CI release check (owner: whoever maintains `android-native.yml`; acceptance criterion in the PoC):** the release APK must be non-debuggable, contain no exported receiver for the test action, and the DEX and resources must not contain the intent-action string or test symbols (a symbol-only check can miss R8-renamed code). An equivalent iOS check is added with iOS.
- **iOS simulator** rules are DEFERRED until iOS work starts (Decision §9).

### 4. Stage 3 — physical, device-agnostic ("bring your own phone")

Physical tests are written so that **any** Android device can be attached; no scenario names a model.

- **One runner** (`scripts/device-test.sh`, provisional name) takes `ANDROID_SERIAL` (or iterates attached devices). It collects a **device profile** (API level, manufacturer/model, ROM family stock / oem-skin / aosp-like / custom, root yes/no, battery state). Scenarios carry capability tags (`needs-second-device`, `needs-real-mic`, `needs-sim`, `needs-wifi-lan`, `soak`); the runner executes the applicable subset and reports the rest as _skipped (reason)_ — never as pass, never as failure.
- **Report:** a local JSON + Markdown file per run (schema in [test-schemas.md](../../../../docs/technical/test-schemas.md)). Nothing is uploaded by the tool; identifiers are removed before anything is written (AGENTS.md §7.6). `dumpsys` content enters a report only through a per-service allowlist plus a scrub pass.
- **Maintainer devices** are suggestions, not requirements: an older phone for endurance and a low-end baseline, a custom-ROM rooted phone for OS-level inspection, a daily-driver phone as parent (secondary user or debug-suffix build). Rooted/custom-ROM devices are labelled as such because they under-represent OEM battery managers. Borrowed devices (e.g. an older Samsung from family) are used opportunistically and add a row to the tested-device list; their absence blocks nothing beyond Decision §5.
- **NSD / offline LAN (ADR-012)** is tested here on real Wi-Fi with two devices; the logic is covered in Stage 1.
- Community-submitted reports, an in-app self-test emitting the same schema, and a known-device list are DEFERRED (Decision §9).

### 5. Mandatory physical soak gate (owner decision B1)

- Before every **milestone release** (`mvp-v*`, `mobile-v*` marked stable), at least one physical Android device completes an **overnight soak: ≥ 8 h, screen off, unplugged where safe, baby role streaming, parent receiving**, with the heartbeat-gap statistics and foreground-service sampling from the device report. Battery condition is checked first (ADR-010 safety copy applies to testers too).
- **Owner:** the release manager named in the release PR. If no soak was run, the release carries the label **"not soak-tested"** in its notes and is **not tagged stable**.
- **Exit condition on failure:** the cause is fixed, or the affected device class is documented as unsupported or limited in the user-facing compatibility notes. Disclosure alone does not clear a failed soak.
- Oracle for long runs: heartbeat gaps measured on the parent side (host-visible from the `Heartbeat` events), not the in-app "Lakmus test" of failure-model F-5, which is only a `STATUS: IDEA` (device-management.md).

### 6. Release policy

- The manual checklist (`manual-release-testing.md`) is rewritten around the three stages and capability tags; items covered by Stage 1/2 are removed or reduced to spot checks.
- Gates: Stage 1 green on the release commit; last nightly green (Stage 2); soak per §5.
- **"At least two OEMs" is relaxed** (owner decision D; applied to ADR-001 and the checklist by a status note once this ADR is DECIDED): test as many distinct OEM families as available; list tested devices, Android versions and ROM families in the release notes and name known-untested classes (e.g. aggressive OEM battery managers). Unverified OEM behaviour is handled by the battery-optimization setup flow and user reports.
- Any further relaxation of ADR-001 needs the owner.

### 7. Cloud device farms — optional, never a gate

A maintainer **may** use a farm (e.g. Firebase Test Lab, AWS Device Farm, BrowserStack) for occasional Stage-3 smoke on models otherwise unavailable. Constraints: never required by CI or a release; no farm secrets in the repository; synthetic audio only; debug builds with throw-away endpoints (a per-run signaling instance and a throw-away PSK), because the APK is uploaded to a third party; suitable for smoke (install, permissions, pairing against a peer we control), **not** for soak or battery behaviour. This is outside the media and alert path of real users and therefore does not touch AGENTS.md §7.1. Pricing was looked up on 2026-10-19 (pay-per-minute offerings in the range of single-digit USD per month at smoke volumes) and is **UNVERIFIED and not part of this decision**.

### 8. Cadence summary

| Trigger             | Runs                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| Every PR            | Stage 1 complete; Stage 2 smoke only when Android code is touched and a runner is available                  |
| Nightly             | Stage 2 lean matrix (API 26 + 34), OEM-chaos set, PoC gate job; non-blocking                                 |
| Milestone / release | Stage 1 + last green nightly + **soak per §5** + device runner on all available devices + manual spot checks |
| Ad hoc              | Device runner on any phone                                                                                   |

### 9. Deferred (explicit)

| Item                                                                                  | Reason                                                                                          | Target phase                   |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------ |
| Device-side trace recorder and shareable-trace replay                                 | Security-sensitive code path; no field bug yet that needs it                                    | Phase 2, or on first field bug |
| `gs-scenario` with many fault verbs                                                   | Start with the AC-9 slice and the AC-10 scenarios; add further verbs when a scenario needs them | with link-recovery design      |
| Community device reports, in-app self-test using the report schema, known-device list | Needs privacy sign-off (model name, timestamps, share flow) and owner                           | Phase 2                        |
| Gradle Managed Devices evaluation, emulator matrix beyond API 26 + 34                 | CI budget; floor decision (ADR-010)                                                             | after PoC                      |
| iOS simulator rules and iOS release-artifact inspection                               | iOS starts after its own gate (ADR-015)                                                         | Phase 2                        |
| Markdown rendering of reports                                                         | Convenience only                                                                                | Phase 2                        |

## Consequences

### Positive

- Safety-critical behaviour is verified early and cheaply on the host; the physical soak keeps the core promise honest.
- Emulators are used for what they are good at, not as proof of OEM behaviour.
- Any phone can run the physical tier; no dependency on paid services or specific hardware.
- Deferred parts are visible with reasons instead of silently dropped.

### Negative

- Up-front work: simulation slice, fake platform, runner and report schema, TURN in the test stack.
- Existing Capacitor hybrid tests and WebView-specific scripts are retired.
- Field debugging is weaker until the deferred trace recorder exists.
- Documentation to rewrite: `manual-release-testing.md`, `apps/e2e/README.md`, status notes on ADR-001/007, AGENTS.md §4/§9.

### Risks

- **Simulation drifts from reality** — mitigated by driving the fake signaling server from conformance vectors and by Stage 2/3 mirroring simulated scenarios.
- **Emulator ≠ OEM** — mitigated by the chaos set, the soak gate, disclosure and (optional) farm/borrowed devices; not eliminated.
- **No soak device available at release time** — the label rule in §5 makes this visible instead of silent.
- **CI budget** — non-blocking nightly; self-hosted runner or weekly cadence as fallback.

## Critical review record

Four review agents (Technical Architect, Security/Privacy, UX/Localization, Critical Reviewer) reviewed the first draft on 2026-10-19; all verdicts were _accept with changes_. Responses:

| Finding (severity)                                                                                                             | Response                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| CRITICAL: core promise unmitigated without a minimum physical gate                                                             | **Accepted** — Decision §5 (mandatory soak, label, exit condition)                                                                      |
| CRITICAL: "Supersedes" on DECIDED ADR-001; relaxation of the two-OEM rule; missing review record                               | **Accepted** — "amends/narrows" via status note; relaxation recorded as owner decision D; this section                                  |
| CRITICAL: contradiction with the earlier rejection of a mandatory emulator gate                                                | **Accepted** — nightly, non-blocking; release gate wording in §3                                                                        |
| CRITICAL: trace is not "the same structure" as the `session` vector                                                            | **Accepted** — superset with converter ([test-schemas.md](../../../../docs/technical/test-schemas.md) §1)                               |
| CRITICAL: shareable-trace replay under-specified; `oracle`/`test-oracle` core path is an authentication-bypass risk            | **Accepted** — oracle removed; recorder deferred; replay uses a synthetic PSK recomputed by the driver                                  |
| CRITICAL: simulation assumes reconnect behaviour the core lacks                                                                | **Accepted** — verified in `session.rs`; link-recovery design first (owner decision C1); simulation scoped to the current state machine |
| IMPORTANT: redaction incomplete / fails open                                                                                   | **Accepted** — allowlist model plus lint in test-schemas.md                                                                             |
| IMPORTANT: release-APK check insufficient; test surfaces                                                                       | **Accepted** — §3 (debuggable, exported, DEX/resource grep, suffix, owner)                                                              |
| IMPORTANT: Android command inaccuracies (`am kill`, `emu network`, Doze step), matrix (30 vs 29), TURN per type, GMD vs Docker | **Accepted** — §3                                                                                                                       |
| IMPORTANT: time model wall-clock vs monotonic                                                                                  | **Accepted** — §2                                                                                                                       |
| IMPORTANT: Lakmus is only an IDEA; oracle                                                                                      | **Accepted** — §5 uses host-visible heartbeat gaps                                                                                      |
| IMPORTANT: over-engineering for one to two maintainers                                                                         | **Accepted** — Option B-lite and §9                                                                                                     |
| IMPORTANT: device-report privacy (model, timestamp, `dumpsys`), upload promise                                                 | **Accepted** — allowlist/scrub, date-rounded timestamps, no upload; community/in-app flow deferred                                      |
| IMPORTANT: localization (plurals, headless notifications, manifest text, lint, RTL, fallback)                                  | **Accepted** — ADR-015 "Localization"                                                                                                   |
| SUGGESTION: Options A/C partly strawmanned                                                                                     | **Partly accepted** — Option B-lite added, A's merits stated; C kept as the explicitly optional farm                                    |
| SUGGESTION: testable numbers for CI duration and Doze soak                                                                     | **Accepted for the soak** (≥ 8 h); PR duration target is tracked, not enforced, until measured                                          |
| SUGGESTION: pricing unsourced                                                                                                  | **Accepted** — marked UNVERIFIED, not part of the decision                                                                              |

No unresolved disagreement remains between reviewers and author. Owner decisions are recorded in "Decision".

## Exit conditions for DECIDED

1. ADR-015's AC-8, AC-9 and AC-10 pass (AC-10 in simulation and on emulators in CI, plus one real-device run).
2. One Stage-3 soak per §5 has been run and its report reviewed.
3. The link-recovery design (ADR-015) is decided (owner and date set).
4. The owner accepts ADR-015 and ADR-016 together.

## Discarded alternatives

- **Keep ADR-015's test section unchanged (Option A)** — rejected: leaves gaps 2–6 and 11.
- **Full programme up front (Option B)** — rejected for now: disproportionate; kept as deferred items.
- **Mandatory cloud device farm (Option C)** — rejected: cost, third-party dependency, poor fit for soak.
- **Manual-only physical testing (Option D)** — rejected: no regression signal.
- **Appium with WEBVIEW ↔ NATIVE_APP context switching** — rejected with the WebView architecture (ADR-015 context).
- **Text-based UI selectors** — rejected: break with every copy or locale change.
- **A core-side "recorded oracle" for `pair_proof` replay** — rejected: an authentication-bypass path in the core; replay uses a synthetic PSK instead.
