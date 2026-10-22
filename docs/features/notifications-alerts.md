# Feature: Notifications and Alerts

## Overview

The parent unit must reliably alert the caregiver when events are detected on the baby unit. Alerts must be attention-grabbing enough to wake a sleeping parent, but also configurable to avoid false alarm fatigue.

**Failure model:** When the **parent phone** is off, crashed, or killed by the OS, GuardedSleep cannot deliver alerts -- same contract as a hardware receiver. Mitigations are **observable parent-side state** (persistent notification, ambient connection affordance, onboarding training) and **one-tap reconnect** without visiting the baby device ([failure-model.md](../concept/failure-model.md), [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md)). **Baby-side sirens** when the parent link drops are **out of scope** (they wake the child).

**Audio output routing** for alert-triggered child audio follows [ADR-011](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md) (**one** primary UX with Advanced overrides; not separate persona skins).

## Requirements

> STATUS: DECIDED -- moved to [`openspec/specs/parent-alerts`](../../openspec/specs/parent-alerts/spec.md) (noise alert chain and lifecycle, DND override, alert output, test alert, readiness, snooze, battery warnings). Connection-loss feedback is in [`link-supervision`](../../openspec/specs/link-supervision/spec.md). Ideas and later-phase items are in the [backlog](../backlog.md) under `parent-alerts`.

## Dependencies

- [Communication](communication.md) -- receives alert messages and heartbeat status
- [Audio Monitoring](audio-monitoring.md) -- source of noise threshold events
- [Device Management](device-management.md) -- battery level reporting

## Open Questions

Resolved or moved to the [backlog](../backlog.md) (incoming phone calls: "Alert audio routing UX").

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
