# Design

## Context

Measurements on 2026-10-25 (two local peer connections, `getStats`, 4–8 s per state; wire figures add RTP 12 + SRTP 10 + UDP/IPv4 28 + TURN ChannelData 4 bytes per packet):

| State                                 | Chromium             | Firefox 150              |
| ------------------------------------- | -------------------- | ------------------------ |
| Audio, defaults                       | 51 pkt/s, 50 kbit/s  | 50 pkt/s, 86 kbit/s      |
| `track.enabled = false` (today)       | 50 pkt/s of silence  | 49 pkt/s of silence      |
| `encodings[0].active = false`         | 0 pkt/s              | 47 pkt/s, **full audio** |
| `replaceTrack(null)`                  | 0 pkt/s              | 0 pkt/s                  |
| Resume after `replaceTrack(track)`    | 55 ms to first audio | 21 ms                    |
| Opus 24 kbit/s mono WB, 60 ms packets | 17 pkt/s, 26 kbit/s  | 17 pkt/s, 28 kbit/s      |
| Opus 16 kbit/s mono WB, 60 ms packets | 17 pkt/s, 20 kbit/s  | 17 pkt/s, 20 kbit/s      |

The last emulator gate run (2026-10-20) counted 91,408 audio packets at the parent during 30 quiet minutes (50.8 pkt/s), so Android behaves the same. AC-3 measured 987 ms from injection to the parent's alert with a 0.5 s hold time. The parent's quality (`LinkMapping.quality`) is today RTP loss over windows of at least 20 packets. Heartbeats (`t`, `seq`, optional `rms`, `battery`) run every 10 s by default over an ordered, reliable DataChannel and carry no echo. Web captures with echo cancellation, noise suppression and auto gain off (`audio.ts`); Android builds `JavaAudioDeviceModule` with defaults and `createAudioSource(MediaConstraints())`.

## Decisions

### 0. Capture keeps running while the sender is detached (Critical Review C1)

Android reads the room level only through the `JavaAudioDeviceModule` record callback (`LibWebRtcMediaEngine.kt`). In libwebrtc, `setTrack(null)` clears the send stream's source, and the audio device module stops recording when no sending stream is left. If that holds, the noise gate gets no buffers in a quiet room: no noise alert, and `mic_silent` does not fire either, because it only watches for digital silence. The gate would not notice: `LinkRunner` falls back to `InjectedAudio.level()` when no microphone level arrives.

Requirement: capture SHALL keep running while the sender is detached, and the platform SHALL prove it. Options, decided by the spike in task 2.1:

- **Option A: keep the libwebrtc audio module recording** (for example a second, never-attached local track or the module's own start/stop control in the pinned webrtc-sdk). Pros: one capture path for sending and the gate. Cons: depends on libwebrtc internals that may change with a webrtc-sdk bump; the spike must show it on a device and the emulator.
- **Option B: own `AudioRecord` capture for the noise gate**, libwebrtc only for sending. Pros: the gate never depends on the send state. Cons: two consumers of the microphone (Android allows concurrent capture only for some sources and API levels), a second level path to keep in step.

Recommendation: spike A first; fall back to B if A cannot be proven on both the emulator and a real device. Either way the core reports `mic_silent` when no capture buffers arrive for 30 s, and the gate's quiet phases run without the injected-level fallback. Web: `replaceTrack(null)` leaves the `MediaStreamTrack` and its `AudioWorklet`/analyser running, which Vitest and Playwright check.

### 1. Detach the track, keep it disabled

- **Option A: `replaceTrack(null)` / `setTrack(null, false)` plus `enabled = false`** (recommended): zero packets in Chromium and Firefox, no renegotiation, resume in 20–60 ms; a failed detach degrades to silence, never to sound.
- Option B: `encodings[0].active = false`: Firefox silently keeps sending full audio, a privacy defect.
- Option C: Opus DTX: still 2–3 packets/s and comfort-noise frames.
- Option D: transceiver direction `inactive`: needs renegotiation through signaling.

The core keeps owning the mute decision (`apply_mic`); the platform applies it through one serialised queue, so quick toggles never race (`setParameters`-style transaction errors, a pending detach overtaken by an attach).

### 2. Send self-checks

After every detach the platform compares the sender's `packetsSent` 10 s apart. If it still grows, it reports `QuietSendLeak { kbps }` to the core, which emits a status the baby UI shows once (dismissible) and sends to the parent in a status field over the DataChannel (additive in protocol v3, with vectors), so the parent's diagnostics show it. The estimate per night is `kbps × 10 h`. Detection by measurement, not by user agent, covers browsers we cannot test (Safari) and future regressions.

After every attach the platform checks that `packetsSent` grows within 1 s. If not, it reports `AudioResumeFailed` to the core, which asks the platform to attach once more and, if that fails too, tells the parent "No sound from the baby device" while the noise event itself still reaches the parent over the DataChannel (Critical Review I5).

### 3. Opus profile from the core

The core already parses the SDP (`sendonly` check). A new pure function sets the Opus `fmtp` and `ptime` for both offer and answer: `stereo=0; sprop-stereo=0; maxplaybackrate=16000; useinbandfec=1; usedtx=0; maxaveragebitrate=24000`, `a=ptime:60`, `a=maxptime:60`. It touches only the Opus `fmtp` and `ptime` lines, never the DTLS fingerprint or ICE lines (golden vector). Check Chrome's deprecation of local-description SDP munging before task 1.2; if it applies, the web client sets the remote description values instead (the receiver's values bind the sender).

**Fixed level at alpha (split, owner 2026-11-08):** 24 kbit/s. The table below is the basis for the later audio quality setting (backlog `IDEA`), which waits for the listening test (task 6.1). If it comes, the parent's value decides, because the receiver's values bind the sender, and a change applies from the next attempt.

| Level              | Opus      | Wire via TURN              | Per hour of audio | Typical night\* |
| ------------------ | --------- | -------------------------- | ----------------- | --------------- |
| Data saver         | 16 kbit/s | about 20 kbit/s            | about 9 MB        | about 6 MB      |
| Standard (default) | 24 kbit/s | about 26 kbit/s            | about 12 MB       | about 7 MB      |
| Clear              | 32 kbit/s | about 34 kbit/s (estimate) | about 15 MB       | about 8 MB      |

\* 19 minutes of audio (three 5-minute crying phases, six short noises, four 30-second listen checks) plus about 3 MB of heartbeats and ICE checks; one relayed leg; doubles when both devices relay.

Wideband (up to 8 kHz) keeps crying, babbling, coughing and rustling clear; a listening test in the hardware round confirms the levels (task 6.1). The 40 ms of extra packet delay is far inside the 1–2 s budget.

### 4. Raw capture

- **Echo cancellation off:** the baby only sends (talk-back is `DEFERRED`), so there is no far-end signal to cancel; it can only distort.
- **Noise suppression off:** it is tuned for speech; it removes steady sounds (a white-noise machine, a fan) and can damp whimpering, breathing and rustling, so "is it really quiet?" would mislead and the gate would see a different level.
- **Auto gain off:** it raises the level in a quiet room over time, so a fixed threshold would fire at different loudness; loudness for listening is the parent's playback gain.
- **Android:** `JavaAudioDeviceModule.builder().setUseHardwareAcousticEchoCanceler(false).setUseHardwareNoiseSuppressor(false)`, audio source `UNPROCESSED` where `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED` is true, else `MIC`; software processing off through the audio options of the pinned webrtc-sdk (verify the exact API in task 2.3). The default threshold is re-checked on hardware because raw levels differ from processed ones (task 6.2).
- Discarded: a "reduce steady background noise" toggle now (backlog `IDEA` after the listening test).

### 5. Link quality from the alert path

The alert travels over the reliable DataChannel, so loss shows up as **delay**, never as a lost alert. The quality model therefore measures the delay of that path instead of RTP loss.

- **Heartbeat echo:** `heartbeat` gains `ack` (last received peer `seq`) and `ack_hold_ms` (time between receiving it and sending this heartbeat). RTT = now − own send time of `ack` − `ack_hold_ms`. No clock sync. Default interval 5 s (range 5–15 s unchanged): the radio already wakes for ICE consent about every 5 s, so the battery cost is small; about 0.6 kbit/s.
- **Liveness:** `liveness_ms = 2 × interval + 5 s`, so 15 s by default and 35 s at the 15 s maximum interval (owner, 2026-11-08). The gate already runs at 5 s; hardware round task 6.3 watches for extra reconnects on congested cells.
- **Inputs to the core:** heartbeat RTT and lateness (gap > interval + 2 s), and every 5 s from the platform where it has them: the ICE candidate-pair `currentRoundTripTime`, consent `requestsSent`/`responsesReceived`, the local candidate **type** (host, srflx, relay; addresses never enter the core, traces or logs, AGENTS.md §7.6) and the network type (Wi-Fi, cellular); RTP loss and jitter while audio flows. ICE inputs are optional (Firefox exposes them only partly); without them the rating uses heartbeats alone. The baby's candidate type and network type reach the parent in the same status field as the send leak (§2).
- **Model over a 2-minute window:**
  - `good`: p90 RTT below 400 ms and no late heartbeat;
  - `slow { expected_delay_s }`: p90 RTT 400 ms–2 s, or one late heartbeat; the delay is the larger of p90 RTT and the worst lateness, rounded up to whole seconds;
  - `poor`: p90 RTT above 2 s, two or more late heartbeats, consent replies below 70 %, or RTP loss of 10 % or more while audio flows.
  - Upgrades only after 60 s in the better class (hysteresis); downgrades at once.
- **What is lost against RTP loss:** light loss (1–5 %) needs about 10 minutes of heartbeat samples to show, against seconds with 50 RTP packets per second; it affects only sound quality, and RTP loss is measured again whenever audio flows. **What is gained:** queue build-up on congested mobile cells (high delay without loss), which RTP loss never showed, appears within one or two heartbeats, and the UI can say how late an alert may be.
- Thresholds live in the core with golden vectors and sim scenarios (high RTT, burst loss, slow cell).

### 6. Parent UI

- Liveness line under the status: "Connected · checked {seconds} s ago", from the last peer message.
- `slow`: chip "Slow connection — alerts may be delayed by about {seconds} s" plus the cause when known ("over mobile data", "relayed through TURN").
- `poor`: the existing "Weak connection" chip. Loss of the link stays the existing alarm path.
- Diagnostics: connection type, RTT, send leak, as part of the diagnostics view of `alpha-connection-model` (same wording, "relayed through TURN"); the values are shown only and never sent anywhere.

## Discarded alternatives

- Audio quality setting (Data saver / Standard / Clear) at alpha: deferred by the split (owner, 2026-11-08); the levels need the listening test, and the setting would collide with alpha's settings work. Kept as backlog `IDEA` with the measured table.

- `encodings[].active` (Firefox sends full audio), DTX only, direction `inactive` (see 1).
- Keep RTP for quality and add a periodic audio probe instead: more traffic, and audio and DataChannel share one bundled transport anyway. A short probe may come back as an end-to-end check of the audio pipeline (backlog `IDEA`).
- Heartbeats every 1–2 s: more radio wake-ups on mobile for little gain over ICE consent at 5 s.
- Quality level on the baby device: the receiver's SDP binds the sender, and each parent pays its own data.
- Runtime bitrate change through `setParameters({maxBitrate})`: not verified for audio in Firefox, and `ptime` cannot change without renegotiation.
- Noise suppression or auto gain on (see 4).

## Risks / Trade-offs

- Aggressive NATs may drop a binding kept alive only by ICE consent every 5 s; the mobile-data hardware rounds watch for it.
- The heartbeat and status fields are additive in protocol v3, no version bump; `ack` is absent until the first peer heartbeat, and parsers accept missing fields. There is no installed base to protect.
- Capture while detached (§0) depends on the spike; until it is proven, groups 2–3 do not start.
- Raw Android capture changes levels, so the default threshold and the gate's injected levels need a re-check.
