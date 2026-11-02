# Proposal

## Why

The parent's alert path is the product's safety promise, and it is built on Android in detail, but its rules are spread over ADR-005, ADR-011, ADR-018 §2, `notifications-alerts.md` and `alpha-ux-flows.md` §3.7–§3.11 (still marked `UNDER DISCUSSION`). This change records the built behaviour as the `parent-alerts` spec (tier T0, docs only). Per the owner (2026-10-22), built behaviour is specified and thereby decided; later changes go through OpenSpec.

## What Changes

- New capability spec `parent-alerts`: noise alert chain and event lifecycle, DND and silent-mode override, alert output and gain, test alert, readiness check, snooze, battery warnings, persistent foreground notification.
- Connection-loss feedback stays in `link-supervision` ("Escalation feedback per device").
- Superseded: feature-doc snooze 5/10/15 min (built: 10/30), baby battery thresholds 20/10 % (built: 15/5 %), "user can opt out of the DND override" (not built).
- ADR-011 is archived; its unbuilt parts (dual route, minimum gain, call resume) go to the backlog under the planned UX re-evaluation. ADR-005 is archived once `distribution` is migrated.

## Capabilities

### New Capabilities

- `parent-alerts`: how the parent unit alerts its user about noise and the baby unit's state, independent of silent mode and Do Not Disturb, and how the user checks that alerts will work.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/parent-alerts/`, ADR-011 (archived), ADR-005 note, `notifications-alerts.md`, `alpha-ux-flows.md` (reference note), `docs/backlog.md`.
