# Proposal

## Why

Detecting the child's noise on the baby unit and getting the child's audio to the parent, without ever making a sound next to the child, is the core of the product. Its rules are spread over ADR-003 §1, ADR-018 §3, `audio-monitoring.md`, `communication.md`, `user-interface.md` and `alpha-ux-flows.md` §3.8 and §3.12. This change records the built behaviour as the `audio-monitoring` spec (tier T0, docs only).

## What Changes

- New capability spec `audio-monitoring`: capture and noise gate on the baby unit, audio sent only on demand and only baby → parent, listen mode, the parent's monitoring screen (meter, "All quiet since", baby battery), microphone health, the child-side silence guarantee, the baby's Do Not Disturb rule, silence status and remote silence.
- Superseded: alpha-ux-flows "audio stops 15 s after the level falls below the threshold" (built: audio stops when the gate turns quiet); ADR-003 §1 "enable on alert/listen" is kept and made precise.
- Moved to the backlog: sound categorisation, on-device AI, community data, multiple alert levels, talk-back, data estimate and ceiling, roaming warning, codec settings, latency display, nightlight, temperature display.

## Capabilities

### New Capabilities

- `audio-monitoring`: noise detection on the baby unit, transmission of the child's audio to the parent, and keeping the baby phone silent next to the child.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/audio-monitoring/`, notes in ADR-003 and ADR-018, `audio-monitoring.md`, `communication.md`, `user-interface.md`, `docs/backlog.md`.
