# Proposal

## Why

Repurposed old phones are a core story, but unmanaged old hardware risks unreliable monitoring and battery hazards. Migrated verbatim from ADR-010 (proposed 2026-05-13; decided by the owner 2026-10-21 with an amendment: no hardware minimum, trust comes from functional self-tests); `design.md` is the original ADR.

## What Changes

- No product-level minimum API and no whitelist; a non-blocking device check with OK / limited / not recommended; battery safety copy.
- Recurring self-tests, a first-use warning and safe parent alerts on potential issues are `PLANNED` after wave 3 ("Baby-unit self-tests" in `docs/backlog.md`).

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `device-setup` and `audio-monitoring` (migrated 2026-10-22).

## Impact

Archive-only.
