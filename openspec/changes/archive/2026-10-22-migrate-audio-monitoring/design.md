# Design

## Context

Sources: ADR-003 §1, ADR-018 §2 (parent monitoring, test alert, other alerts, child-side silence row) and §3, `audio-monitoring.md`, `communication.md`, `user-interface.md` (meter, "All quiet since", battery display), `alpha-ux-flows.md` §3.8 and §3.12. As built: `crates/guardedsleep-core/src/noise.rs` and `link.rs` (`apply_mic`, quiet requests, `mic_silent`), Android `silence/`, `media/`, `ui/screens/SettingsScreen.kt`, `locales/en.json`.

## Goals / Non-Goals

**Goals:** spec what the core and the Android app do today. **Non-Goals:** the web baby unit (wave-3 change; it will report silence as "unknown"); codec tuning.

## Decisions

- **Capability split:** the parent's monitoring screen (meter, "All quiet since", battery, listen) belongs here because it renders the audio path; device roles, onboarding, kiosk and settings get the separate capability `device-setup`.
- **Code over docs:**

| Topic                               | Doc                                                                      | Built                                                                                                            |
| ----------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| End of noise audio                  | "stops 15 s after the level falls below threshold" (alpha-ux-flows §3.7) | the microphone is muted when the gate turns quiet; the parent's visual event state lasts 2 min (`parent-alerts`) |
| Hold options                        | 3/5/10/15 s (feature doc, ADR-018)                                       | same; the core accepts 1–15 s                                                                                    |
| Multiple alert levels (soft/urgent) | feature doc "possibly"                                                   | not built → backlog                                                                                              |
| Roaming cost warning                | communication.md "MVP"                                                   | not built (ADR-018 T2) → backlog                                                                                 |

- ADR-018 §3's best-effort items were `UNDER DISCUSSION` "until hardware-tested"; they are built, so they are specified (owner rule 2026-10-22). Hardware results can change them through a normal change.

## Risks / Trade-offs

- Silence behaviour depends on OEM Do Not Disturb implementations; the spec states the intent and the honest reporting, the hardware soak tests check it.
