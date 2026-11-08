# Proposal

## Why

A baby monitor sends audio only when something happens, but the clients send a packet stream all night. Muting uses `track.enabled = false`, which keeps the Opus encoder running on digital silence: 50 RTP packets per second, about 30 kbit/s on the wire (measured in Chromium and Firefox; the emulator gate counted 91,408 packets in 30 quiet minutes on Android). That is about 136 MB per night per relayed leg over TURN and about 4 GB per month on a baby device on mobile data, keeps the radio awake, and contradicts the spec scenario "Quiet night: no audio from the baby unit is sent". The Opus defaults are also larger than the use case needs (Chromium about 32 kbit/s, Firefox about 64 kbit/s, 20 ms packets), and Android captures through the voice-call path with echo cancellation, noise suppression and auto gain, while web captures raw, so the noise gate sees different levels on the two platforms.

Sending nothing in quiet phases removes the RTP packets that the parent's connection-quality indicator is computed from today, so quality and "the link is alive" need a source that does not depend on audio. Detaching the track must not stop the microphone: on Android the noise gate reads the room level from the libwebrtc record callback, which may stop when no stream sends (Critical Review C1). Reliable noise detection on Android is the alpha's first requirement (owner, 2026-11-08), so capture that keeps running while nothing is sent is proven first, by a spike and by the gate with real microphone buffers.

This changes link supervision (liveness default 25 s → 15 s, owner 2026-11-08) and moves the link quality model from the platforms into the core, so the Critical Reviewer applied (review of 2026-11-08, findings answered in design and tasks). Tier T0, before the alpha hardware round.

## What Changes

- **Capture never stops while monitoring:** the noise gate keeps getting real microphone buffers while the sender is detached; the platform proves it, and the core reports `mic_silent` also when no buffers arrive. A spike (task 2.1) decides between the libwebrtc audio module and an own capture path on Android.
- **No RTP while muted:** the baby detaches the track from the sender (`replaceTrack(null)` on web, `RtpSender.setTrack(null, false)` on Android) and keeps `enabled = false` as a fallback, so a failing detach can only ever send silence. `encodings[].active` is not used: Firefox ignores it for audio and keeps sending full audio.
- **Send self-checks:** after muting, the platform checks the sender's packet counter; if packets keep flowing, the baby device shows a one-time notice with the measured extra data per night, and the parent's diagnostics show it. After unmuting it checks that packets flow within 1 s; otherwise the core retries once and tells the parent.
- **Speech-grade Opus profile from the core:** mono, wideband (`maxplaybackrate=16000`), in-band FEC, no DTX, 60 ms packets, a fixed 24 kbit/s. Measured over TURN: about 26 kbit/s instead of 50 (Chromium) or 86 (Firefox).
- **Raw capture on both platforms:** echo cancellation, noise suppression and auto gain off; Android records from the unprocessed or plain microphone source, so the noise gate sees the room as it is and the parent hears it as it is.
- **Link quality without audio:** heartbeats every 5 s by default carry an echo of the last received sequence number, so the core measures the round-trip time on the path alerts take. Liveness becomes `2 × 5 s + 5 s = 15 s` by default. Quality combines that RTT, late or missing heartbeats and, where the platform has them, the ICE candidate-pair RTT and consent replies; while audio flows, RTP loss and jitter are added. The core states the expected alert delay.
- **Parent UI:** a liveness line ("Connected · checked 3 s ago"), a "Slow connection — alerts may be delayed by about {seconds} s" chip with the cause (mobile data, relayed through TURN) and the existing weak-connection chip for poor quality.
- **Gate:** AC-2 proves Doze liveness by heartbeats instead of audio packets and asserts a quiet baby sends no RTP; the quiet phases run with real microphone buffers and without the injected-level fallback; new AC-3b measures noise → first audio packet and → playback start on the parent, including noise after a long quiet phase.
- **Out of scope (split, owner 2026-11-08):** the parent's audio quality setting (Data saver / Standard / Clear) and its data info sheet wait for the listening test (task 6.1); the backlog `IDEA` for audio quality settings gets the measured levels. Also out: pre-roll and an audio probe (backlog `IDEA`s).

Persona impact (`docs/concept/personas.md`): serves every parent on mobile data or a metered connection (less data and battery on the baby device) and every parent at night (a liveness line and an honest alert-delay chip); trades off a little faster loss alarms against possible extra reconnects on congested cells, checked in hardware round task 6.3.

## Capabilities

### New Capabilities

### Modified Capabilities

- `audio-monitoring`: capture keeps running while detached, no RTP while muted, send self-checks, fixed Opus profile, raw capture, microphone health without buffers.
- `link-supervision`: heartbeat echo and 5 s default, 15 s liveness, link quality model in the core.
- `connection-status`: liveness line and slow-connection chip; weak connection from the new model.

## Impact

`crates/guardedsleep-core` (additive protocol v3 heartbeat and status fields, SDP profile, quality model, vectors, traces, sim scenarios), `crates/guardedsleep-{ffi,wasm}` (api snapshots), `apps/android` (media engine: capture path, sender detach, audio source, stats feed; parent UI), `apps/web` (sender detach, stats feed, parent UI), `locales/*.json`, `scripts/poc/gate.sh`, `docs/ci-gates.md`, `docs/contributing/turn-server-vps.md`, `docker/public-server/README.md`, `docs/backlog.md`.

Dependencies: archive **after** `alpha-connection-model` (this change's `Loss detection` and `Supervision lives only in the core Link` deltas build on that change's text). Groups 2 and 3 start after alpha tasks 5.2 and 6.1 have landed (same media engine, link driver and ICE configuration); parent UI and diagnostics (2.4, 3.3) are coordinated with alpha 5.3/6.2, and the gate work (group 5) with alpha 5.4 (`gate.sh`, `ci-gates.md`).
