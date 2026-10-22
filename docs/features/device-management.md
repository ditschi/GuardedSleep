# Feature: Device Management

## Overview

Manages the device modes (baby unit vs. parent unit), battery optimization, and background execution. These are largely platform-level concerns that directly affect the feasibility and reliability of the entire application.

## Requirements

> STATUS: DECIDED -- moved to [`openspec/specs/device-setup`](../../openspec/specs/device-setup/spec.md) (roles, setup checklist, device check, battery safety, kiosk, settings), [`audio-monitoring`](../../openspec/specs/audio-monitoring/spec.md) (background capture, battery reporting) and [`link-supervision`](../../openspec/specs/link-supervision/spec.md) (process death). Reboot auto-start, role switching, kiosk-unlock notices, multi-parent and other later items are in the [backlog](../backlog.md) under `device-setup` and `pairing`.

## Dependencies

- [Audio Monitoring](audio-monitoring.md) -- battery and background execution directly affect monitoring
- [Connectivity](connectivity-pairing.md) -- battery level reporting uses the connection
- [Notifications](notifications-alerts.md) -- low battery alerts

## Open Questions

Moved to the [backlog](../backlog.md) under `device-setup`.

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
