# ADR-018: Alpha milestone — scope, trust features, child-side silence

- Status: DECIDED (owner, 2026-10-20; revision 2 after role reviews)
- Date: 2026-10-19
- Deciders: Project owner (decided 2026-10-20, §6); reviewed by Product Owner (APPROVED WITH CONCERNS), UX/Usability (APPROVED WITH CONCERNS), Security/Privacy Architect (NEEDS REWORK → addressed), Technical Architect (APPROVED WITH CONCERNS), Critical Reviewer (NEEDS REWORK → addressed)
- Related: [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md), [ADR-004](../../openspec/changes/archive/2026-04-15-adr-004-encryption-transport-and-key-storage/design.md) (amendment), [ADR-009](ADR-009-personas-and-failure-model.md), [ADR-010](ADR-010-baby-unit-hardware-requirements.md), [ADR-011](ADR-011-audio-routing-parent-side.md), [ADR-013](ADR-013-connection-status-observability-and-copy.md), [ADR-015](ADR-015-native-clients-with-shared-rust-core.md), [ADR-016](ADR-016-test-strategy-local-virtual-physical.md), [ADR-017](../../openspec/changes/archive/2026-10-20-adr-017-link-recovery-and-connection-supervision/design.md); flows and copy: [alpha-ux-flows.md](../technical/alpha-ux-flows.md)

> **Context note (2026-10-20):** the "SvelteKit web client" is rebuilt as `apps/web` on Vite 8 + Svelte 5 ([ADR-020](ADR-020-toolchain-and-development-setup.md) §5); the alpha uses internet signaling for all pairs, offline LAN modes are beta exit criteria ([ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md) §1); distribution at alpha is Google Play (`org.guardedsleep.app.alpha`) plus GitHub Releases APK ([ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md) §3). Alpha scope changes only where inline marks say "superseded".

## Context

The owner asked (2026-10-19, paraphrased from German) to move from the PoC to a **first alpha** suitable for hardware tests — automated and manual, native app plus browser — with every feature needed for a usable app, everything virtually testable tested, a good user experience, and above all **trust**: parents are reliably informed, the parent side clearly signals any problem, and the app never wakes the child (no audio on the baby device except an explicit talk-back; the parent can check and, where possible, set remotely that the baby phone is silent; alarms may stay).

An alpha is a milestone towards the full MVP (ADR-001 unchanged), not an end-user or store release.

## Decision

### 1. What the alpha is

- **Clients:** native Android (Kotlin + Compose + Rust core) as baby and parent; the SvelteKit web client (superseded, see ADR-020 §5: Vite + Svelte 5) on the same core via WASM as baby or parent, at reduced parity (§2). ADR-015 stays PROPOSED until its gate passes; alpha builds are the vehicle.
- **Server:** signaling server with ADR-017 §10 changes. Tester setup: a LAN or self-hosted signaling server as primary, a public one as fallback (fallback URL list in Advanced settings, ADR-012); TURN configured.
- **Identity:** `applicationId org.guardedsleep.app.alpha`, namespace `org.guardedsleep.app`, version `0.1.0-alpha.N`, debug-signed CI artifacts (CI artifacts only; Play and GitHub releases are signed with the release key, ADR-020 §10). Release builds disable cleartext traffic (alpha only; from beta the LAN listener path follows ADR-021 §1, which partly supersedes this); test hooks (autostart/config extras, audio injection, `GS_POC` logs) exist only in debug and are checked absent by the release-APK inspection.
- **Languages:** en and de complete; fr and es may fall back to en per key without blocking hardware tests (superseded, see ADR-021 §2: a locale ships only with all `critical` keys; non-critical keys may fall back); the catalogue mechanism (AC-8) is required.

### 2. Scope

**T1 — required:**

| Area                   | Content                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Link recovery          | ADR-017 in core, Android and web; server liveness                                                                                                                                                                                                                                                                                                                                                                                                                |
| Connection visibility  | Parent ambient bar (colour + symbol + text) with levels 0–3; persistent foreground notification; "Connected again — not monitored for m:ss"; baby edge indicator and dim pulse; **explicit stop notices**: a stopped baby triggers one non-repeating parent alert ("Baby unit stopped monitoring"), a stopped parent shows a calm grey state on the baby — never an outage alarm                                                                                 |
| Connection alarm       | Level 2 full-screen alert + vibration (+ tone per §7 decision); level 3 continuous alarm; **"I'm checking"** silences sound/vibration for 5 min while the bar stays red, then escalates straight to level 3 if still down                                                                                                                                                                                                                                        |
| Parent alert path      | Alarm stream, **DND bypass** (notification-policy access on the parent), full-screen intent (`canUseFullScreenIntent` checked), distinct vibration patterns for noise vs. connection                                                                                                                                                                                                                                                                             |
| Parent readiness check | At start, on resume and every 5 min: notifications allowed, alarm volume > 0, lock-screen/full-screen alerts allowed, DND bypass, battery optimisation, parent battery (< 20 % not charging → "plug in your phone"); the worst failing item shown as a chip with a fix                                                                                                                                                                                           |
| Pairing                | Baby shows a QR (inverted, ≤ 50 % brightness) and copyable text; parent scans (CameraX + zxing, camera asked just in time) or pastes. Every QR carries a **fresh PSK** (room derived, ADR-017 §1), the signaling URL list and an expiry; valid 5 min with a visible countdown and "Show new code"; closes after the first verified parent. Keystore-encrypted storage; one-tap start; "Forget" also wipes the key and restores DND                               |
| Parent monitoring      | Headline (never "All quiet" while disconnected; after recovery only if the gap is covered), live meter, baby battery + charging ("Baby unit is no longer charging"), silence status chip (§3), Listen (push-to-listen, slide-to-lock, auto-stop after 60 min, screen wake lock), Test alert, Snooze 10/30 min (noise only, survives restarts, alerts immediately if noise persists at its end); "Stop monitoring" only in the menu behind confirmation           |
| Noise alert chain      | Vibration + high-priority notification + full-screen alert + child audio on the alarm stream + meter. Lifecycle per alpha-ux-flows §3: "Stop sound" ends the current event; new noise within 2 min continues it; later noise is a new full alert                                                                                                                                                                                                                 |
| Test alert (probe)     | A **digital** test event through the baby's pipeline (synthetic tone mixed into the outgoing stream, never played on the baby) plus the full parent chain. Microphone health: the baby reports `mic_silent` when it receives only digital silence (F-6)                                                                                                                                                                                                          |
| Other alerts           | Low baby battery (< 15 % not charging, < 5 % always); missed-events card ("not monitored for m:ss", count only when complete)                                                                                                                                                                                                                                                                                                                                    |
| Baby monitoring        | Dark kiosk (brightness at device minimum, touch-wake ≤ 10 %, transitions ≥ 1 s, hold-and-slide unlock), edge indicator, threshold/hold settings with live preview; no sound, vibration or haptics of any kind                                                                                                                                                                                                                                                    |
| Child-side silence     | §3                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Onboarding             | One checklist screen per role (permissions with plain rationale, battery-optimisation exemption, DND access, battery safety per ADR-010, honest limits: parent phone and server are single points of failure, alarms on the baby phone can ring); indicator training as coach marks on the real screen after the first connection, ending with the test alert                                                                                                    |
| Baby self-test         | Microphone level, battery, signaling reachability, permissions, DND access → recommended / limited                                                                                                                                                                                                                                                                                                                                                               |
| Settings               | Basic: threshold, hold 3/5/10/15 s (default 5), alert output Default / Speaker only, alert gain, theme (dark default), language, "silence this phone while monitoring" (baby). Advanced: signaling URL list, TURN, heartbeat 5–15 s, diagnostics                                                                                                                                                                                                                 |
| Web client             | Full client in both roles on the WASM core (laptop baby unit supported): "keep this unit awake" setup and warnings (plugged in, lid open, tab visible, wake lock, suspend detection); link status, alerts while the tab is open, "Tap to turn on alert sound", "keep this page open and the screen on" banner + wake lock, "page was in the background — alerts may have been missed" warning; a web baby reports silence status "unknown". No onboarding polish |
| Tests                  | §4                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

**T2 — if time allows:** resume after phone calls (ADR-011), headphone warning, network-mode banner, 30-minute heartbeat self-check after onboarding (mandatory old-phone soak replaces it for the alpha), emoji fingerprint, roaming warning, fr/es completeness, remote sensitivity change.

**Not in the alpha:**

- Offline LAN signaling without any server (embedded listener + NSD) — `DEFERRED` to alpha 2 (superseded, see ADR-021 §1: target beta); UC-4 (hotspot without internet) is unsupported in the alpha and the docs say so.
- Baby auto-start after reboot — `DEFERRED` (Android 14+ microphone foreground-service restriction, ADR-017 §9); the parent escalates. Recorded in device-management.md and failure-model.md; "baby phone reboots overnight" is a must-pass hardware scenario (parent reaches level 2).
- Dual audio routing (ADR-011 "Both"), talk-back, multi-parent, app lock, video.

### 3. Child-side silence

**Guaranteed (`STATUS: DECIDED`, owner request):** GuardedSleep itself never produces sound, vibration or haptics on the baby phone — no WebRTC playout (the core refuses a baby SDP that is not `sendonly`), silent minimum-importance notification channels, no alert sounds, no TTS. Enforced by tests (core SDP check, channel settings, no audio-output code path in the baby role, web baby included).

**Best effort, reported honestly (`STATUS: UNDER DISCUSSION` until hardware-tested):**

1. With a one-time grant of Do-Not-Disturb access on the baby phone, the app activates an `AutomaticZenRule` with a `ZenPolicy` that allows **alarms only** and **suppresses visual effects** (screen on, peeking, notification light, ambient display) while monitoring. Deactivating the rule restores the user's own mode; a rule left active after a crash is deactivated at the next app start and the user is told. Default on when access is granted.
2. The baby reports a coarse `device_status` (ADR-017): ringer (normal / vibrate / silent), DND state, alarms allowed, minutes to the next alarm clock, DND access, `mic_silent`, battery bucket, charging. **Vibrate counts as not silent.**
3. The parent sees one value: silent / screen may light / may buzz / may ring / unknown / no access, with the fix. Even when green, the parent sees "Alarms and emergency alerts on the baby phone can still sound" and the next alarm time if one is set.
4. **Remote control:** the parent may only request silencing **on** (when the baby lost or never applied it); there is no remote "off". Accepted only on a verified attempt, rate-limited, and shown on the baby screen.
5. The baby screen tells a person using their own phone as baby phone that calls and messages stay silent until monitoring stops.
6. Browsers cannot read or set any of this: a web baby reports "unknown" (grey, not amber) and the parent shows the manual instruction.

### 4. Test bar

- **Core:** unit, property, conformance vectors (incl. reflection attack), host simulation of AC-9 and every ADR-017 §12 scenario, trace replay through Rust, Kotlin (JVM) and WASM.
- **Android without devices:** Robolectric tests for platform glue; Roborazzi screenshots of every screen and state in en and de (fr/es/pseudo where complete) — basis for the UX review; release-APK inspection; size check.
- **Web:** Vitest; Playwright two-role tests on the WASM core including recovery with a hidden page.
- **Emulator gate (mandatory):** `scripts/poc/local-gate.sh` runs AC-1…AC-4 and AC-10 locally on KVM (available since 2026-10-19) and nightly in CI.
- **Hardware:** `docs/contributing/alpha-hardware-test.md` with rounds: native↔native at home (pairing, one-tap restart, noise alert with the parent locked/silent/DND, probe, listen, Wi-Fi off < 30 s and > 60 s on each side, kill/force-stop each side, baby reboot, router and server restart); child silence (call, SMS, messenger, charger plug, system low-battery warning, alarm clock, restore on stop); overnight 8 h soak on both phones and on one old phone (Persona C); browser roles (laptop baby ↔ native parent, Android Chrome parent ↔ native baby, mobile data with the public server).

### 5. Exit criteria ("ready for hardware tests")

Measured in the emulator gate and host simulation:

- noise → parent alert ≤ 2 s; outage ≤ 30 s → no level 2; outage ≥ 60 s → level 2 within 62 s of last contact and `link_restored` afterwards; one-tap start of a saved pairing → connected ≤ 15 s; wrong key rejected; reflection attack rejected;
- automated baby-silence checks green (Android and web baby);
- all T1 rows implemented; role reviews (UX with screenshots, Security, Critical Reviewer) without open CRITICAL findings;
- hardware checklist written. A real-device smoke test (pairing, overnight foreground service, Wi-Fi flap, link-lost alarm) is the first hardware round (§6).

### 6. Owner decisions (2026-10-20)

- [x] **Connection-alarm sound:** level 2 (60 s) repeats a **soft tone every 10 s** (like classic hardware baby monitors) with vibration; continuous alarm from level 3 (180 s).
- [x] **Level 1 (20 s):** visual only, no vibration.
- [x] **QR validity:** 5 min, single use, fresh key per code; supersedes the ~30 s in connectivity-pairing.md.
- [x] **Terms in copy:** device-neutral **"baby unit" / "parent unit"** (de "Babyeinheit" / "Elterneinheit"); never "phone", because tablets and laptops can be units. The UX recommendation "baby phone" is rejected.
- [x] **Order of work:** build T1 locally first; hardware tests afterwards. The emulator gate passed AC-1…AC-4 with 30 min forced Doze on 2026-10-20.
- [x] **Web client:** permanent full client in both roles (see [ADR-019](ADR-019-language-repository-and-client-policy.md)); a laptop or desktop browser with its internal microphone is a supported baby unit, with an explicit "keep this unit awake" setup (plugged in, lid open, no OS sleep or night-time restarts, tab visible, Screen Wake Lock).

## Options Considered

### Option A: Alpha on the Capacitor web app (ADR-002)

- **Pros**: more UI exists.
- **Cons**: inherits the WebView background and iOS problems; trust features are native anyway. Rejected.

### Option B: Native Android + web client on the shared core (CHOSEN)

- **Pros**: tests the intended architecture; one implementation of the safety logic.
- **Cons**: more work before the first hardware test; web client migration.

### Option C: Native Android only

- **Cons**: the owner wants browser + app tests; protocol v3 would leave the web client incompatible. Rejected.

## Consequences

### Positive

- Every failure becomes visible on the parent side; the app's own silence on the baby side is enforced by tests; what the platform cannot guarantee is stated honestly.
- The PoC gate and AC-10 run locally on emulators, then on hardware.

### Negative

- Large scope; offline LAN mode slips to alpha 2.
- Sensitive permissions on both phones (DND access on the baby, notification-policy access on the parent).

### Risks

- OEM differences in DND rules, full-screen intents and process killing — found in hardware rounds, documented per device report.
- A crash leaving DND active on the baby phone — fails towards silence (alarms still allowed), restored and announced at the next start.
