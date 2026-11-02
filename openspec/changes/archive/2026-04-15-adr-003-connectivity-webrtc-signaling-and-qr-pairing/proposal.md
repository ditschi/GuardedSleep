# Proposal

## Why

Devices must connect over LAN, over the internet without forcing a VPN, and optionally over a VPN, and pairing must be simple and secure. Migrated verbatim from ADR-003 (DECIDED 2026-04-15); `design.md` is the original ADR.

## What Changes

- WebRTC peer-to-peer with a DataChannel and a pre-negotiated muted audio track.
- Internet signaling with STUN, optional TURN; local paths; VPN treated like any network.
- QR pairing (payload superseded by the v3 invite of ADR-017).

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `pairing`, `signaling-relay` and `audio-monitoring`; local discovery is part of the prepared change `add-offline-lan` (migrated 2026-10-22).

## Impact

Archive-only.
