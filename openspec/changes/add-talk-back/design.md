# Design

Roles applied: Product Owner, Technical Architect, Security/Privacy Architect, UX; Critical Reviewer findings and answers in §12. Owner decisions in §11.

## Context

See proposal.md for the motivation. State on `origin/main` (2026-11-11, after `alternate-roles-and-apk-size`):

- **Core.** `Link` (`crates/guardedsleep-core/src/link.rs`) owns listen intent and the microphone decision: the baby's `apply_mic` emits `SetMicEnabled` when `(parent_listen || noise_streaming || probing) && live_verified()`. "Trusted" is `attempt.is_verified() && proof_due.is_none()` (pin proof done). The noise gate (`noise.rs`) is fed by `LinkInput::AudioLevel` (about 10 Hz) and has no pause input. The baby refuses a local SDP unless every audio m-line is `sendonly` (`attempt.rs` `audio_is_sendonly`, failure key `baby_sdp_not_sendonly`); the parent's SDP is not checked. DataChannel protocol v3 (`protocol.rs` `DataMessage`) ignores unknown `type`s and unknown fields, so new messages are additive. `QuietRequest` shows the rate-limit pattern (`last_quiet_at`, `QUIET_RATE_MS`).
- **Android.** `LibWebRtcMediaEngine` builds one `JavaAudioDeviceModule` with playout `USAGE_ALARM`; the baby mutes the ADM speaker (`setSpeakerMute(true)`) and adds one `SEND_ONLY` transceiver; the parent adds no transceiver and has no microphone track. The baby's noise level comes from the ADM record callback (`CaptureTap`). The baby's Do Not Disturb rule (`SilenceController`) allows alarms only (`allowAlarms(true)`, `allowMedia(false)`). The parent never asks for `RECORD_AUDIO`; `MonitorService` runs as `mediaPlayback` on the parent and `microphone` on the baby. The 60-minute listen lock limit lives in the platform models.
- **Web.** Both roles; the baby adds a `sendonly` transceiver; the parent has no `getUserMedia`.
- **Active changes:** `quiet-link-no-rtp` (sender detach while muted, raw capture without AEC/NS/AGC on the baby, Opus profile from the core, capture spike), `multi-parent-pairings` (PR ditschi/GuardedSleep#54: `BabyHub` with up to 2 parent links, one shared noise gate, `set_mic { link }`), `add-offline-lan` (media path unchanged).

## Goals / Non-Goals

**Goals:** a parent can say a short sentence to the child with a hardware-monitor-like push-to-talk; the decision when talk audio may play lives in the core; the baby unit stays silent in every other situation; talk-back never raises a false noise alert or escalation; it fits one or two parents.

**Non-Goals:** full duplex calls, video, talking to several baby units, pre-recorded or scheduled messages, remote volume control, echo cancellation on the baby's monitoring capture, a web baby unit that plays talk audio (§8, owner decision pending).

## Decisions

### 1. Duplex mode

- **Option A — half duplex push-to-talk (recommended).** Hold to talk, release to stop; the parent mutes the baby's audio while holding.
  - Pros: matches hardware monitors and walkie-talkies; no acoustic echo loop without echo cancellation (the talking parent does not play the room, so its own voice coming back from the baby's speaker is never heard); the baby keeps raw capture (`quiet-link-no-rtp` §4); an open speaker next to the child is impossible by construction (no hold, no sound); short, bounded noise-gate pauses.
  - Cons: the parent cannot hear the child's reaction while speaking (only after release); a hold gesture is harder for some motor impairments (TalkBack double-tap-and-hold works).
- **Option B — full duplex (a call).**
  - Pros: natural conversation; hears the reaction at once.
  - Cons: needs echo cancellation on both devices, which contradicts raw capture for the noise gate; the noise gate is blind for the whole call; an open speaker channel next to the child for minutes; much harder audio routing on Android (communication mode, earpiece/speaker switching).
- **Option C — half duplex, tap to start and tap to stop with an auto-timeout.**
  - Pros: no holding.
  - Cons: a forgotten open channel (the same failure the 60-minute listen limit exists for, but next to the child and audible); accidental taps make sound in the baby room.

Recommendation: A. B and C are kept under Discarded alternatives once the owner confirms.

### 2. Media: a second, pre-negotiated audio m-line

- **Option A — two transceivers (recommended).** The baby's offer carries the monitoring line (`sendonly`, unchanged) and a talk line (`recvonly`). The parent answers `recvonly` / `sendonly`. The parent's talk sender carries no track outside a talk, so it sends no RTP (with `quiet-link-no-rtp` the same detach rule applies to it).
  - Pros: no renegotiation per press (no signaling round trip, works while a mailbox is down); the core can check each direction separately and keep the strong rule "the baby never sends on a receiving line and never receives on the monitoring line"; the Opus profile can differ per line; the monitoring line and its mute/detach logic stay untouched.
  - Cons: two transceivers per attempt; an older peer with one m-line is not compatible (no installed base, acceptable).
- **Option B — one `sendrecv` line.**
  - Pros: one transceiver.
  - Cons: loses the SDP-level guarantee that the baby's monitoring line never receives; playout gating would be the only barrier; the monitoring mute and the talk sender share one transceiver.
- **Option C — renegotiate a talk line on every press.**
  - Pros: no receiving line exists outside a talk.
  - Cons: a signaling round trip per press (seconds over the project server, impossible while mailboxes are closed on a verified link); renegotiation is a new failure path on a verified attempt, which the link rules forbid touching.

Recommendation: A. The SDP rule in the core changes from "every audio m-line `sendonly`" to "exactly two audio m-lines; baby: `sendonly`, `recvonly`; parent: `recvonly`, `sendonly`|`inactive`" (spec "Sans-IO interface contract"). Order identifies the lines (monitoring first). A parent that answers the talk line `inactive` or rejects it (port 0) cannot talk on that attempt; the baby reports `talk: unsupported` to it.

Playout on the baby is gated twice, below the core's decision: the remote talk track is disabled and the ADM speaker muted outside a granted talk (Android); on the parent, no microphone track exists outside a talk.

### 3. Core: talk state and protocol (additive v3)

Talk state lives in the core `Link` (and, once `multi-parent-pairings` lands, its arbitration in `BabyHub`, §6). Platforms only do I/O.

**Parent side.** Input `set_talk { pressed }`. On press, if the link is trusted and the talk line is usable, the core sends `talk_request { id }` (id: per-run counter) and emits `talk_changed { state: requesting }`. On `talk_grant { id, max_ms }` for the current id, it emits `set_talk_mic { enabled: true }` and `talk_changed { state: talking, until_ms }`, plus `set_playback_ducked { ducked: true }` from the press on (half duplex). On release, deadline (`max_ms`), `talk_deny`, attempt loss or stop, it sends `talk_end { id }` (if requested), emits `set_talk_mic { enabled: false }`, `set_playback_ducked { ducked: false }` and `talk_changed { state: idle | denied { reason } }`. The parent's own deadline is a backstop; the baby's is binding.

**Baby side.** On `talk_request { id }` from a trusted attempt (the `proof_due` gate already blocks it before the pin proof): deny with `talk_deny { id, reason }` when `reason ∈ { off, unsupported, busy, rate }`; else emit `play_talk_chime { level }`, pause the noise gate (§5) and remember `talk = { id, granted_at: None }`. On the platform's `talk_chime_done` (or 1.5 s after the chime action, whichever comes first, so a failed chime never blocks) the core emits `set_talk_playback { enabled: true, level, fade_in_ms: 800 }`, sends `talk_grant { id, max_ms: 30000 }` and sets the deadline. It ends the talk on `talk_end { id }` (after a 300 ms drain so the jitter buffer can play out the last syllable), at the deadline, on attempt loss, on stop, on release of the link (multi-parent) and when talk-back is switched off: `set_talk_playback { enabled: false }`, `talk_end` to the parent if the baby ended it, and the noise pause continues for the 2 s tail. At most one request per link per second (`TALK_RATE_MS = 1000`), else `rate`.

**Status.** The baby's `device_status` gains `talk: on | off | unsupported` and `talker: none | you | other` (additive, omitted when unknown), sent when they change, so the parent can render the button before a press and show "Other parent device is talking". The baby emits `talk_active { active }` for its own indicator, and parents emit `noise_paused { paused }` from `talker != none` plus the tail flag in the status.

**Constants** (core, covered by configuration tests): `TALK_MAX_MS = 30_000`, `TALK_RATE_MS = 1_000`, `TALK_CHIME_TIMEOUT_MS = 1_500`, `TALK_DRAIN_MS = 300`, `TALK_NOISE_TAIL_MS = 2_000`.

Vectors: `parse_data_v3` cases for the four messages and the two status fields (valid, missing id, unknown reason, extra field); `link` vectors for grant, deny, deadline and loss; new sim traces. Envelope, invite and proofs are unchanged.

### 4. Echo, capture and audio processing

- The baby keeps raw capture without echo cancellation (`quiet-link-no-rtp` §4). Its reason changes from "the baby only sends" to "half duplex: the talking parent does not play the room, so there is no echo loop"; that change's design gets the corrected sentence (task 6.2).
- While a parent talks, the baby's room audio still reaches a listening other parent, including the talking parent's voice from the speaker. That is intended (it is what is happening in the room) and needs no cancellation.
- The parent captures with voice processing on (echo cancellation, noise suppression, auto gain), because its microphone is a voice microphone: auto gain evens out loudness, so a parent who shouts does not startle the child. Android: a parent-only `AudioSource` with processing constraints; web: `getUserMedia` with the three flags on.
- The baby's level samples during playback are ignored by the core (§5); the platform does not try to subtract the playout.

### 5. Noise gate during and after playback

- **Option A — pause the gate in the core from the chime until 2 s after playback (recommended).** The core drops `AudioLevel` samples for the gate (and the `mic_silent` timer) while `talk` or the tail is active; a running noise event continues, no event starts or ends, and a fresh hold time starts afterwards.
  - Pros: simple, deterministic, testable in the sim; no false alert ever; bounded by `TALK_MAX_MS + TALK_NOISE_TAIL_MS` per press.
  - Cons: a cry that starts during a talk is detected up to about 32 s + hold time later. Mitigations: the talking parent is attending at that moment; the other parent sees "Noise detection paused"; the rate limit allows a new press only after the previous ended, so pauses do not chain silently (each press is visible on both parents).
- **Option B — compensate: raise the threshold by the playout level.** Pros: detection continues. Cons: speaker-to-microphone coupling differs per device and position; under-compensation fires false alerts, over-compensation hides crying; not testable without hardware.
- **Option C — reference-based echo cancellation for the level only.** Pros: keeps detection. Cons: needs an AEC on the gate path, which `quiet-link-no-rtp` removed for good reasons; device-dependent.

Recommendation: A. Repeated presses: each press pauses at most 32 s; between presses the gate runs again with a fresh hold time. A parent who talks in a loop for minutes keeps the gate mostly paused; this is visible to both parents and is the parent's own action. (Critical review may tighten this, §12.)

### 6. Multi-parent compatibility

- Talk is per link: one `talk_request` belongs to one parent link, the talk line of that link's attempt carries the audio, and the baby plays only that link's remote talk track.
- One talker: the arbitration sits where the noise gate sits. On `origin/main` (one `Link`) it is trivially one talker. With `BabyHub` the hub holds `talker: Option<LinkId>`; a request from another link is denied with `busy`, and every other trusted link's status gets `talker: other`. The noise-gate pause is the hub's (one gate).
- Microphone per link (`multi-parent-pairings` "Audio per parent link") is unchanged: a talk does not open the baby's microphone to any link; listening, noise and tests do, as before.
- Order: whichever change lands second carries the integration (task 0.2). If talk-back lands first, `multi-parent-pairings` gets a task to move talker arbitration and the noise pause into the hub; if multi-parent lands first, talk-back implements them in the hub directly.

### 7. Baby playback: chime, volume, routing, Do Not Disturb

- **Stream.** Playout stays on the alarm stream (`USAGE_ALARM`, as the parent's ADM already does). The baby's automatic Do Not Disturb rule allows alarms, so talk-back stays audible while calls, messages and media of other apps stay silent; the ringer mode does not affect the alarm stream. `allowMedia` stays false.
- **Volume.** A talk-back level on the baby unit (§11 decision): during a talk the platform sets the alarm stream to the level's fraction of its maximum and restores the previous index afterwards (also after a crash: the previous index is remembered with the monitoring session, like the DND rule); the voice gets an 800 ms fade-in and a peak limiter on the remote track gain. The parent cannot change the volume.
- **Chime.** A soft two-note chime (about 600 ms, two sine tones with a slow attack and release, generated, no asset) at about 12 dB below the talk level, played by the platform on `play_talk_chime`, reported done by `talk_chime_done`.
- **Routing.** Loudspeaker; never the earpiece; no `MODE_IN_COMMUNICATION`. Bluetooth headsets connected to the baby phone are an edge case: the alarm stream also plays on the speaker on most devices (verified in the spike, task 2.1).
- **Indicator.** A dim "Parent is talking" chip in the kiosk at the existing edge-indicator brightness; the screen does not wake.

### 8. Web client

- **Option A — full web support:** web parent talks; web baby plays.
  - Pros: parity, "full client in both roles".
  - Cons: a browser cannot set or read the system volume or detect a muted output, so the parent may believe the child heard a message that never played; autoplay and output device vary; the chime, fade and alarm-stream behaviour cannot be guaranteed; more test surface (Firefox, Safari).
- **Option B — web parent talks; web baby reports `unsupported` (recommended).**
  - Pros: the common case (laptop or tablet as parent) is covered; a browser parent captures with `getUserMedia` on press and releases it on release; honest about what a browser baby unit cannot guarantee.
  - Cons: no talk-back with a laptop as baby unit; a backlog `IDEA` keeps it.
- **Option C — Android only in this change, web later.**
  - Pros: smallest change.
  - Cons: the web parent shows an unexplained gap; the web SDP must still carry the talk line (core rule), so most of the plumbing is needed anyway.

Recommendation: B. Even with B the web baby negotiates the talk line `recvonly` (the core rule is uniform) and never plays it.

### 9. UX on the parent monitoring screen

- A round talk button next to the listen button, same size, microphone symbol, label "Hold to talk"; testTag/id `parent_talk`. States: idle; requesting ("Wait…", button pressed); talking ("Talking — release to stop", pulsing ring, countdown from 5 s before the limit); denied (snackbar with the reason: off, other parent talking, baby device cannot play talk-back); unavailable (disabled with a reason line: not connected, talk-back off on the baby device, not supported).
- On grant the parent vibrates once briefly (a haptic "speak now"); no sound on the parent.
- First press asks for microphone permission with a plain reason ("To speak to your child, GuardedSleep needs the microphone while you hold the button"); a refusal shows how to allow it later.
- "Noise detection paused while you talk" under the level meter while paused; the other parent sees "Other parent device is talking — noise detection paused".
- Accessibility: TalkBack double-tap-and-hold talks; a long-press custom action is not offered (no latching). The button is never placed where the listen slide-to-lock gesture could reach it.
- Baby unit: a settings row "Talk-back: parents can speak through this device" (switch) and "Talk-back volume" (low, medium, high); the baby setup checklist shows one informational line about talk-back and the switch (§11 default).
- Copy in all locales (machine translated, `critical` tags for the talk denial reasons and the noise-paused line, because they explain why the parent's message did not arrive or why alerts are paused).
- Test tags (shared Android/web): `parent_talk`, `parent_talk_state`, `parent_talk_unavailable_reason`, `parent_noise_paused`, `baby_talking`, `baby_talk_toggle`, `baby_talk_volume`.

### 10. Security and privacy

- **Who can talk:** only a trusted link (verified attempt, pin proof done, alive). Requests through mailboxes are impossible (DataChannel only); the relay sees nothing new. Talk audio rides the attempt's DTLS-SRTP, bound to the pairing proof.
- **Abuse by a paired parent:** a paired parent is trusted by definition; the limits (30 s, rate, visible indicator, off switch on the baby unit) bound what a stolen or misused parent phone can do: it can speak, for at most 30 s per press, while an indicator shows it.
- **No recording:** no talk buffer, no file, no log of audio or levels of the talk line; the log canary runs in the talk sim scenarios; a JVM/Vitest check that no talk audio sink writes to storage.
- **Microphone indicator on the parent:** Android shows the system microphone indicator while the parent captures; the parent's microphone track exists only between grant and release.
- **No guardrail change:** no third party in the media path (§7.1 unchanged), no new crypto, no telemetry. Layers: decisions in the core (§3, §5), I/O in the platforms (§7), UI renders contract models only.

### 11. Owner decisions

Pending (asked 2026-11-11): duplex mode (§1), chime and volume (§7), web scope (§8), default of the baby's talk-back switch. Recorded here once answered.

### 12. Critical review

Pending: Critical Reviewer, Security/Privacy and UX perspectives (2026-11-11).

## Risks / Trade-offs

- [Alarm-stream volume changes are visible to the user and may fail on some OEMs] → spike task 2.1 on the emulator and a real device; fallback: software gain on the remote track only, with the parent told the level is not guaranteed.
- [Hardware AEC defaults on the baby ADM differ per device] → `quiet-link-no-rtp` disables them; until then, talk-back works with whatever the device does, and the noise pause covers the gate.
- [A cry during a talk is detected late] → bounded pause (§5), indicator on both parents.
- [Parent `RECORD_AUDIO` on Android 14+ needs the `microphone` foreground-service type while capturing] → the parent service adds the type only while a talk runs, which always starts from the foreground (button held on screen); verified in task 2.2.
- [Chrome's SDP munging deprecation] → no munging needed: directions come from `addTransceiver`.

## Migration Plan

No installed base; both devices update together. A peer without the talk line fails the new SDP rule, as an `update_required`-class mismatch would. Rollback: revert the change; the monitoring line is unchanged.

## Discarded alternatives

- §2 Option B (one `sendrecv` line) and Option C (renegotiation per press).
- §5 Options B and C (compensation, reference AEC).
- Parent-side volume control for the baby speaker: lets a remote party make loud sound next to the child; the adult at the baby unit decides.
- Text-to-speech or pre-recorded messages: out of scope; "no text-to-speech" stays.
- §1, §7 and §8 alternatives move here after the owner decision.
