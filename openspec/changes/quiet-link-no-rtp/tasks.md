# Tasks

> **Spec base, 2026-11-10 (review 2026-11, finding D7).** `alpha-connection-model`'s `link-supervision` deltas were synced into `openspec/specs/`, so the MODIFIED blocks "Supervision lives only in the core Link" and "Loss detection" now quote the main text and differ from it only in this change's edits (15 s liveness, the quality rating, measurements from the platform). The archive order no longer matters for the specs; the code dependency on alpha 5.2 and 6.1 below stays.

Branch from `origin/main`. Commit after each task; `just test-affected` after each task. Group 1 can start now; groups 2 and 3 start after `alpha-connection-model` 5.2 and 6.1 have landed and depend on group 1; **task 2.1 blocks the rest of group 2 and group 5.** Archive after `alpha-connection-model`.

## 1. Core

- [ ] 1.1 Protocol v3 heartbeat fields `ack` and `ack_hold_ms` (additive, no version bump; optional when parsing, sent once a peer heartbeat was received); default interval 5 s and `liveness_ms = 2 × interval + 5 s` (15 s default); RTT computation per the spec scenario. Also an additive status field from the baby to the parent with the send leak, the local candidate type and the network type (never addresses). Verify: protocol unit tests, conformance vectors blessed and reviewed, `just api-check` after `just api-snapshot`.
- [ ] 1.2 Opus profile: a pure SDP function for offer and answer (`stereo=0`, `sprop-stereo=0`, `maxplaybackrate=16000`, `useinbandfec=1`, `usedtx=0`, `maxaveragebitrate=24000`, `a=ptime:60`, `a=maxptime:60`) that changes nothing else; exposed through both façades. First check Chrome's local-description SDP munging deprecation (design §3). Verify: golden SDP vectors (Chromium, Firefox, libwebrtc offers) including an unchanged DTLS fingerprint, parser-module lint denies.
- [ ] 1.3 Quality model: inputs heartbeat RTT and lateness, optional `IceStats { rtt_ms, consent_sent, consent_received, local_candidate_type, network }`, RTP loss while streaming; output `LinkQuality::{Good, Slow { expected_delay_s }, Poor}` with cause; window, thresholds and hysteresis per the spec; heartbeats alone when ICE stats are missing. Verify: unit tests, sim scenarios (high RTT without loss, burst loss, slow cell, quiet hour, no ICE stats), traces.
- [ ] 1.4 Inputs `QuietSendLeak { kbps }`, `AudioResumeFailed` (one retry, then the parent status "no sound from the baby device") and "no capture buffers" for `mic_silent` (30 s); the baby and parent statuses they produce. Verify: unit tests, sim scenario "capture stops while quiet".

## 2. Android

- [ ] 2.1 **Spike (blocking): capture while detached** (design §0). Show on the emulator and one real device whether the `JavaAudioDeviceModule` record callback keeps delivering buffers after `RtpSender.setTrack(null, false)`; try option A, else build option B (own `AudioRecord` for the noise gate). Record the result and the chosen option in design §0. Verify: an instrumented or gate check that counts record buffers for 60 s with the track detached.
- [ ] 2.2 Mute by `RtpSender.setTrack(null, false)` plus `setEnabled(false)` through one serialised queue; resume by `setTrack(track, false)`; quiet-send self-check and resume check from `packetsSent`; capture keeps running (2.1). Verify: JVM tests with a fake sender and a fake capture source.
- [ ] 2.3 Raw capture: `setUseHardwareAcousticEchoCanceler(false)`, `setUseHardwareNoiseSuppressor(false)`, audio source `UNPROCESSED` when supported else `MIC`, software processing off with the pinned webrtc-sdk API (document the API and link its source in a comment). Verify: JVM test of the source choice; gate run.
- [ ] 2.4 Platform stats feed every 5 s (candidate pair RTT, consent counters, local candidate type, network type from `ConnectivityManager`) into the core; `LinkMapping.quality` replaced by the core rating. Verify: JVM tests.
- [ ] 2.5 Parent UI: liveness line, slow chip with cause, weak chip from the new rating, diagnostics values inside alpha's diagnostics view (coordinate with alpha 5.3). Verify: Robolectric tests; Roborazzi baselines re-recorded in the pinned image (report `SKIPPED` without it).

## 3. Web

- [ ] 3.1 Mute by `sender.replaceTrack(null)` plus `track.enabled = false` through one serialised queue; quiet-send and resume checks from `packetsSent`; the analyser keeps getting samples while detached. Verify: Vitest with a fake sender; Playwright check in Chromium and Firefox that a quiet baby sends 0 RTP packets over 10 s and still detects injected noise.
- [ ] 3.2 Stats feed into the core (same fields as 2.4 where the browser exposes them; network type from `navigator.connection` where present). Verify: Vitest, including a browser without ICE stats.
- [ ] 3.3 Parent UI as 2.5, same test ids (coordinate with alpha 6.2). Verify: Vitest; Playwright.

## 4. Copy

- [ ] 4.1 Keys for the liveness line, slow chip (with cause variants), quiet-send notice and "no sound from the baby device", in all locales in `locales/` (`machineTranslated` markers). Critical tags for the slow and weak chips and the no-sound status. Verify: `just gen`, `just lint-text`.

## 5. Gate

- [ ] 5.1 AC-2: prove Doze liveness by heartbeats received at the parent (not audio packets), assert a quiet baby sends no RTP packets, and assert the baby receives real record buffers during the quiet phase; turn the `InjectedAudio.level()` fallback off for AC-2 and AC-3b.
- [ ] 5.2 AC-3b: noise → first audio packet at the parent and → playback start, also after a quiet phase of at least 10 minutes with the track detached; limit 250 ms at the baby from the gate change (report the measured values).
- [ ] 5.3 Update `docs/ci-gates.md` for the changed and new gate steps (coordinate with alpha 5.4). Verify: `just gate` (report `SKIPPED` without KVM).

## 6. Hardware round

- [ ] 6.1 Listening test with recordings (crying, babbling, coughing, rustling, white noise) at 16, 24 and 32 kbit/s; the owner confirms 24 kbit/s and decides on the audio quality setting (backlog `IDEA`).
- [ ] 6.2 Re-check the default noise threshold with raw capture on at least two Android devices, including noise after an hour of quiet.
- [ ] 6.3 Mobile-data round: NAT binding kept alive by ICE consent only; 15 s liveness without extra reconnects on a congested cell; slow-chip behaviour.

## 7. Docs

- [ ] 7.1 `docs/contributing/turn-server-vps.md` §2 and `docker/public-server/README.md`: traffic figures from design §3 (per audio minute, per night, per month, per 10 TB). Verify: `just lint-text`.
- [ ] 7.2 Backlog: extend the audio quality `IDEA` with the measured levels of design §3; add `IDEA`s for pre-roll of the last seconds before the gate opened, a periodic short audio probe as an end-to-end pipeline check, and optional steady-noise reduction after the listening test. Verify: `just lint-text`.

## 8. Checks

- [ ] 8.1 `just lint`, `just test`, `just verify-android`. Verify: all green.
