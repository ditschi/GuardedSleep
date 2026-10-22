# Proposal

## Why

Audio and control data must be protected in transit, pairing must resist MITM, and keys must not live in plaintext on disk. Migrated verbatim from ADR-004 (DECIDED 2026-04-15, amendments 2026-10-19/20 decided 2026-10-21); `design.md` is the original ADR.

## What Changes

- WebRTC DTLS-SRTP and DTLS, bound to the QR pairing by a fingerprint proof; no custom transport crypto.
- Amendments: direction-bound pairing proof v3, PSK-derived room id, strict fingerprint parsing and the DTLS cross-check.
- Keystore-encrypted pairing storage; WSS for signaling; protocol version on every message.
- Application-level E2E layer `DEFERRED` (Phase 2); a deep security audit is `PLANNED` before the alpha release.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `pairing` (migrated by `migrate-pairing` and `migrate-signaling-relay`, 2026-10-22).

## Impact

Archive-only.
