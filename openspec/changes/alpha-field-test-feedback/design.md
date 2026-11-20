# Design

## Context

See proposal.md, Why. The findings below were read from the code at `f42654c0`.

- **Noise gate** (`crates/guardedsleep-core/src/noise.rs`):
  - The gate is an absolute linear-RMS threshold with a hold time. `above_since` resets on any sample below the threshold, and the gate turns quiet at once.
  - The platform feeds one sample per 100 ms (`AUDIO_LEVEL_MS`), the RMS of the libwebrtc record buffers (`CaptureTap` in the ADM record callback).
  - Thresholds are limited to 0.01–1.0 (`link.rs`). The UI slider is linear from 0.03 to 0.4; the default is 0.12 (−18 dBFS).
  - No calibration exists anywhere.
- **Baby capture:** the baby ADM sets no audio source and does not turn off hardware AEC/NS, so libwebrtc's default `VOICE_COMMUNICATION` source and the device's voice-call processing apply. The device check samples with `MIC`, which is a different path.
- **Talk-back playout:**
  - The baby ADM is built with `USAGE_ALARM` / `CONTENT_TYPE_SPEECH`, speaker-muted outside a grant.
  - `TalkPlayout` sets `AlarmVolume.prepare(level)`, then ramps `AudioTrack.setVolume(attenuation · fade · limiter)`.
  - LOW is 0.25 of the alarm maximum. If the user's alarm index is above the target, the stream is left alone and the track is attenuated by the linear index ratio.
  - The limiter (`CEILING_RMS = 0.30`) reads the stats `audioLevel`, which libwebrtc probably measures after the output gain and as a peak-like value, so it reacts to its own gain.
  - `LinkController` calls `setSpeakerOnly(false)` on the baby, which sets `isSpeakerphoneOn = false`.
  - Emulators have no acoustic path; RTP arrival is proven (AC-11), audible playout never was.
- **Parent alarm screen:** `AlertScreens.kt` `LinkAlarmContent`, variant `STOPPED`: "OK" calls `onClose()` only. `StopNotice` is sent only by a deliberate `stop` (`link.rs`), so `PEER_STOPPED` already means "stopped on purpose".
- **Test alert:**
  - `ParentModel` sets `noiseIsTest` and `lastTestWallMs`, but the tracker still counts the probe as a noise alert and later sets `lastSound`.
  - `AlertScreens` shows the noise template with a small TEST badge.
  - The monitor hero shows "Sound detected" without a marker.
- **Battery:** the status bar shows the level, with a bolt when charging. Separately, the `BABY_UNPLUGGED` banner shows while `unit_warnings` contains `not_charging`, which the baby derives from `EXTRA_PLUGGED == 0`, not from a change. The one-time `ChargerUnplugged` event (true → false) already reaches `AlertController`.
- **Permissions:** `Entry.kt` shows `RoleScreen` first, then `SetupScreen(onboarding = true)` per role. The camera is asked on the scan screen (`PairParentScreen`); the parent microphone is asked in `TalkPermissionSheet`.
- **QR:** `PairBabyScreen` calls `WindowBrightness(if (bright) 0.9f else 0.4f)`; the 0.4 override is brighter than auto brightness in a dark room.

## Goals / Non-Goals

**Goals:**

- Fix what the field test found with as little new surface as possible.
- Keep every safety decision in the core, and make the gate rule testable in vectors and simulation.

**Non-Goals:**

- An adaptive noise floor.
- Web upfront permissions.
- Bluetooth routing policy beyond "a connected headset may be used".
- Any change to the parent's noise-event rules (2-minute join and quiet stay as they are).

## Decisions

### 1. Noise gate: tolerate pauses (sliding window)

Rule (core, `noise.rs`):

- Each sample covers `min(now − prev, 1000 ms)` (the first sample covers one nominal interval).
- The gate keeps a ring buffer of `(covered_ms, loud)` for the last `hold` ms; at most 150 entries at a 100 ms cadence.
- A noisy stretch starts at the first loud sample (`stretch_since`) and ends after 2 s without a loud sample.
- It goes loud on a sample at or above the threshold when `now − stretch_since ≥ hold` and the loud coverage in the window is at least `hold / 2`. The first condition keeps the user's hold time meaning the full time (review round 2, N1); the second makes only the last `hold` count.
- A loud gate turns quiet when no sample reached the threshold for 2 s (`last_loud`).
- `restart_hold` clears the buffer and sets `last_loud = now`, so a loud gate is not released at once.
- `Loud` and the link-up noise alert carry the triggering sample's rms, which is always at or above the threshold.

Constants live in the core policy/config with invariants (release ≥ 1 s, release < minimum hold).

Each heartbeat from the baby carries `loud: bool` (`gate.is_loud()`), an optional field defaulting to false. The parent core's keep-alive counts a heartbeat as loud when `loud || rms ≥ threshold` (review C3).

- **Option A, a sliding half-window plus release (recommended).**
  - Pros: matches speech and crying, which come in bursts; a single slam fails; only the last `hold` counts, so old background noise cannot delay an alarm; deterministic and easy to vector.
  - Cons: a ring buffer; dense knocking that covers more than half the window fires (it is sustained noise).
- **Option B, smoothing the level (EMA) before the threshold.**
  - Pros: tiny code.
  - Cons: blurs onsets and delays the alarm; the effective sensitivity depends on the time constant.
- **Option C, cumulative counting from the span start with a 1 s pause bridge** (the first draft).
  - Pros: no buffer.
  - Cons: long background noise with short gaps keeps the span open and dilutes the ratio, delaying a real cry by tens of seconds (review C2); a 250 ms interval cap starved sparse browser samples (review I1).
- **Recommendation:** A.

### 2. Raw capture (moved from `quiet-link-no-rtp` 2.3)

Baby ADM:

- `setAudioSource(UNPROCESSED)` when `AudioManager.getProperty(PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED) == "true"`, else `MIC`.
- `setUseHardwareAcousticEchoCanceler(false)` and `setUseHardwareNoiseSuppressor(false)`.
- The baby's `createAudioSource` constraints turn off the software echo canceller, noise suppressor and gain control too, so the parent hears the room as it is.

The device check keeps `MIC`, now the same family of source. Half duplex plus the talk pause in the gate covers the loss of echo cancelling during a talk. The raw-capture flag the playback check waits for (`add-talk-back` "Playback check") is reported as `true` once this lands.

### 3. Sensitivity in decibels

The core keeps a linear threshold. Its lower limit moves from 0.01 to 0.0015 (−56 dBFS) through **one shared constant** used by the `DeviceStatus` parser (`protocol.rs`), the config clamp (`link.rs`), the conformance and negative vectors, and the Android and web clamps. Otherwise a status carrying a quieter threshold would be rejected whole, and the parent would lose battery and microphone state (review C1). The UI maps the slider position `p ∈ [0,1]` to `dB = −55 + 40·(1−p)`, so the left end is the most sensitive, and stores `10^(dB/20)`.

- Android: `SettingsScreen`, with the live preview drawn in the same dB scale.
- Web: `sensitivity.ts`.

The provisional default is −36 dBFS (0.016). White-noise machines and fans often sit at −40 to −35 dBFS, so the device check warns when the room level is within 6 dB of the threshold (review I10). The default is tuned in task 7.1 on the reference phone with raw capture. A stored threshold outside the new range is clamped. The app was never released, so no migration is needed.

- **Option A, a dB slider over a wider range (recommended).**
  - Pros: even feel; reaches quiet speech.
  - Cons: the default needs measurement.
- **Option B, keep the linear slider and lower the default.**
  - Pros: no UI change.
  - Cons: the sensitive end stays cramped, and −30 dBFS is still the floor.
- **Recommendation:** A.

### 4. Talk-back audible

1. **Routing.**
   - The baby no longer calls `setSpeakerOnly(false)`; it applies to the parent only.
   - The app never enters `MODE_IN_COMMUNICATION`, so `setSpeakerphoneOn` does not move a `USAGE_ALARM` stream (review I6) and is not used as a remedy.
   - During a grant the platform logs the routed output of the playback (`getActivePlaybackConfigurations`, API-guarded).
   - If the spike in task 7.2 finds the earpiece, the remedy (a preferred output device on the ADM track) is a follow-up inside this change, decided by the planner with the measurement.
2. **Volume.**
   - The core `TalkLevel` enum becomes a percent step (10–70, step 10, default 50) in the core, the bindings, the contract, settings and web models, with `just api-snapshot` and re-blessed vectors (review I4). The parent never sends it.
   - The 70 % top keeps the S16 decision on infant hearing (review I5).
   - `AlarmVolume.prepare` sets the alarm stream to the step nearest `max · pct / 100`, clamped to `[1, round(0.7 · max)]`. A residual track gain (≤ 1.0) makes up the difference between the requested percent and the step, so neighbouring percents still differ.
   - The stream is restored after the talk under the existing "user did not touch it" rule. The linear ratio attenuation is removed.
3. **Cap.**
   - The talk line stays sink-free (the earlier privacy decision and `TalkPlayoutTest` stay).
   - The limiter judges the pre-gain level as `audioLevel / applied_gain` (when the gain is above 0.05) if the spike shows the stats `audioLevel` is post-gain, or `audioLevel` directly if it is pre-gain.
   - The same pre-gain level feeds the core's `TalkLevel` input (`LinkRunner`), not only the limiter.
   - Unit tests cover the math for both cases: normal speech keeps the gain, and a full-scale jump lowers it within 200 ms.
4. **Diagnostics, end to end** (PocLog, no PII):
   - Parent: outbound `audioLevel` and packets sent.
   - Baby: `totalSamplesReceived`, `concealedSamples`, ADM playout started, `setSpeakerMute(false)` applied, routed device type, alarm index before and target, track gain per second, pre-gain level.

- **Option A, fix volume and cap, log the route end to end, then verify on a device (recommended).**
  - Pros: fixes what is certain from the code and measures the rest.
  - Cons: the route remedy may need a second pass.
- **Option B, diagnostics only first, then fix.**
  - Pros: certainty.
  - Cons: needs another field round before anything is heard.
- **Recommendation:** A, with task 7.2 as a release gate.

### 5. Permissions on first start (Android)

- A new `PermissionsScreen` in `app/ui/screens`, shown by `Entry.kt` while `!app.permissionsIntroDone` (a new persisted `AppState` flag, set by `controller.completePermissionsIntro()`), before `RoleScreen`/Home.
- Rows reuse the `SetupScreen` row composable and `SystemSettings` helpers: microphone, camera, notifications (API 33+), full-screen intent (API 34+), notification-policy access, battery optimisation.
- "Allow all" runs one `RequestMultiplePermissions` for the runtime permissions, then opens each special setting in turn on return.
- The post-role `SetupScreen(onboarding = true)` step is removed; `onboardingDone` is set together with the new flag. The battery safety advice (`device-setup` "Battery safety copy") moves to the baby pairing screen, so baby setup still shows it.
- The existing lazy prompts (scan screen, `TalkPermissionSheet`, baby start) stay as fallbacks for a missing permission.

- **Option A, one screen before the role, both roles (recommended).**
  - Pros: matches the owner's request; one moment of setup.
  - Cons: a parent-only user is also asked for silence access, and a baby-only user for the camera. Mitigated by "Later" on every row.
- **Option B, after the role, but everything for that role including camera and microphone.**
  - Pros: fewer rows.
  - Cons: switching roles later triggers a second round, which is exactly the complaint.
- **Recommendation:** A.

### 6. Smaller items

- **Stopped OK:**
  - A new core input `AcknowledgePeerStopped` stops the parent only while the link is still `peer_stopped`; otherwise it is ignored (review I8). The authentication of the stop notice is unchanged (`link.rs` stop handling).
  - `LinkAlarmContent` variant `STOPPED` gets a primary button labelled "OK, stop here too" that sends it via `controller.acknowledgePeerStopped()` and closes the screen. Web `LinkAlarmScreen` does the same for `baby_stopped`.
  - No notification action maps to it; back or dismiss keeps monitoring. The `LOST` path is untouched.
- **Test alert:**
  - `ParentModel` (Android and web) no longer feeds probes into the sound tracker or `lastSound`.
  - The full-screen noise template gets a test variant: label "Test", headline "Test alert received", body naming the style and "The baby device stayed silent", "OK" opening the existing heard-and-felt question.
  - The notification keeps the `noiseTest*` copy.
- **Battery:** remove `BannerKind.BABY_UNPLUGGED` (Android `Banners.kt`, web `BannerStack`). Keep the `ChargerUnplugged` notification and the low and critical banners. `not_charging` stays in `unit_warnings` for the device check.
- **Device check copy:**
  - `DeviceCheckRow` shows one summary line: "Connection over the internet: works", "works (2 of 3 servers)" or "doesn't work".
  - An expandable "Details" lists `host: reachable / not reachable`.
  - The main-line keys are new; the host keys move into the details.
- **QR:** `WindowBrightness(if (bright) 0.9f else null)`. By default, no override, so the system or auto brightness applies; the dim grey modules stay.
- **Coach copy:** `native.ui.talk.coach` and `webScreens.talk.coach` become "Hold to talk to your child." in all locales.

### Privacy and layers

There is no effect on the §7 guardrails: no new host, permission, storage or log content (the diagnostics carry levels and device types only).

| Part                                                     | Layer    |
| -------------------------------------------------------- | -------- |
| Gate rule, threshold limits                              | Core     |
| Capture, routing, volume, cap measurement                | Platform |
| Permissions screen, copy, alert variants, slider mapping | UI/State |

## Discarded alternatives

- **Startup calibration or an adaptive noise floor:** the owner suspected one existed; it does not, and the field problem is explained without it. A floor measured in a loud room would make the baby device _less_ sensitive, which is the failure the owner feared.
- **Raising the window brightness to 1.0 for scanning:** dazzles in a dark nursery; scanners read the dim inverted code, and "Make code brighter" stays for hard cases.
- **Removing the unplug signal entirely:** the core requirement to tell the parent once when the charger is pulled is a safety warning; only the permanent banner goes.
- **Keeping low/medium/high and raising the default to medium:** "medium" was a fraction of the stream maximum further reduced by attenuation, so the owner could not tell what it meant. A percent slider is what phone users know.

## Risks / Trade-offs

- [A more sensitive default and the half-window rule raise false alarms (second-worst failure)] → tune the default on the reference phone (task 7.1); simulation scenarios for TV and talking at the dining table vs a crying recording; the hold time stays user-adjustable.
- [Raw capture levels differ widely between devices] → the dB slider has 40 dB of range; the device check shows the live level.
- [Software AGC off makes Listen audio quieter] → accepted for an honest room sound (the requirement taken from `quiet-link-no-rtp`); the parent's output gain compensates.
- [Without AEC on the baby, the parent's voice echoes back during a talk] → half duplex; the baby track is muted during playout (existing "No speech into the open microphone").
- [The stats `audioLevel` may be post-gain] → the limiter divides by the applied gain behind one switch set from the spike; unit tests cover both positions.
- [Upfront permissions feel heavy] → one sentence per row, "Later" everywhere, no blocking.
- [Overlapping active changes] → the MODIFIED blocks are based on the newest text in `alpha-connection-model` (device check, invite), `redesign-calm-ui` (stopping, pairing entry, test alert) and `add-talk-back` (checklist, talk-back), and this change is archived after those three. `quiet-link-no-rtp` hands over task 2.3 and its "Raw microphone capture" requirement, which moves into this change. The `redesign-calm-ui` banner list drops "baby unplugged". If option B of `quiet-link-no-rtp` 2.1 (a separate `AudioRecord`) is chosen later, the raw-source choice moves with it.

## Migration Plan

The app was never released, so stored settings are reset or clamped. Archive order: `alpha-connection-model`, `redesign-calm-ui` and `add-talk-back`, then this change. The heartbeat `loud` field is optional with a default, so mixed builds during development still parse.

## Open Questions

- The final default threshold (task 7.1).
- Whether the speaker re-route is needed on real devices (task 7.2).

## Review history

**Critical Review, round 1 (2026-11-20): 5/10, rework needed.** Each `CRITICAL` and `IMPORTANT` finding was answered in the artifacts:

| Finding                                              | Resolution                                                                                                                                 |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| C1: threshold below 0.01 rejected on the wire        | One shared limit constant in parser, config, vectors and clients (§3, task 2.1); spec scenario "Most sensitive setting reaches the parent" |
| C2: cumulative ratio delays alarms                   | Sliding window over the last hold time (§1); spec scenario "Crying after a long noisy background"; proptest in task 2.1                    |
| C3: sticky gate hidden from the heartbeat keep-alive | Heartbeat carries `loud` (§1); `link-supervision` "Noise event lifecycle in the core" modified; sim scenario in task 2.2                   |
| I1: 250 ms cap starves sparse samples                | Cap at 1 s; scenario "Sparse samples"                                                                                                      |
| I2: deltas conflict with active changes              | Blocks rebased on the newest text; archive order (Migration Plan)                                                                          |
| I3: "Parent pairing entry" contradictory             | Modified in `device-setup`                                                                                                                 |
| I4: talk volume touches the core                     | Task 5.2 covers core, bindings, snapshots and vectors                                                                                      |
| I5: 70 % cap and the sink-free talk line overturned  | Both kept (§4)                                                                                                                             |
| I6: speakerphone re-route ineffective                | Dropped; end-to-end diagnostics; remedy after measurement (§4)                                                                             |
| I7: test must use the real alert path                | SHALL and scenario added to "Test alert"                                                                                                   |
| I8: OK can act on a stale notice                     | Core `AcknowledgePeerStopped` guarded by `peer_stopped`; new `link-supervision` requirement                                                |
| I9: repeated denials, web parity                     | `device-setup` "No later permission prompts"; web scenarios                                                                                |
| I10: white noise near the threshold                  | Device-check warning (spec) and sim scenarios (task 2.2)                                                                                   |

**Round 2 (2026-11-20): 8/10, go once N1 is fixed.** N1 (continuous sound fired after half the hold time) fixed with the stretch condition in §1, the spec and task 2.1. N2 (the talk pause keeps `loud`) is in task 2.2. N3 (leftover text) is fixed.

The round-1 minor findings are folded in: limit value 0.0015, `Loud` rms ≥ threshold, `restart_hold` sets `last_loud`, residual gain between volume steps, a critical-key check in all locales, the QR wording, and the AGC trade-off.
