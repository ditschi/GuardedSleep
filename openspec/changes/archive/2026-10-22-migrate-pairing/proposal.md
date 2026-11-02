# Proposal

## Why

Pairing is the trust anchor of GuardedSleep, but its rules are spread over ADR-001, ADR-003, ADR-004 (with two amendments), ADR-017 §1, ADR-018 §2 and `connectivity-pairing.md`, and the older texts contradict the newer ones (QR payload, validity, join requests). This change records the pairing behaviour as built today as the `pairing` spec, as part of the ADR → OpenSpec migration (tier T0, docs only).

## What Changes

- New capability spec `pairing`: 1:1 pairing by a physical QR scan, invite payload v3, PSK-derived room id, direction-bound pairing proof, fingerprint parsing and DTLS cross-check, encrypted storage, "Forget".
- Superseded statements are dropped rather than migrated: the ADR-003 QR payload (`psk`, `dtls_fingerprint`, `ips`, `nonce`, `ts`), the "~30 s one-time QR", "join request after the QR is dismissed" for the 1:1 alpha, and "long-term keys exchanged over the DataChannel".
- Not-yet-built and later-phase items move to `docs/backlog.md`: multi-parent and its join flow, babysitter mode, revocation and re-keying, key rotation, forward secrecy, app lock, application-level E2E layer, emoji fingerprint, QR payload codec in the core.
- ADR-001 is archived (archive-only: its scope and PoC process are history; app lock goes to the backlog). ADR-003, ADR-004 and ADR-017 are archived by `migrate-signaling-relay`, which covers their remaining sections.
- Out of scope: behaviour changes. Cleartext signaling toward private LAN addresses (ADR-021 §1) belongs to the prepared `add-offline-lan` change.

## Capabilities

### New Capabilities

- `pairing`: physical QR pairing of one baby unit and one parent unit, the pairing proof that binds the DTLS session to the shared PSK, and safe storage and removal of pairing material.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/pairing/`, `docs/decisions/ADR-001` (archived), notes in ADR-003, ADR-004 and ADR-017, `docs/features/connectivity-pairing.md`, `docs/backlog.md`. No code changes.
