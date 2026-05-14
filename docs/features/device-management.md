# Feature: Device Management

## Overview

Manages the device modes (baby unit vs. parent unit), battery optimization, and background execution. These are largely platform-level concerns that directly affect the feasibility and reliability of the entire application.

## Requirements

- `[STATUS: DECIDED]` Two device modes: Baby Unit and Parent Unit, selectable at app startup or in settings -- **MVP**
- `[STATUS: DECIDED]` Battery optimization: baby unit must run for extended periods (hours) without excessive drain -- **MVP**
- `[STATUS: DECIDED]` Background execution: audio monitoring must continue when the app is not in the foreground -- **MVP**
- `[STATUS: DECIDED]` Low battery reporting: baby unit sends battery level to parent unit -- **MVP**
- `[STATUS: DECIDED]` Deep-link to Android battery optimization settings -- **MVP**
- `[STATUS: DECIDED]` Minimal kiosk/screen-lock on baby unit -- **MVP**
- `[STATUS: DECIDED]` **MVP:** one baby + one parent (1:1). **Phase 2:** one baby + up to **4** parent devices; each extra parent is added with the **same QR flow** as the first device (`Add device` → show QR → scan).
- `[STATUS: DECIDED]` **MVP:** Baby-unit **self-test** on first baby-mode start (or when user marks device as "old/spare"): microphone check, basic WebRTC/WebView sanity, optional WiFi RTT / signaling reachability, battery level + health hint where APIs exist -- outcome: recommended / limited / not recommended ([ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md)).
- `[STATUS: DECIDED]` **MVP:** In-app **battery safety copy** for baby unit: do not use devices with swollen or damaged batteries; place on stable surface; follow OEM charging guidance ([ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md)).
- `[STATUS: DECIDED]` **MVP:** **Auto-resume baby mode after reboot** (Advanced setting, default ON for "daily home" profile): after phone unlock, return to baby monitoring + kiosk path without requiring the parent to re-open the app manually ([failure-model.md](../concept/failure-model.md)).
- `[STATUS: IDEA]` **Phase 2:** Auto-start parent mode after reboot (more conservative; event users may not want auto-start).
- `[STATUS: IDEA]` **MVP / Phase 1 polish:** OEM **lakmus** test -- count heartbeats for ~30 minutes after battery-optimization onboarding; warn if counts imply OS is killing the app in background.

## Sub-Features

### Device Modes

- **Baby Unit Mode**: activates microphone, runs noise detection, sends alerts, receives talk-back audio
- **Parent Unit Mode**: receives alerts, displays status, provides listen mode and talk-back controls
- Mode selection should be simple and clear (large buttons, no ambiguity)
- A device can only be in one mode at a time

### Battery Optimization

The baby unit is the critical concern -- it runs continuously:

- Minimize CPU usage during idle periods (no noise detected)
- Reduce screen brightness or turn off screen entirely in baby mode
- Optimize audio sampling rate and processing frequency
- Provide estimated remaining runtime to the parent

### Background Execution

This is one of the most critical technical challenges:

- **Android**: Foreground services with persistent notification can keep the app alive
- **iOS**: Severely limited background execution; audio background mode helps but has restrictions
- **PWA**: Very limited background capabilities, especially on iOS
- The chosen platform approach (see [technical analysis](../technical/analysis.md)) will heavily depend on this constraint

### Baby-unit hardware floor and self-test

> STATUS: DECIDED intent for MVP; numeric floor (min API) **PROPOSED** in [ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md) pending PoC.

- **Why**: "Use an old phone" is a core story ([personas.md](../concept/personas.md) Persona C) but unmanaged old hardware drives WebRTC flakiness and real-world battery safety issues.
- **Self-test flows**: short guided checks (mic level, WebView/WebRTC probe, optional WiFi RTT to signaling or LAN gateway, battery % + OEM health hint when available). Outcome gates only the **baby** role -- parent role may still install on older devices with warnings.
- **Battery safety**: visible warning if user indicates device stays on charger 24/7 near crib; link to OEM guidance; never claim medical/fire certification -- plain-language hazard reduction only.

### Phase 2: Multi-parent (same baby, up to 4 parents)

> STATUS: DECIDED scope for parents; additional babies/rooms remain future.

- **One baby device** may have up to **four** paired **parent** devices (product-enforced cap).
- **Adding parent 2–4** uses the **identical** pairing mechanism as parent 1: **Add device** → device shows **QR** → new device **scans**.
- **QR host:** either the **baby** or any **already-paired parent** may display the QR. Baby: only after **kiosk unlock** and explicit **Add device** (prevents accidental pairing while monitoring).
- Signaling/WebRTC: multiple parallel parent sessions are a Phase 2 engineering task (see [connectivity-pairing.md](connectivity-pairing.md)).
- **Failover:** when one parent device stops responding, the baby unit escalates monitoring alerts to another paired parent ([failure-model.md](../concept/failure-model.md) F-1a) -- not only a convenience feature.

**Later / not committed in this scope:**

- Multiple **baby** units (one per child/room) with one parent app switching between them
- Talk-back targeting one baby when multiple babies exist

### Minimal Kiosk / Screen Lock -- MVP

> STATUS: DECIDED -- Moved from DEFERRED to MVP (low effort, high UX impact)

- Lock the screen to prevent accidental taps on baby unit
- Keep the app in foreground, prevent switching
- **Unlock: Hold + Slide** (same gesture family as push-to-listen on parent screen)
  - Touch and hold → unlock area appears at bottom
  - Slide up to unlock area → screen unlocks
  - Release early = nothing happens
  - Consistent pattern across the app (push-to-listen, slide-to-lock, hold-to-unlock)
- **Auto screen dimming**: screen dims to minimum over 5s, then near-black
- **Touch to wake**: any touch briefly wakes screen (3s) showing status, then dims again
- Shows small status icons: signal strength + battery (12dp, color-coded)
- Option: screen completely off (icons only visible on touch)

### Deep-Link to Battery Optimization Settings -- MVP

> STATUS: DECIDED

- Button in setup flow that opens the device's battery optimization settings
- Guides user to whitelist the app from battery restrictions (OEM-specific: Samsung, Xiaomi, Huawei etc.)
- Critical for reliable background execution on Android
- Use [Don't Kill My App](https://dontkillmyapp.com/) as reference for OEM-specific instructions
- Show simple explanation why this step is needed
- **Platform note**: This feature is Android-only. On iOS (Phase 2), this UI element must be hidden -- iOS does not expose battery optimization settings in the same way. Use platform detection to conditionally show/hide.

## Dependencies

- [Audio Monitoring](audio-monitoring.md) -- battery and background execution directly affect monitoring
- [Connectivity](connectivity-pairing.md) -- battery level reporting uses the connection
- [Notifications](notifications-alerts.md) -- low battery alerts

## Security: Physical Access Protection -- MVP

> STATUS: DECIDED -- Kiosk + QR rules + parent notifications; **no in-app App-Lock in MVP** ([ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md)). Rely on **OS screen lock** for device protection.

**Kiosk mode is the primary defense against unauthorized physical access:**

- In baby mode: kiosk mode is DEFAULT ON
- Kiosk prevents: app switching, settings changes, QR code display, uninstallation
- Unlock: Hold + Slide (same gesture as push-to-listen on parent screen -- consistent interaction pattern)
- After unlock: 30-second inactivity timeout → auto re-lock

**QR code / Add device in baby mode (MVP vs Phase 2):**

- **MVP (1:1):** Initial pairing happens during setup; no second parent yet. If UX keeps QR off the baby during active monitoring-only flows, that remains a product detail — **Phase 2 requirement** is explicit below.
- **Phase 2:** Baby unit **may** show **Add device** QR (same flow as parent) after **kiosk unlock** and user confirmation — so a second parent can pair **without** forcing baby mode exit, while still requiring physical access + intent.
- Showing QR without unlock remains **disallowed**; kiosk is the gate for baby-side pairing UI.

**Parent notification on unlock:**

- When someone unlocks kiosk mode: instant notification to all connected parent devices: "Baby unit [name] was unlocked"
- When baby mode is exited: warning to parents: "Baby unit [name] is no longer monitoring!"

## Open Questions

- How aggressive can we be with screen-off / CPU throttling on the baby unit?
- What are the exact background execution capabilities of each target platform?
- How does multi-parent (max 4) affect battery and WebRTC session count on the baby unit? (Phase 2 engineering.)
- ~~Should device mode be persistent (remembered) or selected each time?~~ **RESOLVED**: Remember last role + one-tap confirmation

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
