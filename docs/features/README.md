# Feature Overview

This document is the central feature map of GuardedSleep. It lists all features with their current phase, status, and priority. Each feature links to a detailed specification document.

> Updated after full expert review round (Technical Architect, UX Expert, Product Owner, Security Architect, Critical Reviewer). See [technical analysis](../technical/analysis.md), [SW architecture](../technical/sw-architecture.md), and [UI design](../technical/ui-design.md). **2026-05:** Personas, failure model, and use cases: [personas.md](../concept/personas.md), [failure-model.md](../concept/failure-model.md), [use-cases.md](../concept/use-cases.md); review: [concept-review-2026-05.md](../reviews/concept-review-2026-05.md); ADRs [ADR-009](../decisions/ADR-009-personas-and-failure-model.md), [ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md), [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md) (PROPOSED).

## Feature Map

| Feature Area             | Detail Document                                    | MVP Scope                                                                  | Phase 2 Scope                                                                                 | Status  | Priority | Primary personas                                                            |
| ------------------------ | -------------------------------------------------- | -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ------- | -------- | --------------------------------------------------------------------------- |
| Audio Monitoring         | [audio-monitoring.md](audio-monitoring.md)         | Noise detection + listen mode                                              | Sound categorization, on-device AI                                                            | DECIDED | High     | B (daily), A (events)                                                       |
| Communication            | [communication.md](communication.md)               | DataChannel + audio stream                                                 | Talk-back (high expectation vs hardware monitors)                                             | DECIDED | High     | All                                                                         |
| Notifications and Alerts | [notifications-alerts.md](notifications-alerts.md) | Alerts + vibration + snooze + connection loss                              | Configurable alerts, DND override                                                             | DECIDED | High     | All (routing: [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)) |
| Connectivity and Pairing | [connectivity-pairing.md](connectivity-pairing.md) | QR pairing (multi-IP), local + signaling + VPN; **MVP: 1 baby + 1 parent** | Multi-parent (max **4** parents), same add-device flow; **failover**; handoff, remote pairing | DECIDED | High     | A (out-of-home), B                                                          |
| Device Management        | [device-management.md](device-management.md)       | **MVP: 1+1 devices**, Android, kiosk mode, battery deep-link               | iOS; **Phase 2:** up to **4** parent devices per baby, add device via QR from baby or parent  | DECIDED | High     | B+C (baby unit reuse)                                                       |
| User Interface           | [user-interface.md](user-interface.md)             | Night mode, volume bars, battery, quiet-since, quality, screen wake lock   | sensors, advanced config; further languages beyond en/de/fr/es                                | DECIDED | High     | All                                                                         |
| Video Monitoring         | [video-monitoring.md](video-monitoring.md)         | --                                                                         | -- (Phase 3+)                                                                                 | IDEA    | Low      | --                                                                          |
| Desktop client           | [desktop-client.md](desktop-client.md)             | --                                                                         | Installable app (Tauri/Electron) wrapping web build; parent-focused; optional                 | IDEA    | Low      | B                                                                           |
| Multi-baby (siblings)    | _see [use-cases.md](../concept/use-cases.md) UC-5_ | --                                                                         | **STATUS: IDEA** -- multiple baby units / rooms; after Phase 2 multi-parent; not MVP          | IDEA    | Low      | B                                                                           |

## Phase Definitions

- **MVP (Phase 1)** -- Android only (Svelte + Capacitor), 3 connectivity paths (local + signaling server + optional VPN), **exactly one baby device and one parent device (1:1)**, DTLS-SRTP + fingerprint binding, saved connections, kiosk mode with physical access protection (**no in-app App-Lock** -- OS screen lock per [ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md)). Google Play + F-Droid distribution. Full documented scope retained (no deliberate reduction). Includes UX: battery display, quiet-since, connection quality, alarm snooze, vibration alert, screen wake lock, visual alert indicators, push-to-listen + slide-to-lock. **PoC gate** before full implementation: [ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md).
- **Phase 2** -- iOS support (Apple App Store), LAN<->Internet automatic handoff, talk-back, **multi-parent: up to 4 parent devices per baby** (same QR “add device” flow as the first pair; QR may be shown on **either** baby or parent — see [connectivity-pairing.md](connectivity-pairing.md)). Multi-parent is also the **privacy-preserving failover** when one parent device is unresponsive ([failure-model.md](../concept/failure-model.md) F-1a). **Pair revocation** and **re-keying after device removal** ([connectivity-pairing.md](connectivity-pairing.md) Cluster Management). Enhanced security (E2E Layer 3, optional periodic key rotation).
- **Phase 3+** -- Video monitoring, on-device AI audio/video analysis, sound categorization (ML), community data contribution, advanced cluster management. **Desktop installable client** (Tauri/Electron) remains **IDEA** and may land earlier if demand and maintenance capacity allow — see [desktop-client.md](desktop-client.md).

## Platform Tiers

| Tier | Platform               | Support Level                                                                                                  |
| ---- | ---------------------- | -------------------------------------------------------------------------------------------------------------- |
| 1    | Android (Capacitor)    | Fully supported by core developer                                                                              |
| 2    | iOS (Capacitor)        | Community-maintained, CI builds via GitHub Actions                                                             |
| 3    | Desktop/Browser (PWA)  | Best-effort, browser fallback                                                                                  |
| 4    | Desktop (native shell) | IDEA — installable `.exe`/`.dmg`/`.AppImage` via Tauri or Electron; see [desktop-client.md](desktop-client.md) |

## Prerequisite: Proof of Concept

Before MVP implementation begins, the following must be validated:

- WebRTC works reliably in Capacitor Android WebView
- Background audio capture works via Android Foreground Service
- Battery consumption is acceptable (target: 8h+ screen-off monitoring)
- Reconnection works after WiFi interruption
- **2026-05 extensions (see [ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md)):** venue-style / captive-network or cellular-fallback path; parent audio + phone-call interruption; older Android baby-unit smoke run ([ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md))

## Deferred and Rejected Features

Features that were considered during brainstorming but not included in the active feature set:

| Feature                                                   | Status   | Reasoning                                                                                                                                                        | Revisit In |
| --------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| iOS support                                               | DEFERRED | Unvalidated background audio, no Apple hardware                                                                                                                  | Phase 2    |
| LAN <-> Internet automatic handoff                        | IDEA     | Both modes in MVP, but switching is manual                                                                                                                       | TBD        |
| Talk-back (parent to baby)                                | DEFERRED | Not essential for monitoring                                                                                                                                     | Phase 2    |
| Application E2E layer (Layer 3)                           | DEFERRED | DTLS + fingerprint binding sufficient for local-only                                                                                                             | Phase 2    |
| Key rotation                                              | DEFERRED | Pairing key valid until manual re-pair                                                                                                                           | Phase 2    |
| Multi-parent (extra parent devices, max 4)                | DEFERRED | MVP is 1:1; Phase 2 adds up to 4 parents, same QR add flow                                                                                                       | Phase 2    |
| Remote pairing (invite code)                              | DEFERRED | Physical QR pairing only in Phase 1                                                                                                                              | Phase 2    |
| Multi-language (en, de, fr, es)                           | DECIDED  | Officially supported from Phase 1 (owner decision 2026-10-19); one shared translation catalogue, see ADR-015. Further languages: community contributions, `IDEA` | Phase 1    |
| Audio sensitivity calibration / room baseline             | DEFERRED | Nice-to-have                                                                                                                                                     | Phase 2    |
| Event history / sleep log                                 | DEFERRED | Adds significant complexity                                                                                                                                      | Phase 2    |
| White noise / lullaby playback                            | REJECTED | Out of scope -- this is a monitor, not a sound player                                                                                                            | --         |
| On-device AI audio/video analysis                         | IDEA     | Feasible (TFLite, YAMNet), but complex; run when charging                                                                                                        | Phase 3+   |
| Community data contribution for model training            | IDEA     | Privacy/legal concerns must be resolved first                                                                                                                    | Phase 3+   |
| Accessibility (screen reader, high contrast)              | DEFERRED | Important for inclusivity, but later phase                                                                                                                       | Phase 2    |
| Sleep/wake scheduling                                     | DEFERRED | Automatic monitoring activation during sleep times                                                                                                               | Phase 2    |
| Video monitoring                                          | DEFERRED | Future feature                                                                                                                                                   | Phase 3+   |
| Sound categorization (ML)                                 | DEFERRED | Future feature                                                                                                                                                   | Phase 3+   |
| Baby-side acoustic alarm on parent-device failure         | REJECTED | Wakes the child; contradicts core purpose ([failure-model.md](../concept/failure-model.md), [ADR-009](../decisions/ADR-009-personas-and-failure-model.md))       | --         |
| Cloud push / SMS / VoIP as alarm when parent phone is off | REJECTED | Violates privacy-first / no cloud in alarm path ([vision.md](../concept/vision.md) Non-Goals)                                                                    | --         |
