# Proposal

## Why

Parents who switch from a hardware baby monitor expect to speak to the child ("I'm on my way") without walking in first; the concept review of 2026-05 counts talk-back among the top three expectations. It was `DEFERRED` to Phase 2 (AGENTS.md §9, `docs/backlog.md`). The owner pulled it forward on 2026-11-11.

Talk-back breaks a core concept and a prior decision: the baby unit was "always silent" (`audio-monitoring` "Silent baby unit", `device-setup` "Baby kiosk", the send-only SDP check in `link-supervision` "Sans-IO interface contract"), and audio flowed only from the baby unit to the parent. It also touches the security model (a remote party can now make sound next to the child) and the noise gate (the baby unit hears its own speaker). The Critical Reviewer therefore applies (AGENTS.md §6). Tier **T0** (alpha).

## What Changes

- **Push-to-talk from the parent (half duplex).** The parent holds a talk button on the monitoring screen; while it is held and the baby unit has granted the talk, the parent's microphone is sent to the baby unit, which plays it through its loudspeaker. Release ends it. One press lasts at most 30 seconds.
- **The core decides.** New core inputs and additive protocol v3 DataChannel messages (`talk_request`, `talk_grant`, `talk_deny`, `talk_end`) and per-link actions that tell the platforms when to attach the parent microphone and when the baby may play. Only a trusted link (verified attempt, pin-proven parent, live) may talk; the baby unit enforces the time limit, the one-talker rule and the rate limit on its own.
- **A second, pre-negotiated audio direction.** Every attempt negotiates a separate parent→baby audio m-line (the baby receives only; the parent sends only) next to the unchanged baby→parent m-line. The core checks both directions in the local SDP. No renegotiation per press; the parent sender carries no track and sends no RTP outside a talk.
- **Exception to the silent baby unit.** The baby unit plays sound only while a granted talk runs: a soft chime, then the parent's voice with a fade-in, at a volume set on the baby unit. Everything else stays silent (alerts, connection states, kiosk). Talk-back can be switched off on the baby unit.
- **Noise gate paused during talk-back.** The core ignores the baby's level while talk audio plays and for a short tail after it, so the speaker never raises a noise alert or escalation; the pause is bounded by the press limit and shown to the parents.
- **Indicators.** The baby unit shows a dim "Parent is talking" indicator while it plays; the talking parent sees the state of the press (waiting, talking, time left); with two parents (`multi-parent-pairings`), the other parent sees "Other parent device is talking" and cannot talk meanwhile.
- **No recording.** Talk audio is never stored, buffered beyond the media pipeline or logged on either device.
- **Web client:** scope per the owner decision in design §8 (open decision).
- **Copy** in all locales; stable test tags shared by Android and web.
- **Out of scope:** full duplex, talking to several baby units at once, a parent-side volume control for the baby speaker, pre-recorded messages, lullabies and white noise (`REJECTED`), video.

Persona impact (`docs/concept/personas.md`): serves every parent who wants to calm or reassure the child from another room, and the switcher from a hardware monitor; trades off the absolute silence of the baby unit for the parents who never use it, which the off switch on the baby unit restores.

## Capabilities

### New Capabilities

- `talk-back`: push-to-talk from a trusted parent link to the baby unit's loudspeaker: grant, limits, one talker, chime and volume, indicators, off switch, no recording.

### Modified Capabilities

- `audio-monitoring`: audio in the parent→baby direction only during a granted talk; the silent-baby-unit exception; the noise gate pauses during talk playback; Do Not Disturb keeps talk-back audible.
- `link-supervision`: the SDP rule of the sans-IO contract checks two audio directions.
- `device-setup`: the kiosk exception and the talk-back settings on the baby unit.
- `web-client`: talk-back in the browser per the owner decision.

## Impact

`crates/guardedsleep-core` (talk state per link, protocol v3 messages, SDP direction check, noise-gate pause, vectors, traces), `crates/guardedsleep-{ffi,wasm}` (API snapshots), `crates/guardedsleep-sim` (talk scenarios, log canary), `apps/android` (media engine: second transceiver, parent microphone capture on demand, baby playout gating, speaker routing, chime; parent `RECORD_AUDIO` and foreground-service type; parent talk button; baby indicator and settings), `apps/web` (per decision), `apps/e2e`, `locales/*.json`, `scripts/poc` (gate scenario), `AGENTS.md` §9, `docs/backlog.md`.

Coordination: `quiet-link-no-rtp` (sender detach applies to the parent's talk sender too; raw capture without echo cancellation is kept, design §4), `multi-parent-pairings` (one talker per baby hub, design §6), `add-offline-lan` (no interaction beyond the media path). Archive after `quiet-link-no-rtp`, which also modifies "Audio only on demand".
