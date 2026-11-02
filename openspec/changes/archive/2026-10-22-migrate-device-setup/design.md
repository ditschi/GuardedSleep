# Design

## Context

Sources: ADR-009 (personas, failure model), ADR-010 with the owner amendment of 2026-10-21, ADR-018 §2 (onboarding, baby self-test, settings, baby monitoring, parent monitoring "Stop" row), `device-management.md`, `user-interface.md` (night mode, settings hierarchy, onboarding). As built: Android `ui/screens/{RoleHomeScreens,SetupScreen,BabyScreens,SettingsScreen}.kt`, `link/LinkController.kt`, `settings/`.

## Goals / Non-Goals

**Goals:** spec the built setup and kiosk. **Non-Goals:** recurring self-tests and first-use warnings (ADR-010 amendment, `PLANNED` after wave 3); indicator training (connection-status UI review); the web client.

## Decisions

- **ADR-009** becomes archive-only: personas, use cases and the failure model stay in `docs/concept/` as non-normative reference (owner, 2026-10-22); the binding consequence ("no audible alarm on the baby unit") is in `audio-monitoring` and `connection-status`.
- **ADR-010** is archived with its amendment: the built one-off device check is specified; recurring self-tests are planned.
- **Code over docs:** no role-switch UI exists (the doc's confirmation dialog and "Start as [last role]?" are not built); kiosk wake brightness 10 % and auto-lock 30 s come from the code; baby auto-resume after reboot is `DEFERRED` per ADR-018.
- The TURN field in Advanced exists while a hardened TURN profile is T1; the field only passes a user-provided server to ICE.

## Risks / Trade-offs

- Without a role-switch UI a user who chose the wrong role must clear app data; recorded in the backlog.
