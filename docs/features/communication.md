# Feature: Communication

## Overview

Handles all data transmission between baby and parent units. Two audio transmission modes: (1) automatic alert-triggered audio when baby noise exceeds threshold, and (2) manual listen mode activated by parent. Audio always flows baby -> parent only. Parent -> child communication ("talk-back") is a future feature.

**Mobile data:** Internet / signaling mode may run on **cellular** whenever the OS default route uses mobile data (same user at home or away). The app must surface **approximate data usage** and a **roaming warning** when Internet mode is active on a metered or roaming network ([use-cases.md](../concept/use-cases.md) UC-3, [ADR-009](../decisions/ADR-009-personas-and-failure-model.md) companion).

## Requirements

- `[STATUS: DECIDED]` Noise threshold exceeded = automatic audio transmission baby -> parent + play on parent
- `[STATUS: DECIDED]` Parent "listen mode": parent explicitly activates audio streaming from baby
- `[STATUS: DECIDED]` Audio direction always baby -> parent only (no parent -> child in MVP)
- `[STATUS: DECIDED]` Pre-negotiated muted audio track (silence frames on LAN, see data analysis below)
- `[STATUS: DECIDED]` On mobile data: silence frames acceptable (~18 MB/h), auto-alert audio also works
- `[STATUS: DECIDED]` **MVP:** When the user first enables **Internet / signaling** path on cellular, show a **roaming cost warning** (plain language) if the OS reports roaming -- user must acknowledge.
- `[STATUS: IDEA]` **MVP:** In-app **session data estimate** (rolling counter + reset) for heartbeat + silence frames + listen bursts; helps Persona A avoid bill shock.
- `[STATUS: IDEA]` **Phase 2:** Optional **per-session data ceiling** that pauses non-essential traffic and surfaces a banner (still keeps heartbeat / safety path until explicitly stopped).
- `[STATUS: DEFERRED]` Parent "talk-back": parent speaks through baby unit speaker -- **Phase 2**; **high parent expectation** vs hardware monitors, prioritize after multi-parent failover engineering ([features README](README.md) Phase 2 blurb).
- `[STATUS: IDEA]` Talk-back targets only one child device at a time (future, multi-child)
- `[STATUS: DECIDED]` Lightweight alert messages via DataChannel when noise thresholds exceeded
- `[STATUS: DECIDED]` End-to-end encryption for all transmitted data (DTLS-SRTP + fingerprint binding)
- `[STATUS: DECIDED]` Minimal heartbeat signal to verify connection is alive
- `[STATUS: IDEA]` Audio quality / bitrate / codec settings (configurable by user)
- `[STATUS: IDEA]` Measure and display audio delay (latency) in parent UI

## Sub-Features

### Alert-Triggered Audio (Automatic)

When the baby unit detects noise above the configured threshold (child awake):

1. Baby unit sends lightweight alert via DataChannel (event type, noise level, timestamp)
2. Baby unit **automatically enables audio track** (`track.enabled = true`)
3. Parent device receives alert → **vibration + screen on + app to foreground + volume meter visible + immediately plays child audio** on the route defined by [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md) (not always the loudspeaker)
4. No separate alarm ringtone -- the child's actual audio IS the alert. Parent hears the child directly.
5. Audio plays on ALARM stream (overrides silent mode / DND)
6. Parent hears the child immediately without any manual action
7. Audio stops when noise drops below threshold (configurable cooldown) or parent manually stops
8. Direction: **baby -> parent only** (no audio sent from parent to child)

### Listen Mode (Manual) -- MVP

> STATUS: DECIDED -- Push-to-listen with slide-to-lock, inspired by voice messages in chat apps.

The parent unit has a "listen mode" with two interaction patterns (like voice messages in WhatsApp/Telegram):

**Primary: Push-to-listen (press and hold)**

- Parent presses and holds the listen button -> audio streams immediately
- Release -> audio stops
- Ideal for quick check-ins ("Is the child still quiet?")

**Extended: Slide-to-lock (continuous listen)**

- While holding the listen button, slide upward/sideways to lock
- Audio continues streaming without holding the button
- Tap the button again to stop
- Visual indicator: lock icon appears, button changes state
- Ideal for extended listening (parent is doing something else)

This pattern is intuitive because billions of users know it from messaging apps.

- Audio is streamed with DTLS-SRTP encryption
- Visual indicator on parent unit showing listen mode is active
- Baby unit may optionally indicate that a parent is listening (privacy transparency)
- Direction: **baby -> parent only**

### Pre-Negotiated Audio Track (Technical Detail)

**How it works:**

- At WebRTC connection setup, a MediaStream audio track is added but `track.enabled = false`
- With `track.enabled = false`: WebRTC sends silence/DTX frames
- When activated (alert or listen): `track.enabled = true` -> audio flows immediately (~50-100ms latency)
- When deactivated: `track.enabled = false` -> back to silence frames
- Alternative: `replaceTrack(null)` for true zero-bandwidth mute (but slightly higher latency on enable)

**Silence frame data analysis:**

| Method                               | Data per Hour                  | Annual (24/7) | Notes                                                     |
| ------------------------------------ | ------------------------------ | ------------- | --------------------------------------------------------- |
| `track.enabled = false` (Opus DTX)   | ~0.5-1 kbps = **0.2-0.5 MB/h** | ~2-4 GB/year  | Opus DTX with SID frames every 400ms                      |
| `track.enabled = false` (worst case) | ~40 kbps = **~18 MB/h**        | ~155 GB/year  | Some implementations send more (Kurento measured 40 kbps) |
| `replaceTrack(null)`                 | **0 MB/h**                     | 0             | True zero, but needs renegotiation to re-enable           |

**Recommendation**: Use Opus DTX (Discontinuous Transmission). With DTX enabled, silence frames cost ~0.2-0.5 MB/h, which is negligible even on mobile data. The 40 kbps worst case is from older/specific implementations -- modern Opus with DTX is far lower. We should verify the exact behavior in the PoC.

**Renegotiation latency (if NOT pre-negotiated):**

- On LAN: ~200-500ms (SDP exchange + DTLS)
- On mobile data: ~500-2000ms (higher RTT, possible TURN negotiation)
- On unstable mobile: up to 3-5 seconds

**Decision**: Pre-negotiate with Opus DTX on all connection types (LAN + mobile). The ~0.2-0.5 MB/h cost is acceptable. Verify exact DTX behavior in PoC. If PoC shows higher than expected data usage, fall back to `replaceTrack(null)` on mobile data.

### Talk-Back (Future Feature)

> STATUS: IDEA -- not in MVP.

The parent can speak through the baby unit:

- One-directional: parent -> baby unit speaker
- If multiple child devices exist, parent selects which one to talk to
- Clear visual/audio indicator on baby unit that talk-back is active
- E2E encrypted audio stream
- Requires separate audio track in reverse direction

### Heartbeat and Connection Monitoring

- Minimal periodic signal between units (e.g., every 10-30 seconds)
- If heartbeat is missed for configurable duration, alert the parent
- Low bandwidth usage (just a small packet, not audio data)
- See [notifications-alerts.md](notifications-alerts.md) for connection-lost alerts

### Alert Transmission

When noise thresholds are exceeded on the baby unit:

- Send a lightweight alert message to the parent (not an audio stream)
- Alert includes: event type, noise level, timestamp
- Parent can then choose to activate listen mode for details

## Dependencies

- [Connectivity and Pairing](connectivity-pairing.md) -- underlying network connection and encryption
- [Audio Monitoring](audio-monitoring.md) -- triggers for alert transmission
- [Notifications](notifications-alerts.md) -- delivery of alerts on the parent device

## Open Questions

- ~~Push-to-listen (hold button) vs. toggle (tap on/off) -- which is better UX?~~ **RESOLVED**: Both -- push-to-listen + slide-to-lock (chat app pattern)
- What audio codec provides the best quality-to-bandwidth ratio for voice?
- Should talk-back be full-duplex (simultaneous listen + talk) or half-duplex?
- What is the optimal heartbeat interval (balance between responsiveness and data usage)?

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
