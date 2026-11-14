# Manual release testing checklist

> **Superseded by [ADR-020](../../openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md) §9 and §10:** the Capacitor baby setup and `mobile-v*` tags in this checklist, and the `pre-commit` CI job name, are legacy; CI is a single `ci-ok` aggregator and releases start from manual `android-v*` tags. Replaced in toolchain realignment waves 1 to 3; this procedure stays valid until then.

`STATUS: DECIDED`

Automated tests (Vitest, Playwright browser E2E) cover WebRTC wiring, pairing, noise alerts, and non-silent inbound audio in **Chromium** and **partially on Android**. They do **not** replace validation on real hardware before a user-facing release.

Use this checklist when merging a **Release Please** release PR or tagging a milestone (`mobile-v*`, `mvp-v*`, …). Record the outcome in the release PR description or maintainer notes (pass/fail, devices used, date).

**Related automation**

| Layer                     | Command                                        | Scope                                                                   |
| ------------------------- | ---------------------------------------------- | ----------------------------------------------------------------------- |
| Unit / integration        | `pnpm test`                                    | Rust core, relay, server, mobile                                        |
| Browser E2E (2× Chromium) | `just e2e`                                     | Pairing, WebRTC, Listen, noise alert, **RMS non-silence** (tests 14–15) |
| Signaling smoke           | `just test` (`guardedsleep-server` smoke test) | WebSocket relay through the built binary                                |

---

## 1. Preconditions

- [ ] A **strict emulator gate run** (`GS_STRICT=1`, with the Maestro UI assertions; the AC-12 pairing journey once enabled) is green on the tagged commit (`gate-ui-journeys`; automating this check belongs to `add-android-release`).
- [ ] CI green on the release commit (`pre-commit`, `test-and-build`, `e2e-browser`).
- [ ] Debug or release APK built from the same commit (`just build-android`).
- [ ] **Two physical Android devices** (or one phone + one tablet) on the same Wi‑Fi, or phone + laptop browser only for a reduced smoke (see §6).
- [ ] Signaling reachable from both devices (local `just dev`, Docker stack, or self-hosted WSS).
- [ ] Microphone and notification permissions granted on both apps.

**Minimum device matrix (PoC gate, ADR-001):**

| Role   | Device A                                              | Device B               |
| ------ | ----------------------------------------------------- | ---------------------- |
| Baby   | Modern Android (API 30+)                              | —                      |
| Parent | Different OEM if possible (e.g. Samsung/Xiaomi class) | Pixel or stock Android |

Repeat critical cases on **at least two OEMs** before calling MVP-ready.

---

## 2. Pairing and session (both devices native app)

- [ ] Fresh install (or clear app data) on both devices.
- [ ] Baby: create room → QR / invite shown.
- [ ] Parent: scan QR **or** paste invite link (paste is acceptable; note if QR failed).
- [ ] Both reach **Connected** within 60 s on home Wi‑Fi.
- [ ] `pair_proof` succeeds (no immediate `failed` after connect).
- [ ] Saved connection appears on home screen; reopening baby/parent shortcuts works.

---

## 3. Audio — Listen mode (push-to-listen)

- [ ] Parent: **Listen off** by default; no continuous streaming/heavy battery on baby.
- [ ] Parent: enable **Listen** → hears baby room audio within 5 s.
- [ ] Parent: disable **Listen** → audio stops; baby unit does not keep streaming unnecessarily.
- [ ] Audio is intelligible (not only “something plays” — human check).

---

## 4. Audio — Noise alert path

- [ ] Baby: threshold at default or low; make sustained noise at baby device (clap/speech).
- [ ] Parent: receives alert (vibration and/or notification if app backgrounded).
- [ ] Parent: **hears baby audio** after alert without manually enabling Listen first.
- [ ] Parent: **Snooze** suppresses repeat alerts for the configured window.
- [ ] Short noise blip (&lt; hold duration) does **not** alert (matches product defaults).

---

## 5. Background and baby-unit behaviour (native only)

- [ ] Baby: foreground service notification visible while monitoring.
- [ ] Baby: screen off for **≥ 10 min** — session stays connected or reconnects automatically (note behaviour).
- [ ] Baby: **≥ 30 min** screen-off run (PoC gate) — record battery % start/end and whether mic/WebRTC survived.
- [ ] Parent: app backgrounded → noise alert still surfaces (notification / full-screen intent per OEM).
- [ ] Parent: incoming **phone call** during alert audio — document whether baby audio resumes (ADR-011).

---

## 6. Connectivity edge cases

- [ ] **Wi‑Fi off/on** on baby — reconnect with saved credentials or clear failure message.
- [ ] **Wi‑Fi off/on** on parent — same.
- [ ] Same LAN, signaling URL unreachable (airplane mode on one device only if safe) — document LAN/NSD fallback if enabled on build.
- [ ] Wrong invite / corrupted PSK — pairing fails safely (no silent connect).

---

## 7. Regression spot-checks (quick)

- [ ] Settings: signaling URL persists.
- [ ] Baby: kiosk / immersive mode does not block required UX.
- [ ] Battery indicator on parent updates from heartbeat.
- [ ] Connection quality indicator moves under poor Wi‑Fi (optional stress).

---

## 8. What automation already covers (do not duplicate unless regressing)

Browser E2E (`apps/e2e`) already asserts:

- Signaling + WebRTC connect, DataChannel, paste-invite + `pair_proof`
- Listen on → inbound RTP; Listen off → baby track disabled
- Noise alert over DataChannel; silence / short-blip negative cases
- **Parent inbound stream RMS above silence floor** after Listen and after noise alert (tests 14–15)
- Reconnect, wrong PSK, unreachable signaling, simulated LAN path ranking

---

## 9. Release sign-off

| Field                          | Value                              |
| ------------------------------ | ---------------------------------- |
| Release / tag                  | e.g. `mobile-v0.2.0`               |
| Tester                         |                                    |
| Date                           |                                    |
| Devices (baby / parent)        | Model, Android version, OEM        |
| Signaling environment          | local / Docker / GHCR / custom WSS |
| Automated E2E                  | pass / fail (CI link)              |
| §3 Listen                      | pass / fail                        |
| §4 Noise alert + audible audio | pass / fail                        |
| §5 Background / 30 min baby    | pass / fail / deferred             |
| §6 Connectivity                | pass / fail / partial              |
| **Ship?**                      | yes / no — blockers listed below   |

**Blockers (examples):** no audio on alert, foreground service killed, reconnect loop, pairing fails on target OEM.

---

## 10. Discarded alternatives

- **Rely only on browser E2E for release** — rejected: Capacitor WebView, foreground service, OEM battery policies, and real microphones are out of scope for Chromium-only tests (ADR-007).
- **Mandatory two-emulator gate in CI** — rejected for now: RAM/flake cost; hybrid smoke is optional/local/nightly until stabilized.
