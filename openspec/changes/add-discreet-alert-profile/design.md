# Design

## Context

Sources: `openspec/specs/parent-alerts/spec.md` (noise alert chain, lifecycle, snooze, test alert), `openspec/specs/audio-monitoring/spec.md` (baby sends audio only while the gate is loud, the parent listens or a test runs), backlog `IDEA` "Discreet alert profile", `docs/concept/why-guardedsleep.md` A1.

What exists today: the core raises `noise_alert { rms, probe, snoozed }` and only computes the `snoozed` flag from the stored snooze end. It decides no feedback. The platform decides it: Android in `FeedbackPlans.forNoise` and `ParentModel.onNoise` (which also mutes or unmutes the received audio), the web client in its mirrored `ParentModel`. The baby streams audio while the gate is loud, so playout is the parent's choice.

## Where the profile lives

**Platform (presentation), once per platform behind the existing contract models. Not the core.**

- Pros: the core never decides sound, vibration or screen (it emits `snoozed` and nothing else); snooze is a core input only because the *core* must hold the end time across reconnects for the event flag. An alert style has no safety semantics the core must enforce (no timer, no escalation); it only chooses what the parent device plays. No wire change, no vectors, no `api.txt` change.
- Cons: the rule "discreet means no playout until Listen" exists twice (Kotlin and TypeScript), like the snooze presentation does today. Mitigation: it is one pure function per platform with the same tests, and the setting itself is part of the mirrored contract models.
- Recommendation: platform. Safety logic that lives once in the core (pairing, link supervision, escalation, noise gate) is untouched.

Discarded alternative: a core input `SetAlertStyle` like `SetSnooze`. Pros: one implementation. Cons: grows the binding surface (`ffi`, `wasm`, `api.txt`, vectors, traces) for a value the core would only echo back in an event field; the core would start owning presentation. Rejected.

## Decisions

- **Setting:** `alertStyle` in `SettingsState` (`SOUND` default, `DISCREET`). Android persists it in `SettingsRepository` next to `alertOutput`; the web client keeps it in its normal, non-secret settings state (in memory, like every other web setting; never pairing material, never `localStorage`).
- **Gate:** `FeedbackPlans.forNoise(kind, snoozed, discreet)` gets a `playAudio` flag: false when snoozed or discreet. `ParentModel` mutes the received audio when `!playAudio` and the parent is not listening. The haptic, notification and full-screen parts of the plan do not depend on the style.
- **Listen:** the existing "Listen" control on the alert screen (`setListen`) unmutes playout, as today, on the chosen alert output. Returning Listen to off re-mutes while an event in "Discreet" is still running, so audio does not leak back in.
- **Snooze end:** when a snooze ends during noise, the full alert starts as today, with playout gated by the style.
- **Stop sound button:** in "Discreet" the alert screen button stops vibration (Android: "Stop vibration"; web: "Dismiss", the web client has no vibration), because "Stop sound" would be wrong. Test ids stay `alert_stop_sound`.
- **Indicator:** `discreet_indicator` on the parent monitor screen, shown whenever the style is "Discreet", tapping it opens the alert style choice. It is a persistent card, not a chip that can be hidden behind "more".
- **Two taps:** the "More" sheet of the parent monitor screen contains the choice (`alert_style_sound`, `alert_style_discreet`): More, then the choice. The same choice is in Settings.
- **Test alert:** runs the active style and states it ("Alert style: Discreet"). In "Discreet" the test vibrates and shows the alert and plays nothing (also no local test tone on the web).
- **Readiness:** unchanged. Items about alarm volume and headphones still warn in "Discreet" (the parent may switch back at any time, and level 3 is audible). Silencing them for the style is a later refinement.
- **Notification action** "Stop sound" on the Android noise notification keeps its label (it also stops vibration); a "Listen" action is out of scope.

### Effect on privacy guardrails and layers

No effect on AGENTS.md section 7: no new data leaves the device, no new storage of pairing material, no change to the media or alert path (the child's audio still flows to the parent unit only; the setting only decides playout). Layers: UI renders contract models (`SettingsState.alertStyle`, `ParentMonitorState` unchanged); platform decides playout; the core is unchanged.

### Failure modes (Critical Review)

- **Forgotten setting at night (missed-alarm risk).** In "Discreet" a sleeping parent with a muted phone is woken only by vibration. Mitigations: the persistent indicator, the default "Sound", level 3 stays an audible alarm with DND bypass, and the open question below. A false sense of safety is the main risk; it is why the safety net is raised to the owner and not silently skipped.
- **Vibration disabled on the device or phone on a table.** Discreet relies on the haptic and the screen; the readiness check does not detect vibration being off. Accepted for alpha; the indicator says what discreet does.
- **Web client:** browsers vibrate nowhere reliably; the web parent in "Discreet" is screen only. This is stated in the web delta spec; a web parent that needs to be woken should stay on "Sound".

## Open question for the owner (not implemented)

Should an ongoing discreet noise event switch to sound after N minutes of continuous unacknowledged noise (a parent who missed the vibration at a party still gets the child's audio)?

- **Option A: none.** Pros: simplest; the parent keeps full control; no surprise sound in a quiet room. Cons: a missed vibration means a crying child is never heard until the parent looks at the phone; the whole safety rests on the indicator.
- **Option B: after 3 minutes of continuous unacknowledged noise.** Pros: bounded worst case; a child that keeps crying reaches the parent. Cons: a sound surprise after 3 minutes may disturb the event or wake others; "unacknowledged" needs a defined acknowledgment (Listen, Stop vibration, Snooze); more state in the model and one more setting copy.
- **Option C: after 5 minutes.** Pros: fewer false escalations during fussing. Cons: a longer period in which a crying child is unheard.
- **Recommendation: B (3 minutes), with Listen, Stop vibration and Snooze counting as acknowledgment and the escalation switching the playout to sound once.** It keeps the discreet promise for ordinary fussing and removes the worst failure (a missed alarm). The decision is the owner's, and the implementation is a follow-up change.

## Risks / Trade-offs

- The rule lives in two platforms: covered by one pure function and the same cases in JUnit and Vitest.
- Android screenshot baselines for the new indicator and alert variant can be recorded only in the pinned build image; if it is unavailable the baselines are recorded later and the tests that need them are not added until then.

## Discarded alternatives

- **Third alert style "Silent" (screen only, no vibration):** the user case is vibration plus screen; a screen-only style is a missed alarm with the phone in a pocket. Out of scope.
- **Per-event-type configuration (sound, vibration, screen per alert type):** a separate backlog `IDEA`; too large for one switch.
- **Automatic discreet by location, calendar or time:** surprising, hard to explain, and a wrong guess misses an alarm. Rejected; the setting is explicit and visible.
- **Gate playout on the baby side (baby sends no audio):** would add latency to "Listen" and change the baby unit and the audio spec; the baby side stays unchanged so listening starts instantly.
