# Concept Review 2026-05 -- Personas, Failure-Model, Hardware-Reuse, Settings

> **Reviewer**: Critical Reviewer (acting as Coordinator), with later user input shaping CRIT-5 revision.
> **Date**: 2026-05-13
> **Scope**: Review of the foundational GuardedSleep concept (vision, features, ADRs) against three real-world user groups: event-only ("Festtags-Eltern"), daily home use, and hardware-recyclers.
> **Trigger**: user-requested critical review focused on non-technical users, hardware reuse, and settings complexity.
> **Verdict**: NEEDS REWORK at the concept level. Technical foundation (WebRTC + DTLS pinning + Capacitor; superseded, see ADR-015/ADR-020) is solid; the gap is persona coverage, failure model, and settings stratification.

This document is the durable record of the review. The implementation of its findings is tracked through three new ADRs (**ADR-009** Personas + Failure Model, **ADR-010** Baby-Unit hardware, **ADR-011** Parent-side audio routing) and updates to all five feature documents.

---

## 1. Methodology

Three personas were defined and run against every existing concept/feature document:

- **Persona A -- Festtags-Eltern**: event-only use (wedding, restaurant, holiday rental). Once or twice a year. Both phones mobile, no charger present, possibly hostile network (captive portal, AP isolation).
- **Persona B -- Alltags-Eltern**: daily home use, baby unit on charger, parent unit is their main phone.
- **Persona C -- Hardware-Recycler**: uses an old/spare phone or tablet as the baby unit. Often overlaps with Persona B.

Defined in [personas.md](../concept/personas.md). The personas were derived from the user's framing in the original review request and validated against the dominant patterns in [docs/concept/competitive-analysis.md](../concept/competitive-analysis.md).

Failure-Model and Use-Cases were spun out as separate documents during the review:

- [failure-model.md](../concept/failure-model.md)
- [use-cases.md](../concept/use-cases.md)

---

## 2. Findings

### 2.1 CRITICAL

**CRIT-1 -- No documented personas; "the user" is implicitly only Persona B.**

_What is missing_: no persona, no user journey, no use-case matrix. The only Persona-A hint in the entire codebase is one line in [user-interface.md](../features/user-interface.md) (Screen Wake Lock "for party/event scenarios").

_Why it matters_: every default (heartbeat, audio routing, battery, onboarding) is silently optimized for Persona B. Persona A and C inherit suboptimal defaults without anyone having decided that.

_Action_: documented Personas A/B/C in [personas.md](../concept/personas.md); added Persona Impact Matrix; introduced first-launch profile question (SUG-1).

---

**CRIT-2 -- Persona A is de facto uncovered by the current connectivity concept.**

_What is missing_: the "Same WiFi" default assumes both devices are on the same SSID. In hotels, festival halls, and most event venues, this fails because of:

- Captive portals (WSS connections blocked until each device logs in separately)
- Client isolation (devices on the same AP cannot see each other)
- Blocked WebSocket/STUN/TURN ports
- Multi-AP roaming (connection drops as parents walk 20 m)

Hotspot fallback fails the moment the parent leaves the hotspot's physical range -- exactly the wedding case. Internet mode requires functional internet on both devices, which the captive portal breaks.

_Why it matters_: the most emotionally important use case ("baby monitor for a wedding") is the one most likely to fail silently, exactly when failure is most painful.

_Action_: new "Out-of-Home / Captive-WLAN" section in [connectivity-pairing.md](../features/connectivity-pairing.md). Defined UC-3 in [use-cases.md](../concept/use-cases.md) with Cellular-first as the recommended Persona-A path. Added PoC test for captive-WLAN to [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md).

---

**CRIT-3 -- Mobile data volume and roaming costs are invisible for Persona A.**

_What is missing_: [communication.md](../features/communication.md) lists 0.2-18 MB/h for silence frames; active listen mode is ~10-18 MB/h. A 6-hour wedding event easily produces 100-300 MB. No in-app counter, no warning, no roaming hint.

_Action_: in-app data-volume monitor (IDEA -> MVP), Roaming warning when activating Internet mode (DECIDED MVP), session-data-cap setting (Phase 2). Tracked in [communication.md](../features/communication.md).

---

**CRIT-4 -- Persona C (old phone as baby unit) is marketing without a technical safety net.**

_What is missing_: [docs/technical/sw-architecture.md](../technical/sw-architecture.md) uses "repurposed phones as baby unit" to justify Svelte bundle size, but no document defines:

- Minimum Android version / WebView version
- Minimum RAM / storage
- Aged-battery handling (fire risk)
- Microphone quality / WiFi radio expectations
- Security-patch age threshold

Concrete safety risks not addressed:

1. WebView age on Android < 7 -> missing WebRTC features.
2. Weak WiFi radio -> reconnect storms -> false connection-loss alarms.
3. **Battery safety**: aged or swollen LiPo battery near a child's bed is a real fire risk.
4. Microphone quality variation.
5. Frozen security updates.

_Action_: [ADR-010 (PROPOSED)](../../openspec/changes/archive/2026-10-21-adr-010-baby-unit-hardware-requirements/design.md) defines minimum Android API + Self-Test + battery safety notice. New "Baby-Unit Self-Test" feature in [device-management.md](../features/device-management.md) (Mic, WiFi latency, battery health, WebView version) as DECIDED MVP. Added PoC test for Persona-C devices to ADR-001.

---

**CRIT-5 (revised) -- Parent device dead is the most important failure case and was undocumented.**

_Initial proposal_ (rejected by the user): add an acoustic siren on the baby unit when connection is lost.

_Why rejected_: contradicts the core purpose of the app. The whole reason this app exists is to NOT wake the sleeping child. An acoustic alarm on the baby unit is the worst possible mitigation. Captured by the user verbatim:

> "no audio on child side, baby is sleeping and will wake up [...] in the end it is the same as a babyphone -- if parent device is empty we can expect the parent to notice as a expected indicator is missing and thus reconnect without physically having the previously connected baby device."

_Revised model_: GuardedSleep follows the classic baby-monitor failure model. The parent recognizes a missing indicator and reconnects with one tap. The reconnect must not require physical access to the baby device. Documented end-to-end in [failure-model.md](../concept/failure-model.md).

_Action_: five-part response:

- **5a** -- New `failure-model.md`, explicit REJECT-list (acoustic siren, Cloud push, SMS, VoIP), classic baby-monitor analogy.
- **5b** -- Observability: Ambient-Glow on parent screen, Persistent Foreground Notification, Indicator-Training in onboarding (NEW-1).
- **5c** -- One-tap reconnect from saved connection -- specification hardened in [connectivity-pairing.md](../features/connectivity-pairing.md).
- **5d** -- Phase-2 Multi-Parent reframed from comfort to **failover**: when Phone-1 is unobservable, baby unit escalates to Phone-2.
- **5e** -- Pre-monitoring parent-battery reminder; Persona-aware default to warn at <30 %.

Additionally, this revision drove two new findings (NEW-1, NEW-2 below) and a new Non-Goal entry in [vision.md](../concept/vision.md).

---

**CRIT-6 -- Settings are a flat mix of basic and advanced; needs Basic/Advanced stratification.**

_What is missing_: a single Settings screen mixes "Volume threshold" with "Heartbeat 5-30s" and "Signaling URL override". Non-technical Persona A/B easily nudges a critical parameter and silently degrades monitoring.

_Action_: Basic/Advanced toggle in [user-interface.md](../features/user-interface.md) and [ui-design.md](../technical/ui-design.md):

- Basic (default): Notifications, Display, Saved Connections, Battery & Background (auto-wizard), About.
- Advanced (sticky toggle with warning): Heartbeat, Detection-Duration, Audio Codec, Signaling URL, VPN mode, Emoji-Fingerprint, Debug Logs, Session Data Cap, Self-Test, Persona-Profile switch.

Hard rule: all defaults work for both personas without entering Advanced.

---

**CRIT-7 -- Phone-call interruption and Bluetooth-headphone routing are undocumented; ADR required.**

_What is missing_: [notifications-alerts.md](../features/notifications-alerts.md) leaves open: "Should alerts play through the phone speaker even if headphones are connected? (Likely yes for safety)". For Persona A at a wedding, "Likely yes" is wrong -- they may be wearing in-ears specifically to monitor discreetly.

_Action_: [ADR-011 (PROPOSED)](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md) -- Option C (persona-dependent default, overridable in Advanced). Phone-call-interruption behavior to be specified and validated in PoC.

---

### 2.2 IMPORTANT

**IMP-1 -- Expectation gap vs. hardware baby monitors.**

Daily users switching from Avent / Philips will expect: nightlight, lullabies (REJECTED -- but should be sign-posted), talk-back (DEFERRED Phase 2 -- top-3 expectation), temp/humidity sensor (IDEA), standalone receiver mode (no, parent phone IS the receiver). Captured in [user-interface.md](../features/user-interface.md) About section + FAQ.

**IMP-2 -- Persona-A Pre-Departure Wizard missing.**

Existing "Verbindung testen" button is too passive. Persona A needs: pre-flight checklist, probe-alarm, 60-second synthetic stream test, at the venue, before parents leave the room. Added to [notifications-alerts.md](../features/notifications-alerts.md).

**IMP-3 -- Multiple children / siblings missing.**

Multi-baby (Geschwister) is not even an IDEA today. MVP is 1+1, Phase 2 is 1+4 parents (not 2+1). Captured as UC-5 in [use-cases.md](../concept/use-cases.md) and listed as Phase 3+ IDEA in [features/README.md](../features/README.md).

**IMP-4 -- Pair-Lifecycle / babysitter risk.**

Once paired, always paired. No revocation, no expiry. Risk for grandparents / former babysitters / stolen phones. Action: pair revocation + key rotation moved from IDEA to DECIDED Phase 2 in [connectivity-pairing.md](../features/connectivity-pairing.md). New "Babysitter Mode" (SUG-4 below) for time-limited pairing.

**IMP-5 -- Auto-Resume / Boot resilience.**

After OS update or power outage, the baby unit does NOT auto-restart monitoring; user thinks they are monitoring but are silently offline. Action: Auto-Resume Baby-Mode after reboot as DECIDED MVP (Advanced setting, default ON for Persona B). Documented in [device-management.md](../features/device-management.md).

**IMP-6 -- Long-term lifecycle / single-developer risk.**

Single-dev project, security patch cadence and store availability not guaranteed. Persona B is most exposed (years of dependency). Action: new [lifecycle.md](../legal/lifecycle.md) document; in-app warning for stale versions; export path for saved connections.

**IMP-7 -- Range expectations vs. hardware baby monitors.**

"100 m like my Avent SCD843" is not what GuardedSleep delivers -- it delivers either unlimited (within WLAN/internet) or 10-30 m (hotspot). Needs explicit FAQ; folded into the Hardware-vs-Software FAQ in [user-interface.md](../features/user-interface.md).

**IMP-8 -- Battery-optimization deep-link is OEM hell.**

OEM-specific behavior (Samsung, Xiaomi, Huawei, OnePlus) varies wildly. Persona C is on the most aggressive OEMs. Action: Lakmus test -- count heartbeats over a 30 min window after onboarding to detect silent OS-kill (IDEA in [device-management.md](../features/device-management.md)).

---

### 2.3 SUGGESTION

- **SUG-1** -- Persona-based onboarding (single question at first launch). Documented in [user-interface.md](../features/user-interface.md).
- **SUG-2** -- Probe-Alarm button always visible on parent screen. DECIDED MVP in [notifications-alerts.md](../features/notifications-alerts.md).
- **SUG-3** -- Emoji-Fingerprints as default in the hidden Expert security view. DECIDED in [user-interface.md](../features/user-interface.md).
- **SUG-4** -- "Babysitter-Mode": time-limited pairing (e.g. 12 h). Captured as Phase 2 IDEA in [connectivity-pairing.md](../features/connectivity-pairing.md) and as UC-2 in [use-cases.md](../concept/use-cases.md).
- **SUG-5** -- Hardware vs Software FAQ section in About. Captured as IDEA in [user-interface.md](../features/user-interface.md).
- **SUG-6** -- Session data cap for Persona A (mobile data protection). Captured in [communication.md](../features/communication.md).
- **SUG-7** -- Baby-Unit battery-health self-test on first start (battery age, swelling warning). Captured in [device-management.md](../features/device-management.md). Drives ADR-010.

---

### 2.4 NEW Findings (raised during CRIT-5 revision)

**NEW-1 -- Indicator-Loss recognition must be trained.**

If the safety model relies on the parent noticing a missing indicator, the parent must be taught what that looks like. Onboarding step: show "Connected" look, show "Disconnected" look, run a 5 s demonstration. Optional periodic re-training. Captured in [user-interface.md](../features/user-interface.md).

**NEW-2 -- Communicate the failure model before installation.**

Play Store / F-Droid listings (F-Droid main deferred, ADR-021 §3) and the first onboarding slide must explicitly state: "Like every baby monitor, GuardedSleep cannot alert if your receiver phone dies. Keep it charged." Added as a Non-Goal in [vision.md](../concept/vision.md). Onboarding slide added to [user-interface.md](../features/user-interface.md).

---

## 3. Cross-document consistency checks

| Checklist item from [critical-reviewer.md](../roles/critical-reviewer.md) | Status before review                                                | Status after review                               |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------- |
| No contradictions with other documents                                    | Audio-routing contradicted Persona A                                | ADR-009 will resolve                              |
| All assumptions explicit                                                  | Personas, hardware, network assumptions all implicit                | personas.md + use-cases.md make them explicit     |
| Edge cases & error scenarios                                              | Heartbeat/reconnect OK; parent-dead/captive-WLAN/phone-call missing | failure-model.md covers all five cases (F-1..F-7) |
| Scope realistic for one developer                                         | MVP yes; Persona-A coverage adds PoC scope                          | ADR-001 PoC scope extended                        |
| Compared against existing solutions                                       | Competitive-analysis covers software; hardware comparison missing   | Hardware-vs-Software FAQ added                    |
| Failure modes identified + mitigated                                      | Parent-dead and battery-fire missing                                | failure-model.md + ADR-010                        |
| Happy path is not the only path                                           | Daily happy path strong; event path almost absent                   | use-cases.md UC-3/UC-4 add the event path         |

---

## 4. Verdict

**NEEDS REWORK (concept level)**.

The technical foundation does not need architectural changes. The blind spots are all at the concept/UX/operations layer:

- Persona coverage (CRIT-1)
- Failure model (CRIT-5 revised)
- Hardware minimums (CRIT-4)
- Audio routing (CRIT-7)
- Settings hierarchy (CRIT-6)

Three new ADRs (009, 010, 011) materialize the three reworks that change "core concepts" per [AGENTS.md §6.2](../../AGENTS.md). All other findings ride along on feature-document updates.

---

## 5. Action plan

Tracked in plan file `concept_review_findings_f5631aa5.plan.md`. Execution status:

| Step                                                                   | Status |
| ---------------------------------------------------------------------- | ------ |
| 1 -- Foundation docs (personas, failure-model, use-cases, this review) | done   |
| 2 -- ADR-009 / 010 / 011                                               | done   |
| 3 -- vision.md + features/README.md                                    | done   |
| 4 -- five feature docs                                                 | done   |
| 5 -- ui-design.md mocks                                                | done   |
| 6 -- ADR-001 PoC extension + decisions/README index                    | done   |
| 7 -- lifecycle.md                                                      | done   |
| 8 -- consistency check                                                 | done   |

## 6. Discarded review threads

| Thread                                                   | Reason                                                                                                                                                               |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Add acoustic siren on baby unit when connection is lost  | Rejected -- wakes the sleeping child, contradicts the core purpose. User-rejected during CRIT-5 revision.                                                            |
| Mandatory cloud push as alarm fallback                   | Rejected -- violates privacy-first principle ([ADR-005](../../openspec/changes/archive/2026-04-15-adr-005-parent-alerts-product-naming-and-distribution/design.md)). |
| Require a second device as hardware fallback             | Rejected -- excludes single-parent setups; mismatches Persona A.                                                                                                     |
| Whitelist of approved baby-unit devices                  | Rejected for ADR-010 -- too restrictive; replaced by API minimum + Self-Test.                                                                                        |
| Force speaker output for alerts regardless of headphones | Rejected for ADR-009 -- Persona A failure mode; replaced by persona-aware default.                                                                                   |
