# Proposal

## Why

A baby monitor that silently stops monitoring is worse than none: the core session ended permanently on failure, only the baby sent heartbeats, the server had no liveness check, and the pairing proof could be reflected. Migrated verbatim from ADR-017 (DECIDED 2026-10-20); `design.md` is the original ADR.

## What Changes

- Protocol v3 with a direction-bound pairing proof and a PSK-derived room id.
- A core `Link` that supervises one monitoring relationship and recovers with a fresh `Attempt`, make-before-break.
- Bidirectional heartbeat, liveness timeout, escalation levels 0–3, authenticated stop notices, missed-event reports.
- Server liveness, `SIGNALING_MAX_PARENTS` default 1 and per-IP limits.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `link-supervision` (§2–§9, §11, §12), `pairing` (§1) and `signaling-relay` (§10), migrated 2026-10-21/22.

## Impact

Archive-only.
