# Feature: Notifications and Alerts

## Overview

The parent unit must reliably alert the caregiver when events are detected on the baby unit. Alerts must be attention-grabbing enough to wake a sleeping parent, but also configurable to avoid false alarm fatigue.

**Failure model:** When the **parent phone** is off, crashed, or killed by the OS, GuardedSleep cannot deliver alerts -- same contract as a hardware receiver. Mitigations are **observable parent-side state** (persistent notification, ambient connection affordance, onboarding training) and **one-tap reconnect** without visiting the baby device ([failure-model.md](../concept/failure-model.md), [ADR-009](../decisions/ADR-009-personas-and-failure-model.md)). **Baby-side sirens** when the parent link drops are **out of scope** (they wake the child).

**Audio output routing** for alert-triggered child audio follows [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md) (**one** primary UX with Advanced overrides; not separate persona skins).

## Requirements

- `[STATUS: DECIDED]` Vibration + immediate child audio + **screen on + app to foreground + volume meter visible** when noise threshold exceeded -- **MVP**
- `[STATUS: DECIDED]` Low battery warning: alert parent when baby unit battery is critically low -- **MVP**
- `[STATUS: DECIDED]` Connection lost warning: alert parent when heartbeat is missed -- **MVP**
- `[STATUS: DECIDED]` Alarm snooze / mute button (monitoring continues, notifications paused) -- **MVP**
- `[STATUS: DECIDED]` **MVP:** **Persistent foreground-service notification** on the **parent** unit while monitoring is expected to be active ("GuardedSleep -- connected to [name]") so that a dead/killed app is noticeable when the notification disappears ([failure-model.md](../concept/failure-model.md)).
- `[STATUS: DECIDED]` **MVP:** **Probe alarm / connection test** action on parent UI -- runs the full alert chain once (vibration + screen + routing per [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)) without needing a real cry; for use after OS updates and before leaving for an event.
- `[STATUS: DECIDED]` **Phase 2:** When **multi-parent** is active, **failover banner + alert escalation** to another parent device if the primary parent stops acknowledging heartbeats ([failure-model.md](../concept/failure-model.md) F-1a, [device-management.md](device-management.md)).
- `[STATUS: DECIDED]` **MVP:** Parent settings (Basic → Notifications): **Alert audio output** -- Default (system routing: BT/headphones if connected, else speaker), **Speaker only**, or **Speaker + default device** (dual route; PoC-gated per OEM); plus **alert playback gain** slider and optional **minimum gain during alert** -- see [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)
- `[STATUS: IDEA]` Configurable alert behavior (which combination of ringtone/vibration/screen beyond the MVP chain)
- `[STATUS: IDEA]` **Connection-context alert policy:** when the parent device is on **unmetered Wi-Fi**, use the full agreed alert chain (including child-audio quality per codec defaults). When on **metered / cellular** (Internet path), optionally **tune** non-safety elements (e.g. slightly lower media bitrate, stronger pre-vibration, or an extra banner: "On mobile data -- tap to confirm full audio") while **never** silently dropping connection-loss escalation -- exact split requires UX alignment with [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)

## Sub-Features

### Noise Threshold Alerts

Triggered by the [audio monitoring](audio-monitoring.md) system:

- Must work even when the parent app is in the background
- Must override Do Not Disturb / silent mode (see technical details below)
- **Alert sequence (no separate alarm ringtone):** vibration → screen turns on → app brought to foreground → **volume meter / noise bars show live level** → child audio plays on the route chosen in **Settings → Notifications → Alert audio output** ([ADR-011](../decisions/ADR-011-audio-routing-parent-side.md): Default / Speaker only / Speaker + default). Playback uses the **alarm-class audio path** (`USAGE_ALARM`) so **alarm volume** applies; **in-app gain** slider scales decoded level before output (see _Parent alert output and volume_ below).
- Parent sees at a glance that the app is active and what is happening (visual + audio)

### Parent alert output and volume -- MVP

> Product spec: [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md). Summarized here for implementers.

- **Alert audio output** (Basic): **Default** | **Speaker only** | **Speaker + default device** (dual; PoC-gated).
- **Alert playback gain:** software slider scaling the alert path; user still uses **system alarm volume** as the master ceiling.
- **Optional minimum gain during alert:** reduces "forgot slider at 30 %" failures; does not bypass hardware mute.

### Pre-flight / probe test (parent device)

> `[STATUS: DECIDED]` **MVP** -- short label in UI: "Test alert" or "Probe".

- Triggers the same native path as a real noise alert (notification channel, vibration, foreground, audio routing per [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)) **without** requiring the baby to cry.
- Offered: after onboarding, after OS update, and prominently before **out-of-home** sessions ([use-cases.md](../concept/use-cases.md) UC-3).

### Silent Mode / DND Override -- MVP (Technical Detail)

> STATUS: DECIDED -- This is a core safety requirement. A baby monitor that goes silent when the phone is on silent is useless.

**How it works on Android:**

The app uses a **dedicated Notification Channel** with alarm-level priority. Three mechanisms work together:

1. **DND Bypass via NotificationChannel:**
   - Create a notification channel with `setBypassDnd(true)`
   - Requires `ACCESS_NOTIFICATION_POLICY` permission
   - During setup, guide user to system settings to grant "Override Do Not Disturb" for our app
   - This is a one-time setup step (part of the battery optimization setup flow)

2. **Audio via ALARM stream (overrides silent mode):**
   - **Noise-alert and probe-test playback** on the parent MUST be attributed to `AudioAttributes.USAGE_ALARM` so the user can rely on the **alarm / clock volume rocker** (distinct from media volume on many devices).
   - The ALARM stream is independent of the ringer mode (silent/vibrate/normal)
   - `FLAG_AUDIBILITY_ENFORCED` ensures playback even at low volume
   - **In-app gain** (settings slider) applies **after** decode, **before** the hardware mixer -- it does **not** replace the system alarm index; copy should tell users to raise **alarm volume** if probe is still too quiet.
   - Continuous **listen mode** WebRTC may use `USAGE_MEDIA` unless/until unified with the alert path ([ADR-011](../decisions/ADR-011-audio-routing-parent-side.md))

3. **Vibration always works:**
   - Vibration is triggered programmatically via `Vibrator` API
   - Works regardless of ringer mode when app has notification permission

**User consent and transparency:**

- During first setup: explain clearly why DND override is needed ("So you never miss your baby crying")
- User can opt out in settings (but with clear warning: "Alerts will be silent when your phone is muted")
- App does NOT change the device's ringer mode -- it only plays on the alarm stream
- Google Play policy compliant: user grants permission explicitly

**Capacitor implementation:**

- Requires a custom Capacitor plugin (or native Android code in the Capacitor project)
- The plugin creates the NotificationChannel with `setBypassDnd(true)` and plays **alert / probe** child-audio via `MediaPlayer` (or `AudioTrack`) with `USAGE_ALARM` and the user's **output mode** + **in-app gain** from settings
- **Listen mode** continuous WebRTC decode may still use `USAGE_MEDIA` until native bridges unify both paths ([ADR-011](../decisions/ADR-011-audio-routing-parent-side.md))

**What this means for the user:**

- Phone on silent → baby noise detected → **vibration + screen on + app open + volume meter + child audio** on the route defined by [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md) (no separate siren tone; the child's voice is the alert)
- Phone on DND → same behavior (if user granted DND override permission)
- Phone on normal → standard behavior

**Limitation:** Some OEMs (Xiaomi MIUI, Huawei EMUI) may restrict alarm stream behavior. Test in PoC on aggressive OEMs.

### Low Battery Warning

- Baby unit monitors its own battery level
- Sends warning to parent at configurable threshold (e.g., 20%, 10%)
- Parent sees a clear, non-dismissible warning
- Different urgency level than noise alerts (important but not urgent)

### Connection Lost Alert

- Triggered when heartbeat is missed for configurable duration
- Escalating urgency: first a gentle notification, then more urgent if connection remains lost
- Must distinguish between "brief glitch" and "sustained disconnection"

### Vibration Alert -- MVP

- Vibration pattern when noise threshold exceeded (together with screen + volume meter + child audio)
- Works even in silent mode / Do Not Disturb
- Optional future setting: vibration strength / pattern (MVP: default pattern)

### Alarm Snooze / Mute -- MVP

- "Snooze" button pauses notifications for a configurable period (e.g., 5/10/15 minutes)
- Monitoring continues in background; noise events still recorded
- Auto-resume after snooze period expires
- Visual indicator that snooze is active
- Prevents alert fatigue when parent is already attending to child

### Alert Configuration

Users should be able to configure:

- **Alert audio output**, **playback gain**, and optional **minimum gain during alert** (MVP -- see [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md) and _Parent alert output and volume_ above)
- Which alert types trigger which notification behavior (sound, vibration, screen) beyond the MVP chain -- Phase 2 / IDEA
- Cool-down period between repeated alerts for the same event

## Dependencies

- [Communication](communication.md) -- receives alert messages and heartbeat status
- [Audio Monitoring](audio-monitoring.md) -- source of noise threshold events
- [Device Management](device-management.md) -- battery level reporting

## Open Questions

- ~~Can we override Do Not Disturb mode on Android and iOS? What permissions are needed?~~ **RESOLVED**: Yes, via NotificationChannel + ALARM stream + DND bypass permission. See technical detail above.
- ~~Should alerts play through the phone speaker even if headphones are connected?~~ **RESOLVED via [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md)**: user-selectable **Alert audio output** (Default / Speaker only / Speaker + default), **alarm-class playback** (`USAGE_ALARM`), and **in-app gain**; dual-route PoC-gated.
- **Incoming phone call** while alerting or listening: document actual Android audio-focus behavior in PoC; app should **attempt auto-resume** after call ends and surface a banner if resume fails.
- How to handle alert fatigue if the baby is noisy for extended periods? (Snooze feature addresses this)
- ~~Platform-specific push notification requirements (FCM, APNs, or custom solution?)~~ **RESOLVED for MVP**: No FCM/APNs in MVP. App must be running (Foreground Service). Documented as known limitation.

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
