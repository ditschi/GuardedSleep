# Design

## Context

Sources: `openspec/specs/parent-alerts/spec.md` (noise alert chain, lifecycle, snooze, test alert), `openspec/specs/audio-monitoring/spec.md` (baby sends audio only while the gate is loud, the parent listens or a test runs), backlog `IDEA` "Discreet alert profile", `docs/concept/why-guardedsleep.md` A1.

What exists today: the core raises `noise_alert { rms, probe, snoozed }` and only computes the `snoozed` flag from the stored snooze end. It decides no feedback. The platform decides it: Android in `FeedbackPlans.forNoise` and `ParentModel.onNoise` (which also mutes or unmutes the received audio), the web client in its mirrored `ParentModel`. The baby streams audio while the gate is loud, so playout is the parent's choice.

## Where the profile lives

**Split by what each part is.**

- **The style, the setting and the playout gate: platform (Android), once.** The core never decides sound, vibration or screen (it emits `snoozed` and nothing else for presentation). Pros: no wire change; the gate is one pure function (`FeedbackPlans.forNoise`) behind the existing contract models. Cons: the rule is not shared with the browser, which does not offer the style anyway.
- **The escalation timer: Rust core (`guardedsleep-core`, parent `Link`).** AGENTS.md section 5.1 says timer-based alarms in a platform are bugs and safety logic lives once in the core. "Unacknowledged noise for N minutes switches to sound" is a safety timer, so the core owns it: sans-IO, time comes in through the existing `Tick` input, no new clock.

Discarded alternative: the whole profile in the core (a `SetAlertStyle` input). Cons: the core would own presentation; grows the surface for a value only the platform acts on. Rejected: the core gets only the delay.

### Core contract

- New input `SetNoiseEscalation { after_ms: Option<u64> }` (parent only; `None` = off).
- New input `AcknowledgeNoise` (parent only): "Stop sound" or "Stop vibration" was not a core input before.
- Existing inputs that already acknowledge: `SetListen { enabled: true }` and `SetSnooze { until_ms: Some(_) }`.
- New event `NoiseEscalated { after_ms }`, emitted once per core noise event when the delay passes without acknowledgment.
- The core's noise event: a non-probe `noise_alert` starts one when none is active or the previous event's last loud signal is 2 minutes old; a loud signal (a non-probe `noise_alert`, or a peer heartbeat whose level is at or above the baby's noise threshold) continues it; 2 minutes without a loud signal end it, so the next noise starts a new event and the count restarts. The core follows alerts and heartbeats, not the parent's audio levels (the parent unit's `NoiseTracker` stays presentation).
- The delay counts from the event start, or from the moment the delay was armed if that is later: switching to discreet in the middle of a running event never escalates at once.
- A snoozed `noise_alert` counts as acknowledged; when the snooze ends or is cancelled during noise the event restarts unacknowledged (the full alert starts then, as the snooze requirement says).

### Who passes the delay: only while discreet (the platform sends `None` otherwise)

- Option A (recommended): the platform sends `Some(delay)` only while the style is discreet and the delay choice is not Off, else `None`. Pros: the core never learns about "discreet"; no UI concept in the core; "Sound" needs no timer; Off is `None`. Cons: the platform must resend on style change and on a new run.
- Option B: the core gets the style and the delay. Pros: one input carries the whole policy. Cons: the core learns a UI concept; two inputs to keep consistent.
- Recommendation: A. Discarded: B.

## Decisions

- **Setting:** `alertStyle` and `discreetEscalation` (1, 2 default, 5 minutes, Off) in Android `SettingsState`, persisted in `SettingsRepository`. Choosing Off shows the warning "Off: a forgotten discreet mode can stay silent all night".
- **Gate:** `FeedbackPlans.forNoise(kind, snoozed, discreet, escalated)`: `playAudio` is false when snoozed, or discreet and not escalated. `ParentModel` mutes the received audio when `!playAudio` and the parent is not listening. The haptic, notification and full-screen parts of the plan do not depend on the style.
- **Escalation:** the platform maps `noise_escalated` to `escalated = true` for the running event, unmutes at once and shows "Switched to sound: no reaction for {minutes} min" on the alert. The flag lasts until the platform's event ends.
- **Listen:** the existing "Listen" control (`setListen`) unmutes playout on the chosen alert output. Ending Listen re-mutes while a discreet event is still running and not escalated. Switching from Discreet to Sound during a running event unmutes at once unless snoozed.
- **Acknowledgment:** Listen, "Stop vibration" (`stopAlertSound`, which now also sends `AcknowledgeNoise`) and Snooze.
- **Android only:** the web client does not offer the style (no vibration: the party case does not work there). It keeps "Sound" and ignores the new core event.
- **Indicator:** `discreet_indicator` on the parent monitor screen while discreet, a persistent chip that opens the style choice.
- **Two taps:** the More sheet holds the style choice (`alert_style_sound`, `alert_style_discreet`); the escalation choices (`discreet_escalation_1m`, `_2m`, `_5m`, `_off`) are in Settings.
- **Test alert:** runs the active style and states it. A test (probe) never starts an escalation timer.
- **Readiness:** unchanged.
- **Notification:** while discreet the noise notification gets a "Listen" action next to "Stop sound" (same action pattern); the action starts Listen like the alert button.

### Effect on privacy guardrails and layers

No effect on AGENTS.md section 7: no new data leaves the device, no new storage of pairing material, no change to the media or alert path (the child's audio still flows to the parent unit only; the setting only decides playout). Layers: UI renders contract models (`SettingsState.alertStyle`, `ParentMonitorState` unchanged); platform decides playout; the core is unchanged.

### Failure modes (Critical Review)

- **Forgotten setting at night (missed-alarm risk).** In "Discreet" a sleeping parent with a muted phone is woken only by vibration. Mitigations: the persistent indicator, the default "Sound", level 3 stays an audible alarm with DND bypass, and the escalation to sound (default 2 minutes, see "Safety net"). A false sense of safety is the main risk, which is why the net exists.
- **Vibration disabled on the device or phone on a table.** Discreet relies on the haptic and the screen; the readiness check does not detect vibration being off. Accepted for alpha; the indicator says what discreet does.
- **Web client:** browsers cannot vibrate reliably, so the party case does not work there: the style is not offered in the browser (backlog `IDEA`).

## Safety net: decided (owner, 2026-11-08)

Configurable, default 2 minutes; choices 1, 2 (default), 5 minutes and Off. Options considered, chosen one first:

- **Chosen: configurable, default 2 minutes, with Off.** Pros: bounded worst case for the default parent; the party parent can choose longer or Off knowingly; the switch is visible. Cons: one more setting; Off removes the net (hence the warning).
- Discarded: **none.** A missed vibration means a crying child stays unheard; the safety rests on the indicator alone.
- Discarded: **fixed 3 minutes** (planner's earlier recommendation). Not configurable; the owner preferred a shorter default with a choice.
- Discarded: **fixed 5 minutes.** A longer unheard period for every user.

## Risks / Trade-offs

- The core's event rule follows alerts and heartbeats, not audio levels, so in rare cases (a gap of 2 minutes between two alerts during sustained noise) the count restarts. Accepted: it can only add a switch to sound, never remove one.
- Android screenshot baselines for the new indicator and alert variant can be recorded only in the pinned build image; if it is unavailable the baselines are recorded later and the tests that need them are not added until then.

## Discarded alternatives

- **Third alert style "Silent" (screen only, no vibration):** the user case is vibration plus screen; a screen-only style is a missed alarm with the phone in a pocket. Out of scope.
- **Discreet style in the browser now:** screen only, no vibration; a missed alarm at a party. Rejected until the browser can vibrate.
- **Per-event-type configuration (sound, vibration, screen per alert type):** a separate backlog `IDEA`; too large for one switch.
- **Automatic discreet by location, calendar or time:** surprising, hard to explain, and a wrong guess misses an alarm. Rejected; the setting is explicit and visible.
- **Gate playout on the baby side (baby sends no audio):** would add latency to "Listen" and change the baby unit and the audio spec; the baby side stays unchanged so listening starts instantly.
