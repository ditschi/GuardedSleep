# Proposal

## Why

ADR-005 makes the child's audio the alert, but not where that audio plays on the parent device (speaker or headphones) or what happens during a phone call. Migrated verbatim from ADR-011 (proposed 2026-05-13, decided by the owner 2026-10-21 with a UX re-evaluation after wave 3); `design.md` is the original ADR.

## What Changes

- Alert output setting: system default routing or speaker only; alert playback gain on top of the alarm volume.
- Dual routing, a minimum gain during alerts and automatic resume after calls were decided but not built; they are part of the `PLANNED` "Alert audio routing UX" item in `docs/backlog.md`.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `parent-alerts` (migrated 2026-10-22).

## Impact

Archive-only.
