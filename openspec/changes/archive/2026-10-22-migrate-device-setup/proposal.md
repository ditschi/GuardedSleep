# Proposal

## Why

Monitoring only works if each phone is set up for its role: permissions, battery optimisation, Do Not Disturb access, a dark and locked baby screen, and settings that casual users cannot break. These rules are spread over ADR-009, ADR-010 (amended), ADR-018 §2, `device-management.md` and `user-interface.md`. This change records the built behaviour as the `device-setup` spec (tier T0, docs only).

## What Changes

- New capability spec `device-setup`: role choice, the per-role setup checklist, the baby device check, battery safety copy, the baby kiosk, settings (Basic and Advanced) and stopping monitoring on the parent.
- ADR-009 and ADR-010 are archived (personas and failure model stay as reference in `docs/concept`; ADR-010's self-test follow-up is `PLANNED` in the backlog).
- Superseded: "auto-resume baby mode after reboot" (MVP in device-management.md) is `DEFERRED` (ADR-018, Android 14 microphone restriction); "Start as last role" and the role-switch dialog are decided but not built.
- Backlog: role switching UI, reboot auto-start, OEM heartbeat check, estimated runtime, multi-baby, About/FAQ.

## Capabilities

### New Capabilities

- `device-setup`: preparing a phone for its role as baby unit or parent unit and keeping the baby unit dark, locked and correctly configured.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/device-setup/`, ADR-009 and ADR-010 (archived), `device-management.md`, `user-interface.md`, `docs/backlog.md`.
