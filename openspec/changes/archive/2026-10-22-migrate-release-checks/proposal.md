# Proposal

## Why

`migrate-distribution` missed two built checks that protect what ships: test-only hooks must not reach release APKs (ADR-016, ADR-018 §1), and the Rust core has a size budget (ADR-015 AC-6). Tier T0, docs only.

## What Changes

- Add two requirements to `distribution`: release builds without test hooks, and the core size budget.

## Capabilities

### New Capabilities

### Modified Capabilities

- `distribution`: adds the release-surface and core-size checks.

## Impact

Docs only: `openspec/specs/distribution/`.
