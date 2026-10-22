# Feature: Communication

## Overview

Handles all data transmission between baby and parent units. Two audio transmission modes: (1) automatic alert-triggered audio when baby noise exceeds threshold, and (2) manual listen mode activated by parent. Audio always flows baby -> parent only. Parent -> child communication ("talk-back") is a future feature.

**Mobile data:** Internet / signaling mode may run on **cellular** whenever the OS default route uses mobile data (same user at home or away). The app must surface **approximate data usage** and a **roaming warning** when Internet mode is active on a metered or roaming network ([use-cases.md](../concept/use-cases.md) UC-3, [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md) companion).

## Requirements

> STATUS: DECIDED -- moved to [`openspec/specs/audio-monitoring`](../../openspec/specs/audio-monitoring/spec.md) (audio on demand, baby → parent only, listen mode), [`parent-alerts`](../../openspec/specs/parent-alerts/spec.md) (alert-triggered audio) and [`link-supervision`](../../openspec/specs/link-supervision/spec.md) (heartbeat, alert messages). Talk-back, data estimates, roaming warning, codec settings and latency display are in the [backlog](../backlog.md).

## Dependencies

- [Connectivity and Pairing](connectivity-pairing.md) -- underlying network connection and encryption
- [Audio Monitoring](audio-monitoring.md) -- triggers for alert transmission
- [Notifications](notifications-alerts.md) -- delivery of alerts on the parent device

## Open Questions

Moved to the [backlog](../backlog.md) under `audio-monitoring`.

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
