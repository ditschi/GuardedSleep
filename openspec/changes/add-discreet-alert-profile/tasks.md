# Tasks

Briefs for the implementer. Layers: UI renders contract models; the platform decides playout; the escalation timer is in the core (see `design.md`). Copy only through `locales/*.json`; ids in snake_case. Decisions of 2026-11-08: escalation timer in the core, browser does not offer discreet.

- [x] 1.1 Locale keys for the alert style, indicator, discreet alert hint and test sheet in `locales/en.json` and all locales; `native.ui.parent.discreet.*` critical; `just gen`
- [x] 2.1 Android contract and platform: `AlertStyle`, `SettingsState.alertStyle`, persistence, `FeedbackPlans.forNoise` with `playAudio`, `ParentModel` gate; JUnit tests
- [x] 2.2 Android UI: `discreet_indicator`, alert style choice in More and Settings, alert screen variant, probe sheet names the style; interaction tests; screenshots recorded in the pinned image
- [x] 3.1 Web: first version of the style (removed again by 5.1)
- [x] 4.1 Core: `SetNoiseEscalation`, `AcknowledgeNoise`, noise event tracking, `NoiseEscalated`; unit tests, link_pair test, vectors and traces blessed and reviewed, `ffi` mirror, `just api-snapshot`, `guardedsleep-sim` scenario if noise scenarios exist
- [x] 4.2 Android: map the core event and send the inputs (delay only while discreet and not Off, `AcknowledgeNoise` from Stop sound, resend on a new run); `discreetEscalation` setting with four choices and the Off warning (`discreet_escalation_1m`, `_2m`, `_5m`, `_off`); escalated alert text; JVM tests for the mapping
- [x] 4.3 Android review fixes: Discreet to Sound unmutes at once unless snoozed; "Listen" action on the noise notification while discreet
- [x] 5.1 Web: remove the style choice, indicator, discreet gating, web-only copy and tests; ignore `noise_escalated`; backlog `IDEA` for the browser
- [ ] 6.1 `just lint`, `just test`; Roborazzi baselines re-recorded only in the pinned build image
