# Tasks

Briefs for the implementer. Layers: UI renders contract models; platform decides playout; the core is untouched (see `design.md`). Copy only through `locales/*.json`; ids in snake_case, shared by both clients.

- [x] 1.1 Locale keys: `native.ui.settings.alertStyle.{label,sound,discreet}`, `native.ui.parent.discreet.indicator`, `native.ui.alert.{stopVibration,vibrationStopped,dismiss,discreetHint}`, `native.ui.probe.{styleSound,styleDiscreet,bodyDiscreet}` in `locales/en.json` and all locales (machine translated); add `native.ui.parent.discreet.*` to `locales/critical.json`; `just gen`
- [x] 2.1 Android contract and platform: `AlertStyle` and `SettingsState.alertStyle`; persist in `SettingsRepository`; `FeedbackPlans.forNoise(kind, snoozed, discreet)` with `playAudio`; `ParentModel` gates playout and re-mutes when Listen ends; `LinkController` passes the style; JUnit tests (discreet gives no playout until Listen, connection levels unaffected, snooze end)
- [x] 2.2 Android UI: `discreet_indicator` card, alert style choice in the More sheet and in Settings (`alert_style_sound`, `alert_style_discreet`), alert screen variant (stop vibration, hint), probe sheet names the style; interaction tests
- [x] 3.1 Web contract and platform: `AlertStyle` in `models.ts`, `defaultSettings`, fake controller; `ParentModel` playout gate; `AlertPlayer` plays no test tone in discreet; Vitest
- [x] 3.2 Web UI: indicator, style choice in the monitor screen and Settings, alert screen variant, test dialog names the style; Vitest and e2e ids
- [ ] 4.1 `just lint`, `just test`; Roborazzi baselines for the indicator and the alert variant recorded only in the pinned build image
