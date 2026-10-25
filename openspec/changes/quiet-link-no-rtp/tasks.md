# Tasks

Branch `feat/quiet-link-no-rtp` (from `feat/turn-credentials`). Commit after each task; `just test-affected` after each task. Groups 2 and 3 depend on group 1.

## 1. Core

- [ ] 1.1 Protocol v3 heartbeat fields `ack` and `ack_hold_ms` (optional when parsing, always sent); default interval 5 s; RTT computation per the spec scenario. Verify: protocol unit tests, conformance vectors blessed and reviewed, `just api-check` after `just api-snapshot`.
- [ ] 1.2 Opus profile: a pure SDP function for offer and answer (`stereo=0`, `sprop-stereo=0`, `maxplaybackrate=16000`, `useinbandfec=1`, `usedtx=0`, `maxaveragebitrate`, `a=ptime:60`, `a=maxptime:60`) with the level from the config; exposed through both façades. Verify: golden SDP vectors (Chromium, Firefox, libwebrtc offers), parser-module lint denies.
- [ ] 1.3 Quality model: inputs `IceStats { rtt_ms, consent_sent, consent_received, local_candidate, network }`, heartbeat RTT and lateness, RTP loss while streaming; output `LinkQuality::{Good, Slow { expected_delay_s }, Poor}` with cause; window, thresholds and hysteresis per the spec. Verify: unit tests, sim scenarios (high RTT without loss, burst loss, slow cell, quiet hour), traces.
- [ ] 1.4 `QuietSendLeak { kbps }` input and the baby/parent status it produces. Verify: unit test.

## 2. Android

- [ ] 2.1 Mute by `RtpSender.setTrack(null, false)` plus `setEnabled(false)` through one serialised queue; resume by `setTrack(track, false)`; quiet-send self-check from `packetsSent`. Verify: JVM tests with a fake sender; gate task 5.3.
- [ ] 2.2 Raw capture: `setUseHardwareAcousticEchoCanceler(false)`, `setUseHardwareNoiseSuppressor(false)`, audio source `UNPROCESSED` when supported else `MIC`, software processing off with the pinned webrtc-sdk API (document the API and link its source in a comment). Verify: JVM test of the source choice; gate run.
- [ ] 2.3 Platform stats feed every 5 s (candidate pair RTT, consent counters, local candidate type, network type from `ConnectivityManager`) into the core; `LinkMapping.quality` replaced by the core rating. Verify: JVM tests.
- [ ] 2.4 Parent UI: liveness line, slow chip with cause, weak chip from the new rating; audio quality setting with info sheet in Basic. Verify: Robolectric tests; Roborazzi baselines re-recorded in the pinned image (report `SKIPPED` without it).

## 3. Web

- [ ] 3.1 Mute by `sender.replaceTrack(null)` plus `track.enabled = false` through one serialised queue; self-check from `packetsSent`. Verify: Vitest with a fake sender; Playwright check in Chromium and Firefox that a quiet baby sends 0 RTP packets over 10 s.
- [ ] 3.2 Stats feed into the core (same fields as 2.3; network type from `navigator.connection` where present). Verify: Vitest.
- [ ] 3.3 Parent UI and settings as 2.4, same test ids. Verify: Vitest; Playwright.

## 4. Copy

- [ ] 4.1 Keys for the liveness line, slow chip (with cause variants), quality levels and info sheet, quiet-send notice; en, de, fr, es. Critical tags for the slow and weak chips. Verify: `just gen`, `just lint-text`.

## 5. Gate

- [ ] 5.1 AC-2: prove Doze liveness by heartbeats received at the parent (not audio packets) and assert a quiet baby sends no RTP packets.
- [ ] 5.2 AC-3b: injection → first audio packet at the parent and → playback start; limit 1 s after the gate opens (report the measured values).
- [ ] 5.3 Update `docs/ci-gates.md` for the changed and new gate steps. Verify: `just gate` (report `SKIPPED` without KVM).

## 6. Docs

- [ ] 6.1 `docs/contributing/turn-server-vps.md` §2: traffic figures from design.md §3 (per audio minute, per night, per month, per 10 TB). Verify: `just lint-text`.
- [ ] 6.2 Backlog `IDEA`s: pre-roll of the last seconds before the gate opened; periodic short audio probe as an end-to-end pipeline check; optional steady-noise reduction after the listening test. Verify: `just lint-text`.
- [ ] 6.3 After `alpha-connection-model` is archived: `Loss detection` names 15 s as the default liveness (2 × 5 s + 5 s). Verify: `openspec validate --all --strict`.

## 7. Checks

- [ ] 7.1 `just lint`, `just test`, `just verify-android`. Verify: all green.

## 8. Hardware round

- [ ] 8.1 Listening test with recordings (crying, babbling, coughing, rustling, white noise) at 16, 24 and 32 kbit/s; the owner confirms the levels.
- [ ] 8.2 Re-check the default noise threshold with raw capture on at least two Android devices.
- [ ] 8.3 Mobile-data round: NAT binding kept alive by ICE consent only; slow-chip behaviour on a congested cell.
