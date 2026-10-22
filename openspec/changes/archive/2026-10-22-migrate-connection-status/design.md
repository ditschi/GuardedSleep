# Design

## Context

Sources: ADR-013 §1–§6, ADR-018 §2 ("Connection visibility"), `user-interface.md`. As built: `ui/common/ParentWidgets.kt` (`LinkBar`), `ui/screens/BabyScreens.kt`, `link/ParentModel.kt`, `alerts/AlertController.kt`, `locales/en.json` (`native.ui.link.*`, `native.ui.baby.status.*`).

## Goals / Non-Goals

**Goals:** spec what both units show today. **Non-Goals:** the web client (wave 3); the planned UX review (backlog).

## Decisions

- **State model from the core**, not ADR-012's state names: the bar is driven by `LinkPhase` and the escalation level of `link-supervision`. ADR-013's 0–20 s "handoff" row maps to level 0, 20–60 s to level 1, > 60 s to level 2+.
- **Copy differs from ADR-013's table** (e.g. "Can't reach {name} — still trying" instead of "Baby unit not reachable -- still trying"); the catalogue wins, and the spec names the meaning, not exact strings, so copy can change without a spec change.
- **Not built, moved to the backlog:** network-mode banner and "Show connection details" (ADR-013 §4, ADR-018 T2; a catalogue key exists, no Android UI), indicator-loss training and its 30-day reminder (ADR-013 §5, ADR-018 onboarding), a user-facing diagnostics view (only a PoC log exists).
- ADR-013 §6 (listen during handoff) is covered by `link-supervision` ("Restoration").

## Risks / Trade-offs

- Indicator training was an alpha requirement in ADR-018 and is not built; it is now visible as a backlog gap instead of an implicit promise.
