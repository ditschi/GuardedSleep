# Proposal

## Why

The signaling relay is the only server in the media path's setup. Its rules live in ADR-003 §2, ADR-004 §4, ADR-017 §10 and the feature doc, while the decided replacement (Rust server, built-in STUN, relay admission) is not built yet. This change records the relay as it runs today, so the Rust server has a parity contract, as part of the ADR → OpenSpec migration (tier T0, docs only).

## What Changes

- New capability spec `signaling-relay`: rooms and roles, protocol pinning, membership, signal forwarding, liveness, resource and rate limits, origin allowlist, privacy of logs, and the client's ICE server defaults.
- VPN (Tailscale/Headscale) gets no special support: media may use VPN candidates, signaling always uses the configured server (owner, 2026-10-21). "No signaling server needed over VPN" is `REJECTED`.
- ADR-004 and ADR-017 are archived: their last open sections (§4 WSS; §10 server, §12 simulation cases) are covered by this spec, `pairing` and `link-supervision`.
- Decided but not built (ADR-020 §6, ADR-021 relay admission) is not specified here; it becomes the prepared change `add-rust-signaling-server`.
- Backlog: operating a public signaling instance (`UNDER DISCUSSION`), TURN profile and compose bundle (`PLANNED`, T1), stricter rate limits (`IDEA`).

## Capabilities

### New Capabilities

- `signaling-relay`: the WebSocket relay that brings a baby unit and a parent unit into one room and forwards their connection setup, and the client-side ICE server configuration.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/signaling-relay/`, ADR-004 and ADR-017 (archived), ADR-003 note, `docs/features/connectivity-pairing.md`, `docs/backlog.md`.
