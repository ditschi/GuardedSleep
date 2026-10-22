# Design

## Context

Sources: ADR-005 (noise alert behaviour, connection-loss alerts), ADR-011 (alert output), ADR-018 §2 (alert path, readiness, test alert, snooze, other alerts), `notifications-alerts.md`, `alpha-ux-flows.md` §3.7–§3.11. As built: `apps/android/.../alerts`, `readiness/ReadinessChecker.kt`, `link/ParentModel.kt`, `config/Policy.kt`, and the battery logic in `crates/guardedsleep-core/src/link.rs`.

## Goals / Non-Goals

**Goals:** spec the built parent alert path, including details that so far only lived in `alpha-ux-flows.md`.

**Non-Goals:** connection-loss feedback (already in `link-supervision`); the web client's alert behaviour (wave-3 change); UI layout (stays in `alpha-ux-flows.md` as reference).

## Decisions

- **Built UX details are specified and thereby decided** (owner, 2026-10-22).
- **Where docs and code differ, the code wins** and the difference is listed:

| Topic                     | Doc                                                                            | Built                                                                      |
| ------------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| Snooze durations          | 5/10/15 min, configurable (feature doc)                                        | 10 and 30 min (ADR-018)                                                    |
| Baby low battery          | 20 %/10 % configurable (feature doc); clears above 20 % (alpha-ux-flows §3.11) | below 15 % not charging, below 5 % always; low re-arms at 15 % or charging |
| Alert output              | Default / Speaker only / Speaker + default (ADR-011)                           | Default / Speaker only; dual route not built (ADR-018 "not in the alpha")  |
| Minimum gain during alert | ADR-011                                                                        | not built                                                                  |
| DND override opt-out      | feature doc                                                                    | not built                                                                  |

## Risks / Trade-offs

- Freezing thresholds that were meant as provisional (`AlertPolicy` says "pending confirmation"). Mitigation: they are owner-confirmed by this spec, and the ADR-011/ADR-013 UX reviews in the backlog can change them through a normal change.
