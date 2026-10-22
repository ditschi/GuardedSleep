# Proposal

## Why

One install and one session must work across different networks, the same Wi-Fi, no internet, live handoff and transient outages. Migrated verbatim from ADR-012 (proposed 2026-05-13; goals decided by the owner 2026-10-21, state machine and handoff logic to be re-evaluated after the waves; partly superseded by ADR-017 and ADR-021); `design.md` is the original ADR.

## What Changes

- Embedded LAN listener on the Android baby unit with opaque NSD discovery; path ranking; handoff with hysteresis.

## Capabilities

### New Capabilities

### Modified Capabilities

The listener rules are in the prepared change `add-offline-lan`; recovery is in `link-supervision`; the re-evaluation is `PLANNED` in `docs/backlog.md`.

## Impact

Archive-only.
