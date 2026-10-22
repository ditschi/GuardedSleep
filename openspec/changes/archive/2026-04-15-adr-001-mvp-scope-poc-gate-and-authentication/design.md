# ADR-001: MVP Scope, PoC Gate, and Authentication Strategy

- Status: DECIDED
- Date: 2026-04-15
- Deciders: Product owner (user), coordinated design

> **Partially superseded (2026-10-20):** the Capacitor/WebView PoC mechanism → the native PoC gate in [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md) and [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) (Capacitor shell removed in wave 1). Scope, App-Lock and authentication decisions stand.

## Context

Before implementation we must fix: (1) how much to build before validating risky assumptions, (2) whether to split MVP into smaller releases, (3) whether an in-app lock (PIN/biometrics) is required for MVP.

## Options Considered

### MVP scope

- **Option A -- Full MVP as documented** (single Phase 1, no deliberate reduction of feature set).
- **Option B -- Split Phase 1a / 1b** (core first, then signaling and UX extras).

### App-level lock

- **Option A -- In-app App-Lock (biometrics/PIN)** before saved connections / monitoring.
- **Option B -- No app-lock in MVP**; rely on **OS screen lock** (device PIN/pattern/biometrics) only.

### Validation strategy

- **Option A -- Proof of Concept (PoC) before full MVP** (minimal app to validate WebRTC in Capacitor WebView (superseded, see ADR-015/ADR-020: native PoC), background audio, OEM battery behavior).
- **Option B -- Start full MVP implementation and test in place** (no separate PoC).

## Decision

1. **MVP scope:** Keep the **full documented MVP** -- **do not** reduce scope via Phase 1a/1b for now. Implementation order inside the codebase may still be incremental (e.g. LAN before signaling), but the **delivered MVP** remains the agreed feature set.

2. **App-Lock:** **Not in MVP.** We **do not** implement an in-app PIN/biometrics gate for GuardedSleep in Phase 1. Users rely on the **operating system screen lock** to protect the device. App-lock may be revisited in a later phase (see consequences).

3. **PoC gate:** Run a **focused Proof of Concept** **before** investing in the full application structure. **Do not** skip PoC unless the team explicitly accepts the risk that WebRTC + background audio may fail on target devices.

**PoC -- purpose:** De-risk: Android WebView + Capacitor + WebRTC (superseded, see ADR-015/ADR-020) (DataChannel + pre-negotiated audio), Foreground Service + continuous capture, rough battery read after long run, reconnection after WiFi drop, **optional** Opus DTX / silence bitrate measurement.

**PoC -- minimal deliverable (suggested):**

- Two Android devices (or emulator + device where applicable).
- Capacitor shell + minimal UI (superseded, see ADR-020: shell removed) (no full GuardedSleep UX).
- Pairing: QR or pasted JSON acceptable for PoC if faster.
- One WebRTC session: DataChannel heartbeat + one muted-then-unmuted audio track.
- Foreground Service with ongoing notification on baby side.
- **Success criteria (Go):**
  - Stable connection for >= 30 minutes with screen off on baby device (measure CPU/battery trend).
  - Audio path works when parent app backgrounded (as far as Android allows without full notification UX).
  - After disabling/enabling WiFi, reconnection with stored credentials works **or** failure mode is understood and fixable.
- **No-Go:** WebRTC or microphone breaks in WebView; Foreground Service killed irreproducibly on all test OEMs without mitigation path.

**PoC extensions (2026-05, concept review follow-up):**

- **Captive / venue WiFi:** exercise signaling path from a network that mimics hotel constraints (client isolation or separate subnets) **or** document failure + fallback to cellular data on two phones.
- **Parent audio focus:** place/receive a PSTN or VoIP test call while WebRTC alert audio is playing; record whether child audio resumes automatically and whether extra UX is required ([ADR-011](../2026-10-21-adr-011-audio-routing-parent-side/design.md)).
- **Persona-C device:** repeat the 30-minute baby-side run on an **older Android** representative of repurposed hardware (per [ADR-010](../2026-10-21-adr-010-baby-unit-hardware-requirements/design.md)); capture WebView version + any ICE failures.

**PoC vs "build MVP then test":** Starting full MVP without PoC is **allowed** only if timelines force it; the **default** is PoC first to avoid throwing away large UI/signaling work if the platform choice is wrong.

## Consequences

### Positive

- Clear gate before main engineering investment.
- Simpler MVP: no biometric plugins, no app-lock UX flows.
- Full MVP vision preserved for stakeholders.

### Negative

- **Security:** Unlocked phone with GuardedSleep open exposes saved connections and monitoring as much as any app -- **mitigation is user OS lock**, user education in onboarding.
- **Schedule:** PoC adds 1--2 weeks before "feature" velocity; accepted as insurance.

### Risks

- **Risk:** PoC passes on one device, fails on another OEM → mitigate by testing **at least two OEMs** (e.g. Pixel + one Samsung or Xiaomi class device).
- **Risk:** App-lock deferred → document in privacy materials that **device** security is the user's responsibility.

## Discarded / superseded

- In-app App-Lock as MVP requirement -- **rejected for MVP** (may return as ADR addendum in Phase 2).
- Mandatory Phase 1a/1b split -- **rejected** for now per user decision.
