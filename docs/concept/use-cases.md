# Use Cases

> STATUS: PROPOSED -- companion document to [personas.md](personas.md) and [failure-model.md](failure-model.md); finalized via [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md).
>
> **Why this document exists**: features should be designed against concrete situations, not against an abstract "user". The [concept review 2026-05](../reviews/concept-review-2026-05.md) (finding CRIT-1) called out the lack of a use-case catalogue as a root cause for blind spots like the captive-WLAN problem (CRIT-2) and the parent-device-dead case (CRIT-5).

## Schema

Each use case is described by:

- **Persona** -- primary user(s) from [personas.md](personas.md)
- **Pre-conditions** -- what is already set up
- **Setup steps** -- what the user does at the start of the session
- **Expected connectivity path** -- which of the three connectivity modes from [connectivity-pairing.md](../features/connectivity-pairing.md) the app should pick
- **Data volume** -- realistic data usage per hour
- **Failure behavior** -- which failure cases from [failure-model.md](failure-model.md) are most relevant here

---

## UC-1: Daily Home / Bedroom-to-Living-Room

**Persona**: B (Alltags-Eltern).

**Pre-conditions**:

- Apps installed on both devices (one-time).
- Pairing done previously, saved connection persists.
- Baby device sits on a shelf in the nursery, **plugged in continuously**.
- Both devices share the home WLAN.

**Setup steps** (per session):

1. Baby device: app is **already running** (Auto-Resume after reboot, or never closed since last evening).
2. Parent device: tap the saved-connection card on the home screen. One tap, reconnect within ~5 s.

**Expected connectivity path**: Local WiFi (Path 1 in [connectivity-pairing.md](../features/connectivity-pairing.md)).

**Data volume**: negligible. Heartbeat ~0.05 MB/h + Opus DTX silence ~0.5 MB/h. Listen-mode bursts ~10-18 MB/h, used rarely.

**Failure behavior priority** (most likely to least):

- F-3 Network lost (router reboot) -- handled by auto-reconnect.
- F-5 OS-kill (battery optimization eroded over months) -- handled by foreground service + battery optimization deep-link; mitigated further by IMP-8 lakmus test.
- F-1 Parent device dead -- handled by indicator-loss; Persona B is trained to notice.
- F-2 Baby device dead -- handled by heartbeat escalation; requires physical check.

**Key UX requirements drawn from this use case**:

- One-tap reconnect from saved connection -- mandatory.
- Persistent foreground notification -- mandatory.
- Auto-resume on baby side after reboot -- mandatory.
- Default settings must "just work" without entering Advanced.

---

## UC-2: Daily Home with Babysitter / Grandparent Visit

**Persona**: B primary, with a transient third device.

**Pre-conditions**:

- UC-1 is set up.
- A trusted but non-resident person (babysitter, grandparent) needs temporary access tonight only.

**Setup steps**:

1. Phase 2: parent device taps "Add device for tonight" -> shows QR with **time-limited Babysitter Pairing** (SUG-4: e.g. 12-hour validity).
2. Babysitter's phone scans the QR -> joins the cluster.
3. After expiry, babysitter's saved connection is automatically invalidated (re-keying behavior to be specified in [connectivity-pairing.md](../features/connectivity-pairing.md)).

**Expected connectivity path**: same as UC-1; babysitter is on the home WLAN.

**Data volume**: same as UC-1.

**Failure behavior priority**:

- F-7 Pairing material compromised (babysitter loses phone after the evening) -- handled by Babysitter-Pairing auto-expiry.
- All UC-1 failure modes still apply.

**Key UX requirements**:

- Babysitter-Mode-Pairing must be distinguishable from regular pairing during the QR step (different visual badge: "12-hour pair").
- The expiry countdown is visible on both the primary parent's and the babysitter's screen.

---

## UC-3: Wedding / Restaurant -- Out-of-Home Event

**Persona**: A (Festtags-Eltern). Most challenging use case in terms of connectivity.

**Pre-conditions**:

- Apps installed on both phones (could be days or months ago).
- At the venue: typically NOT on the same WLAN as a useful network. Hotel WLAN may exist but with captive portal and client isolation. Mobile data is available on both phones.
- Both phones charged that morning, no charger available on the baby-side phone.
- Child sleeps in a hotel room one or two floors up from the reception.

**Setup steps**:

1. Parents arrive at the hotel room. Place the baby-side phone next to the child's cot. Open the app in Baby Mode.
2. App detects this is the first activation in this network. Suggests: "You're at a new location. Set up the connection?"
3. App offers Persona-A path:
   - **Cellular-first option** (recommended for events): "Use mobile data on both devices". Bypasses WLAN-captive-portal hassles. Warns about data volume.
   - **Hotel WLAN option**: "Both devices on this WLAN" -- app first verifies that both devices can actually reach each other via the WLAN (captive-portal check, AP isolation check). If verified, this path is preferred. If not, fall back to Cellular.
4. Pre-flight check (IMP-2): app runs a 60 s probe -- streams a synthetic audio clip from baby to parent, verifies vibration + screen wake + audio + reconnect-after-network-drop. Pre-departure reassurance before parents go down to the reception.
5. Activate Persona-A profile (if not already set): headphone-friendly audio routing (ADR-009 Option c), screen-wake-lock on parent device default ON, in-app data-volume counter visible.

**Expected connectivity path**: Internet (Path 2) -- but specifically the Cellular variant. The signaling server is used to bootstrap WebRTC; if both phones have functional cellular, TURN relay or direct ICE will work. Hotel WLAN is **second** choice, not first.

**Data volume**: realistic worst case 100-300 MB over a 6-hour event (heartbeat + DTX silence + several listen-mode bursts when the child wakes up). Mobile carriers in EU treat this as normal usage; outside the EU, roaming costs apply.

**Failure behavior priority**:

- **Captive-portal failure** (CRIT-2) -- handled by automatic detection + fallback to Cellular.
- **Mobile data exhausted** (CRIT-3) -- handled by data-volume counter + warning; Persona-A profile sets a session-data cap (SUG-6).
- **Phone-call interrupts audio** (CRIT-7) -- handled by ADR-009 routing decisions + post-call auto-resume of listen mode.
- F-1 Parent device dead -- handled as in UC-1, but the parent device is more battery-stressed here.

**Key UX requirements drawn from this use case**:

- Pre-departure pre-flight check (probe-alarm + 60 s synthetic test).
- Headphone-friendly audio routing for Persona A.
- Mobile-data-volume visibility.
- Captive-portal detection and graceful Cellular fallback.

---

## UC-4: Holiday Rental / No-WLAN Scenario

**Persona**: A (Festtags-Eltern) at a holiday cottage, camping pitch, or rural family house where no usable WLAN exists.

**Pre-conditions**: same as UC-3 except no WLAN at all, possibly no reliable mobile data.

**Setup steps**:

1. Open Baby-Mode on the baby phone. Open Parent-Mode on the parent phone.
2. App detects no useful network and suggests **Hotspot Mode**:
   - Step-by-step guided wizard, OS-specific screenshots (Android first; iOS Phase 2).
   - One of the two phones creates a hotspot, the other joins.
   - QR pairing then proceeds normally.
3. **Range warning**: hotspot range is typically 10-30 m. Parents must remain within reach.

**Expected connectivity path**: Hotspot (Path 1 with one device as AP). (Offline topologies target beta, ADR-021 §1.)

**Data volume**: zero -- no internet involved.

**Failure behavior priority**:

- **Range failure** (out of hotspot range when child wakes up) -- handled by heartbeat escalation; parent gets disconnected alarm, walks back closer to the baby room.
- All UC-1 failure modes still apply.

**Key UX requirements**:

- Guided Hotspot wizard with OS-specific screenshots (already DECIDED MVP in [connectivity-pairing.md](../features/connectivity-pairing.md)).
- Range estimation -- not technically measurable, but the app should set expectations via the Hardware-vs-Software FAQ (SUG-5).

---

## UC-5: Multiple Children (Siblings)

> STATUS: IDEA -- not in MVP. Captured to document the gap.

**Persona**: B with two or more children in two separate rooms.

**Pre-conditions**: parents want one parent device to monitor two baby devices.

**Setup steps** (hypothetical):

1. Two baby devices are paired with one parent device.
2. Parent device shows two volume meters (split screen) or a switchable view.
3. Alerts include the child's name.

**Expected connectivity path**: same as UC-1 per baby device.

**Why deferred**: the MVP cap is 1+1 ([connectivity-pairing.md](../features/connectivity-pairing.md)). Phase 2 expands to 1+4 (one baby + four parents), not 2+1. Multi-baby is a structurally different problem (audio mixing, alert prioritization, UI density) and is **Phase 3+**.

**Open questions**:

- How should the parent screen handle simultaneous noise events from two rooms?
- Talk-back (when added) needs target selection -- which baby?
- Battery impact of running two parallel WebRTC sessions on the parent device.

These belong in a future ADR when multi-baby is committed.

---

## Use-Case Coverage Matrix

For each use case, this matrix shows which feature areas are _primarily exercised_. Helps identify which features are critical for which use case.

| Feature area                                                    | UC-1 Daily           | UC-2 Babysitter    | UC-3 Wedding          | UC-4 Holiday                  | UC-5 Siblings             |
| --------------------------------------------------------------- | -------------------- | ------------------ | --------------------- | ----------------------------- | ------------------------- |
| [Audio Monitoring](../features/audio-monitoring.md)             | Primary              | Primary            | Primary               | Primary                       | Primary x2                |
| [Communication](../features/communication.md)                   | Primary              | Primary            | Primary (Cellular!)   | Primary                       | Primary x2                |
| [Notifications and Alerts](../features/notifications-alerts.md) | Primary              | Primary            | Primary (headphones!) | Primary                       | Primary (priority needed) |
| [Connectivity and Pairing](../features/connectivity-pairing.md) | Local                | Local + Babysitter | Internet/Cellular     | Hotspot                       | Local x2                  |
| [Device Management](../features/device-management.md)           | Auto-resume critical | Same               | Battery-critical      | Battery-critical              | Battery x2                |
| [User Interface](../features/user-interface.md)                 | 3-AM single-hand     | Same               | Daytime + headphones  | Daytime, possibly outdoor sun | Multi-baby view           |

## Discarded use-case candidates

| Candidate                                           | Reason                                                                                     |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| "Twins in the same room"                            | Single baby device covers both children acoustically; no separate UC needed.               |
| "Child has a medical monitor that should integrate" | Out of scope per [vision.md Non-Goals](vision.md) -- GuardedSleep is not a medical device. |
| "Remote check-in from work / commute"               | Same connectivity as UC-3; no separate UC.                                                 |
| "Pet monitor reuse"                                 | Out of scope per [vision.md Non-Goals](vision.md).                                         |
