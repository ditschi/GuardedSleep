# Proposal

## Why

A baby monitor sends audio only when something happens, but the clients send a packet stream all night. Muting uses `track.enabled = false`, which keeps the Opus encoder running on digital silence: 50 RTP packets per second, about 30 kbit/s on the wire (measured in Chromium and Firefox; the emulator gate counted 91,408 packets in 30 quiet minutes on Android). That is about 136 MB per night per relayed leg over TURN and about 4 GB per month on a baby device on mobile data, keeps the radio awake, and contradicts the spec scenario "Quiet night: no audio from the baby unit is sent". The Opus defaults are also larger than the use case needs (Chromium about 32 kbit/s, Firefox about 64 kbit/s, 20 ms packets), and Android captures through the voice-call path with echo cancellation, noise suppression and auto gain, while web captures raw, so the noise gate sees different levels on the two platforms.

Sending nothing in quiet phases removes the RTP packets that the parent's connection-quality indicator is computed from today, so quality and "the link is alive" need a source that does not depend on audio. Tier T0, before the alpha hardware round.

## What Changes

- **No RTP while muted:** the baby detaches the track from the sender (`replaceTrack(null)` on web, `RtpSender.setTrack(null, false)` on Android) and keeps `enabled = false` as a fallback, so a failing detach can only ever send silence. `encodings[].active` is not used: Firefox ignores it for audio and keeps sending full audio.
- **Quiet-send self-check:** after muting, the platform checks the sender's packet counter; if packets keep flowing, the baby device shows a one-time notice with the measured extra data per night and that Chrome, Edge or the Android app use almost none, and the parent's diagnostics show it.
- **Speech-grade Opus profile from the core:** mono, wideband (`maxplaybackrate=16000`), in-band FEC, no DTX, 60 ms packets, bitrate by quality level. Measured over TURN: about 26 kbit/s at 24 kbit/s instead of 50 (Chromium) or 86 (Firefox).
- **Audio quality setting on the parent** (Basic): Data saver 16 kbit/s, Standard 24 kbit/s (default), Clear 32 kbit/s, with an info sheet that shows the expected data per hour of audio and per typical night. It applies from the next connection setup.
- **Raw capture on both platforms:** echo cancellation, noise suppression and auto gain off, Android records from the unprocessed or plain microphone source, so the noise gate sees the room as it is and the parent hears it as it is.
- **Link quality without audio:** heartbeats every 5 s by default carry an echo of the last received sequence number, so the core measures the round-trip time on the path alerts take. Quality combines that RTT, late or missing heartbeats and the ICE candidate-pair RTT and consent replies; while audio flows, RTP loss and jitter are added. The core states the expected alert delay.
- **Parent UI:** a liveness line ("Connected · checked 3 s ago"), a "Slow connection — alerts may arrive up to {seconds} s later" chip with the cause (mobile data, relay) and the existing weak-connection chip for poor quality.
- **Gate:** AC-2 proves Doze liveness by heartbeats instead of audio packets; new AC-3b measures injection → first audio packet and → playback start on the parent; a check that a quiet baby sends no RTP packets.
- Backlog: `IDEA` pre-roll (send the last seconds before the gate opened); TURN guide traffic figures corrected.

## Capabilities

### New Capabilities

### Modified Capabilities

- `audio-monitoring`: no RTP while muted, self-check, Opus profile, raw capture.
- `link-supervision`: heartbeat echo and 5 s default; link quality model in the core.
- `connection-status`: liveness line and slow-connection chip; weak connection from the new model.
- `device-setup`: audio quality setting with data estimate.

## Impact

`crates/guardedsleep-core` (protocol v3 heartbeat field, SDP profile, quality model, vectors, traces, sim scenarios), `crates/guardedsleep-{ffi,wasm}` (api snapshots), `apps/android` (media engine: sender detach, audio device module and source, stats feed; parent UI), `apps/web` (sender detach, stats feed, parent UI, settings), `locales/*.json`, `scripts/poc/gate.sh`, `docs/contributing/turn-server-vps.md`, `docs/backlog.md`. Depends on nothing; coordinate `Loss detection` wording with `alpha-connection-model` (see tasks).
