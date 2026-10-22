# Proposal

## Why

Device behaviour (Doze, foreground service, alerts, recovery) had to be proven before hardware tests. Migrated verbatim from ADR-016 (proposed 2026-10-19, decided by the owner 2026-10-21 together with ADR-015; tooling details superseded by ADR-020); `design.md` is the original ADR.

## What Changes

- Local first, then virtual (emulator gate), then physical devices; a device-agnostic runner and an optional cloud farm.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `dev-toolchain` ("Test layers", migrated 2026-10-22).

## Impact

Archive-only.
