# Proposal

## Why

The alpha needed a clear scope, the trust features that make parents rely on it, and a guarantee that the baby phone stays silent. Migrated verbatim from ADR-018 (DECIDED 2026-10-20, revision 2); `design.md` is the original ADR.

## What Changes

- Alpha on native Android and the web client on the shared core; link recovery, connection visibility and alarms, parent readiness, pairing, monitoring, test alert, child-side silence.
- Test bar and exit criteria for hardware tests; owner decisions on alarm tones, QR validity and terms.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `pairing`, `link-supervision`, `parent-alerts`, `connection-status`, `audio-monitoring`, `device-setup` and the prepared change `rebuild-web-client`; exit criteria are `PLANNED` in `docs/backlog.md` (migrated 2026-10-21/22).

## Impact

Archive-only.
