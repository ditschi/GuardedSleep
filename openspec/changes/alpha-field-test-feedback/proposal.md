# Proposal

## Why

The owner's first field test on two real phones (feedback of 2026-11-20) found problems that a parent at 3 AM would hit on the first night:

- **Talk-back was never heard on the baby device**, although the parent showed "Speak now" (owner: about 90 % sure). Talk-back reached the speaker only in theory: task 2.1 of `add-talk-back` (real-device playout) is still open. The default volume is "low" (25 % of the alarm stream), then the volume is attenuated again and a limiter that probably measures its own output pushes the gain down further. The baby also sets `isSpeakerphoneOn = false`, so earpiece routing is not ruled out either.
- **The baby device was nearly deaf.** A conversation at the dining table never fired a noise alert even at a low threshold. The cause is not calibration (none exists): the core gate restarts the hold time on any single 100 ms window below the threshold, so speech with its natural pauses never stays loud for 5 s. On top of that, capture uses the voice-call source with the device's noise suppressor, and the slider is linear in RMS, so most of its travel covers loud levels (default −18 dBFS).
- **Permissions arrive in pieces.** The checklist comes only after the role choice, the camera only on the scan screen and the parent microphone only at the first talk. A parent who has "allowed everything" is asked again later, at the worst moment possible when the first talk is meant to soothe a crying child.
- **Copy and presentation confuse:**
  - The device check prints "gs.mydata.rocks: reachable".
  - A test alert looks like a real "The child is making sound" alert and is later shown as "Sound at …".
  - Battery appears twice: the level in the bar, plus a "Baby device is no longer charging" banner that also appears when the device never charged.
  - Talk-back says "New:" in an app that was never released.
- **Small behaviour bugs:**
  - "OK" on the parent's "baby device stopped monitoring" notice leaves the parent monitoring a stopped baby.
  - The QR screen forces 40 % brightness, which is brighter than auto brightness in a dark room and dazzles.

This change alters prior decisions:

- The "permissions only when needed" rule (`device-setup`, `pairing`, `add-talk-back`).
- The noise gate's quiet rule (`audio-monitoring`).
- The talk-back volume model (`add-talk-back` "Talk-back settings": low/medium/high).

The Critical Reviewer applies (AGENTS.md §6).

## What Changes

- **All permissions on first start**, before the role choice: one screen lists the microphone (noise detection on the baby device, talking to the child on the parent device), the camera (scanning the code), notifications, full-screen alerts, Do Not Disturb access and background operation, each with one plain sentence on why. Android only. The per-role checklist stays in Settings. A feature asks again only if its permission was refused or revoked.
- **Device check in plain words:** "Connection over the internet: works / doesn't work" instead of host names; the host stays visible in an expandable detail line (§7.1 disclosure).
- **Test alert as a calm confirmation:** sound and vibration of the active alert style still run, but the screen says "Test alert received", not "The child is making sound". The test does not count as a sound: no "Sound detected", no "Sound at …", and no noise event.
- **One battery indicator** on the parent: "82 %" with a charging mark only while charging. The persistent "no longer charging" banner goes; the one-time notice when a charging baby device is unplugged and the low and critical warnings stay.
- **Talk-back is heard on the baby device:**
  - Playout is forced to the built-in loudspeaker during a grant, never the earpiece.
  - The volume becomes a slider (percent of the alarm stream, default 50 %, top end 70 % as decided before) instead of low/medium/high.
  - The loudness cap judges the incoming level before its own gain, without adding an audio sink.
  - Diagnostics log the routed output and the gain chain.
  - Diagnostics follow the path end to end; real-device verification (`add-talk-back` 2.1 a–c) moves into this change as a release gate.
- **Noise gate tolerates speech pauses:** the gate goes loud when at least half of the last hold time was at or above the threshold (short pauses no longer restart it) and turns quiet after 2 s below the threshold; heartbeats say whether the gate is loud, so the parent sees ongoing crying in bursts.
- **Raw capture on the baby device** (moved here from `quiet-link-no-rtp` task 2.3): `UNPROCESSED` where supported, else `MIC`, hardware noise suppressor and echo canceller off.
- **Sensitivity slider in decibels** (even steps from about −55 to −15 dBFS) with a new, more sensitive default to be tuned on a real device. The lower threshold limit drops from 0.01 to 0.0015 everywhere it is checked, including the wire parser.
- **The primary button on "baby device stopped monitoring" also stops monitoring on the parent**, through a core input that acts only while the link is still in the "stopped" state. The unplanned case (link loss, empty battery, crash) is unchanged: there is no stop notice, the "No connection" alarm runs and "I'm checking" stays.
- **QR screen never brightens the display:** it may dim to at most 50 % but never raises the brightness above the current level; "Make code brighter" stays.
- **Copy:** drop "New:" from the talk-back hint; new and changed keys in all 20 locales (`machineTranslated`).

**Out of scope:**

- Microphone calibration or an adaptive noise floor (the field test did not need it; backlog `IDEA` if measurements show a need).
- Upfront permissions in the web client (browsers prompt per use, and requesting the camera starts it).
- The rest of `quiet-link-no-rtp`.
- Talk-back behaviour beyond routing, volume and the cap.
- Any change to escalation timing.

**Tier:** T0 (alpha).

**Persona impact:**

- Serves all three personas. B (daily home) and A (event use) get a baby device that hears normal crying and talking, and a talk button that works on the first night.
- C (old phones) gains most from raw capture, since old voice-call processing is the most aggressive.
- Trade-off: a more sensitive default means more false alarms in noisy rooms (the second-worst failure). It is mitigated by the half-window rule and a default tuned on a real device.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `device-setup`: first-start permission screen before the role choice; the checklist no longer requests on demand; plain device-check copy; acknowledging a baby-stopped notice ends parent monitoring.
- `pairing`: camera permission comes from the first-start screen; the QR code never raises screen brightness.
- `audio-monitoring`: gate tolerates short pauses and turns quiet after a release time; raw capture; decibel slider with a wider range.
- `parent-alerts`: test alert presented as a confirmation, not a sound; battery shown as one indicator; the unplug notice only after real charging.
- `link-supervision`: the heartbeat states whether the gate is loud and counts for the parent's noise event; a guarded acknowledgment of a deliberate baby stop.
- `talk-back` (still in `add-talk-back`): microphone permission from the first-start screen; loudspeaker routing; volume slider; the loudness cap measures pre-gain. Archive order: after `alpha-connection-model`, `redesign-calm-ui` and `add-talk-back`, whose newest texts the deltas build on.

## Impact

- **Core (Rust):**
  - `noise.rs` (gate rules), `link.rs` and `protocol.rs` (threshold limit, heartbeat `loud`, `AcknowledgePeerStopped`), `talk.rs` (volume as a percent step).
  - The talk loudness-cap input, if computed in the core.
  - Vectors and traces re-blessed. `guardedsleep-sim` scenarios for speech with pauses.
- **Android:**
  - `app`: a new first-start permission screen, `Entry.kt` order, `SetupScreen`, `PairBabyScreen` brightness, `AlertScreens` (test alert, stopped OK), `StatusBar`/`Banners`, `SettingsScreen` sliders, `TalkWidgets`.
  - `ui-contract`: `TalkVolume` becomes a percentage; a test-alert model flag.
  - `platform`: `LibWebRtcMediaEngine` (raw capture, speaker routing), `TalkPlayout`, `AlarmVolume`, `DeviceChecker`, `ParentModel` (probe not recorded as a sound), `SettingsRepository`.
- **Web:** test alert screen, battery indicator, stopped OK, coach copy, decibel slider (`sensitivity.ts`), mirrored contract models.
- **Copy:** `locales/*.json` (critical keys for the test alert and stopped notice).
- **Other changes:**
  - `quiet-link-no-rtp` task 2.3 and its "Raw microphone capture" requirement move here.
  - `add-talk-back` task 2.1 (a–c) is fulfilled by this change's device check.
  - `redesign-calm-ui` banner list loses "baby unplugged".
- **No new dependency, no new permission, no effect on the privacy guardrails.**
