# Proposal

## Why

Parents must be able to tell at a glance, at 3 AM and without technical words, whether they are monitoring right now. ADR-013 decided the concept (owner, 2026-10-21) and the Android app renders it from the core's link state, but the rules sit in ADR-013, ADR-018 §2 and `user-interface.md`. This change records the built behaviour as the `connection-status` spec (tier T0, docs only).

## What Changes

- New capability spec `connection-status`: the parent's ambient bar per link state and level, accessibility, plain copy, the restored notice, the weak-connection chip, the monitoring notification text, and the baby unit's edge indicator and touch-wake status.
- ADR-013 is archived. Not built and moved to the backlog under the planned "Connection-status UI review": the network-mode banner with "Show connection details", indicator-loss training in onboarding with its 30-day reminder, and a diagnostics view.
- ADR-012's state names (`reconnecting_handoff`, `reconnecting_backoff`) are replaced by the core's link states and levels.

## Capabilities

### New Capabilities

- `connection-status`: how both units show the state of the monitoring link to people, in plain words, without waking the child.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/connection-status/`, ADR-013 (archived), `user-interface.md`, `docs/backlog.md`.
