# Proposal

## Why

Before building the app the project had to decide how much to build before validating risky assumptions, whether to split the MVP, and whether an in-app lock is needed. Migrated verbatim from ADR-001 (DECIDED 2026-04-15); `design.md` is the original ADR.

## What Changes

- Full documented MVP, no Phase 1a/1b split.
- No in-app App-Lock in the MVP; users rely on the OS screen lock (now `DEFERRED` in `docs/backlog.md`).
- A focused PoC before the full app (the WebView/Capacitor mechanism was superseded by the native PoC gate of ADR-015 and ADR-020).

## Capabilities

### New Capabilities

### Modified Capabilities

None: scope and PoC process are history; the app-lock outcome lives in the backlog and in `AGENTS.md` §9.

## Impact

Archive-only (migrated by `migrate-pairing`, 2026-10-22).
