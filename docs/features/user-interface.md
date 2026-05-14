# Feature: User Interface

## Overview

The user interface must be simple, reliable, and usable under stress (tired parent, dark room, urgent situation). Design decisions here directly impact whether non-technical users will adopt the app.

**Personas, use cases, and failure observability** drive several MVP UX additions -- see [personas.md](../concept/personas.md), [use-cases.md](../concept/use-cases.md), [failure-model.md](../concept/failure-model.md), and [ADR-009](../decisions/ADR-009-personas-and-failure-model.md).

## Requirements

- `[STATUS: DECIDED]` On noise alert: screen wakes, app comes to foreground, **volume meter / noise bars show live level** while child audio plays -- **MVP**
- `[STATUS: DECIDED]` Night mode / dark UI: essential for nighttime use without blinding the parent -- **MVP**
- `[STATUS: DECIDED]` Live volume level display: visual noise meter on parent device -- **MVP**
- `[STATUS: DECIDED]` "All quiet since..." indicator: shows time since last noise event -- **MVP**
- `[STATUS: DECIDED]` Connection quality indicator (good/medium/poor from WebRTC stats) -- **MVP**
- `[STATUS: DECIDED]` Baby battery level display on parent device -- **MVP**
- `[STATUS: DECIDED]` Screen wake lock option in listen mode (for party/event scenarios) -- **MVP**
- `[STATUS: IDEA]` Multi-language support (i18n): important for community adoption
- `[STATUS: IDEA]` Temperature / humidity display: show sensor data if the device supports it
- `[STATUS: DECIDED]` **MVP:** **Ambient connection affordance** on parent monitoring screen (e.g. thin top-edge glow / bar: green connected, yellow reconnecting, red disconnected) so "monitoring off" is noticeable at a glance ([failure-model.md](../concept/failure-model.md), [ui-design.md](../technical/ui-design.md)).
- `[STATUS: DECIDED]` **MVP:** **Indicator training** during onboarding -- show "connected" vs "disconnected" visuals once; copy explains that missing green indicator means you are not monitoring.
- `[STATUS: DECIDED]` **MVP:** **One unified app** for all contexts (home + travel + events); **no** persona-specific layouts or mandatory “home vs event” profile that forks defaults. **Context-aware** behavior (network metered, captive portal failure, connection quality) is documented in [connectivity-pairing.md](connectivity-pairing.md) and [notifications-alerts.md](notifications-alerts.md); design lenses remain in [personas.md](../concept/personas.md).
- `[STATUS: DECIDED]` **MVP:** **Settings: Basic vs Advanced** -- Advanced hidden behind toggle + short warning; dangerous knobs (heartbeat interval, signaling URL, codec tweaks) live only in Advanced ([concept review](../reviews/concept-review-2026-05.md) CRIT-6).
- `[STATUS: DECIDED]` **MVP (Expert-only):** Optional **emoji fingerprint** display for manual verification under Security → Connection details (friendlier than hex; see [concept review](../reviews/concept-review-2026-05.md) SUG-3).
- `[STATUS: IDEA]` **About / FAQ:** short **hardware vs software baby monitor** comparison and **range expectations** (WiFi vs hotspot vs internet) to pre-empt false comparisons to dedicated RF hardware ([concept review](../reviews/concept-review-2026-05.md) SUG-5, IMP-7).
- `[STATUS: IDEA]` **Nightlight mode** on baby unit (very dim warm full-screen or edge glow) -- parents often expect this from hardware competitors; low priority, must not wake child ([concept review](../reviews/concept-review-2026-05.md) IMP-1).

### Night Mode / Dark UI

> STATUS: DECIDED -- Default dark with flexible settings.

- **Default: always dark** (baby monitors are primarily used in dark environments)
- Manual toggle in settings to switch to light mode for daytime use
- Settings for auto-switch: based on time of day (configurable schedule) or system dark mode setting
- Minimal brightness, high contrast for essential information
- Baby unit screen should be as dim as possible (or off) to not disturb the child
- Good default: dark mode ON, auto-switch OFF (user can enable if desired)
- This flexible approach gives power users control while keeping the default optimal for 90% of use

### Volume Level Display -- MVP

- Real-time visual representation of noise level on the parent device
- Shows current level even when not in listen mode (derived from detection data, not audio stream)
- Historical mini-graph showing recent activity (last 5-10 minutes)
- Color-coded: green (quiet), yellow (some noise), red (threshold exceeded)
- **Large volume bars** visible from a distance (e.g., phone on a table at a party)
- **Pulsing/blinking visual alert indicator** when noise is detected (attention-grabbing without sound)

### "All Quiet Since..." Indicator -- MVP

- Shows timestamp of last noise event ("All quiet since 22:14")
- Gives parents confidence that the child is sleeping
- Resets when new noise event detected
- Large, readable text in night mode

### Connection Quality Indicator -- MVP

- Shows current connection quality as icon (good / medium / poor)
- Based on WebRTC stats API (RTT, packet loss, jitter)
- Warns when connection is degrading before it drops
- Color-coded: green, yellow, red

### Baby Battery Level Display -- MVP

- Shows baby device battery percentage on parent UI
- Transmitted via WebRTC DataChannel heartbeat
- Warning colors at low levels (yellow <30%, red <15%)
- Integrates with low-battery alert notification

### Screen Wake Lock (Listen Mode) -- MVP

- Optional setting: keep parent screen awake while in listen mode
- Designed for scenarios where parent cannot hear audio alerts (party, wedding, noisy environment)
- Screen shows volume bars, visual alert indicators, and status at a glance
- Dims but stays on; night mode colors to save battery
- Auto-disable after configurable timeout to prevent forgotten wake lock

### Settings hierarchy (Basic vs Advanced) -- MVP

> `[STATUS: DECIDED]` -- keeps casual users from breaking monitoring via expert knobs.

**Basic (default):** Notifications (threshold + duration + snooze defaults; **alert audio output** Default / Speaker / Speaker+default; **alert playback gain** + optional **minimum gain during alert** per [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)), Display (night mode + wake lock in listen mode), Saved connections, Battery & background (wizard + deep link), About / Help.

**Advanced (toggle + warning):** Heartbeat interval, audio codec/bitrate experiments, signaling URL override, VPN hints, emoji fingerprint / security diagnostics, debug log export, auto-resume toggles, optional session data cap hooks ([communication.md](communication.md)).

Rule: **all defaults must work in both quiet home nights and loud social venues** without opening Advanced -- adaptation uses **connectivity and OS signals** (metered, roaming, failed venue WiFi), not a user-declared persona.

### Onboarding and indicator training -- MVP

- **No** first-launch “persona profile” fork (see [personas.md](../concept/personas.md) “Unified app”).
- **Indicator training** during onboarding -- show "connected" vs "disconnected" visuals once; copy explains that missing green indicator means you are not monitoring.
- Optional **probe alert** hand-off to [notifications-alerts.md](notifications-alerts.md) (recommended once after onboarding and before leaving baby out of earshot).

### Multi-Language Support (i18n)

- English as the default language
- Architecture supports easy addition of new languages
- Community can contribute translations
- UI text externalized from code from the start

### Temperature / Humidity Display

> STATUS: IDEA -- Depends on device sensor availability

- If the baby unit device has temperature/humidity sensors, display readings on parent unit
- Common in commercial baby monitors, parents expect this feature
- Not all phones have these sensors -- must handle gracefully when unavailable
- Low priority but low effort if sensors are accessible via platform APIs

## Dependencies

- [Audio Monitoring](audio-monitoring.md) -- volume level data for display
- [Communication](communication.md) -- transmitting sensor data to parent unit
- [Device Management](device-management.md) -- screen brightness control in baby mode

## Open Questions

- ~~Should night mode be automatic or manual toggle?~~ **RESOLVED**: Default dark, manual toggle + optional auto-switch in settings
- Which i18n framework to use? (depends on platform decision)
- How many languages for the initial release? (English + German?)
- Which devices actually have accessible temperature/humidity sensors?
- Phone vs. tablet layout: responsive design approach? Breakpoints?

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
