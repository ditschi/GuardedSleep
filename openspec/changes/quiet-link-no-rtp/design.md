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

### 1. Detach the track, keep it disabled

- **Option A: `replaceTrack(null)` / `setTrack(null, false)` plus `enabled = false`** (recommended): zero packets in Chromium and Firefox, no renegotiation, resume in 20–60 ms; a failed detach degrades to silence, never to sound.
- Option B: `encodings[0].active = false`: Firefox silently keeps sending full audio, a privacy defect.
- Option C: Opus DTX: still 2–3 packets/s and comfort-noise frames.
- Option D: transceiver direction `inactive`: needs renegotiation through signaling.

The core keeps owning the mute decision (`apply_mic`); the platform applies it through one serialised queue, so quick toggles never race (`setParameters`-style transaction errors, a pending detach overtaken by an attach).

### 2. Quiet-send self-check

After every detach the platform compares the sender's `packetsSent` 10 s apart. If it still grows, it reports `QuietSendLeak { kbps }` to the core, which emits a status the baby UI shows once (dismissible) and the parent shows in diagnostics. The estimate per night is `kbps × 10 h`. Detection by measurement, not by user agent, covers browsers we cannot test (Safari) and future regressions.

### 3. Opus profile from the core

The core already parses the SDP (`sendonly` check). A new pure function sets the Opus `fmtp` and `ptime` for both offer and answer: `stereo=0; sprop-stereo=0; maxplaybackrate=16000; useinbandfec=1; usedtx=0; maxaveragebitrate=<level>`, `a=ptime:60`, `a=maxptime:60`. The receiver's values bind the sender, so the **parent's** setting decides, which also fits several parents later (one peer connection each). Changing the level applies from the next attempt; no renegotiation.

| Level              | Opus      | Wire via TURN              | Per hour of audio | Typical night\* |
| ------------------ | --------- | -------------------------- | ----------------- | --------------- |
| Data saver         | 16 kbit/s | about 20 kbit/s            | about 9 MB        | about 6 MB      |
| Standard (default) | 24 kbit/s | about 26 kbit/s            | about 12 MB       | about 7 MB      |
| Clear              | 32 kbit/s | about 34 kbit/s (estimate) | about 15 MB       | about 8 MB      |

\* 19 minutes of audio (three 5-minute crying phases, six short noises, four 30-second listen checks) plus about 3 MB of heartbeats and ICE checks; one relayed leg; doubles when both devices relay.

Wideband (up to 8 kHz) keeps crying, babbling, coughing and rustling clear; a listening test in the hardware round confirms the levels (task 8.1). The 40 ms of extra packet delay is far inside the 1–2 s budget.

### 4. Raw capture

- **Echo cancellation off:** the baby only sends (talk-back is `DEFERRED`), so there is no far-end signal to cancel; it can only distort.
- **Noise suppression off:** it is tuned for speech; it removes steady sounds (a white-noise machine, a fan) and can damp whimpering, breathing and rustling, so "is it really quiet?" would mislead and the gate would see a different level.
- **Auto gain off:** it raises the level in a quiet room over time, so a fixed threshold would fire at different loudness; loudness for listening is the parent's playback gain.
- **Android:** `JavaAudioDeviceModule.builder().setUseHardwareAcousticEchoCanceler(false).setUseHardwareNoiseSuppressor(false)`, audio source `UNPROCESSED` where `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED` is true, else `MIC`; software processing off through the audio options of the pinned webrtc-sdk (verify the exact API in task 2.2). The default threshold is re-checked on hardware because raw levels differ from processed ones (task 8.2).
- Discarded: a "reduce steady background noise" toggle now (backlog `IDEA` after the listening test).

### 5. Link quality from the alert path

The alert travels over the reliable DataChannel, so loss shows up as **delay**, never as a lost alert. The quality model therefore measures the delay of that path instead of RTP loss.

- **Heartbeat echo:** `heartbeat` gains `ack` (last received peer `seq`) and `ack_hold_ms` (time between receiving it and sending this heartbeat). RTT = now − own send time of `ack` − `ack_hold_ms`. No clock sync. Default interval 5 s (range 5–15 s unchanged): the radio already wakes for ICE consent about every 5 s, so the battery cost is small; about 0.6 kbit/s.
- **Inputs to the core:** heartbeat RTT and lateness (gap > interval + 2 s), the ICE candidate-pair `currentRoundTripTime`, consent `requestsSent`/`responsesReceived` and the local candidate type (host, srflx, relay) every 5 s from the platform, the network type (Wi-Fi, cellular) where the platform knows it, and RTP loss and jitter while audio flows.
- **Model over a 2-minute window:**
  - `good`: p90 RTT below 400 ms and no late heartbeat;
  - `slow { expected_delay_s }`: p90 RTT 400 ms–2 s, or one late heartbeat; the delay is the larger of p90 RTT and the worst lateness, rounded up to whole seconds;
  - `poor`: p90 RTT above 2 s, two or more late heartbeats, consent replies below 70 %, or RTP loss of 10 % or more while audio flows.
  - Upgrades only after 60 s in the better class (hysteresis); downgrades at once.
- **What is lost against RTP loss:** light loss (1–5 %) needs about 10 minutes of heartbeat samples to show, against seconds with 50 RTP packets per second; it affects only sound quality, and RTP loss is measured again whenever audio flows. **What is gained:** queue build-up on congested mobile cells (high delay without loss), which RTP loss never showed, appears within one or two heartbeats, and the UI can say how late an alert may be.
- Thresholds live in the core with golden vectors and sim scenarios (high RTT, burst loss, slow cell).

### 6. Parent UI

- Liveness line under the status: "Connected · checked {seconds} s ago", from the last peer message.
- `slow`: chip "Slow connection — alerts may arrive up to {seconds} s later" plus the cause when known ("over mobile data", "through your relay server").
- `poor`: the existing "Weak connection" chip. Loss of the link stays the existing alarm path.
- Diagnostics: connection type, RTT, quiet-send leak.

## Discarded alternatives

- `encodings[].active` (Firefox sends full audio), DTX only, direction `inactive` (see 1).
- Keep RTP for quality and add a periodic audio probe instead: more traffic, and audio and DataChannel share one bundled transport anyway. A short probe may come back as an end-to-end check of the audio pipeline (backlog `IDEA`).
- Heartbeats every 1–2 s: more radio wake-ups on mobile for little gain over ICE consent at 5 s.
- Quality level on the baby device: the receiver's SDP binds the sender, and each parent pays its own data.
- Runtime bitrate change through `setParameters({maxBitrate})`: not verified for audio in Firefox, and `ptime` cannot change without renegotiation.
- Noise suppression or auto gain on (see 4).

## Risks / Trade-offs

- Aggressive NATs may drop a binding kept alive only by ICE consent every 5 s; the mobile-data hardware rounds watch for it.
- The heartbeat field is a protocol v3 extension; old alpha builds without it still interoperate if the parser ignores missing fields, and there is no installed base to protect.
- Raw Android capture changes levels, so the default threshold and the gate's injected levels need a re-check.
