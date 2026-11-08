# Proposal

## Why

Parents at a party, a restaurant or a family event want to notice that the child makes noise **by vibration and screen only**, without the child's audio playing in a room full of people, and then decide to listen or go and check. Today every noise alert plays the child's audio on the alarm stream (`parent-alerts`, "Noise alert chain"): the audio is the alert, so there is no way to be told quietly. The idea is recorded as a backlog `IDEA` ("Discreet alert profile", positioning in `docs/concept/why-guardedsleep.md` A1); the owner decided to ship it in alpha.

This change touches a core concept of the parent alert chain (the child's audio is the alert), so the Critical Reviewer role applies (AGENTS.md section 6). The review points are answered in `design.md` (risk of a forgotten setting, the missed-alarm failure mode, the open safety-net question).

Tier: T0 (alpha). Persona impact: serves the parent at an event or a social evening (`docs/concept/personas.md`); trades off nothing for the night-time parent, because the default stays "Sound" and the setting is one switch of the one unified app, not a persona-specific layout or profile set.

## What Changes

- New parent setting **alert style**: "Sound" (default, today's behaviour) or "Discreet".
- In "Discreet", a noise alert vibrates (same 4 s repeat), wakes the screen, shows the full-screen alert over the lock screen, the live level meter and the visual alert state with the same 2-minute quiet rule, but **does not play the child's audio** until the parent taps "Listen" on the alert. The baby unit is unchanged; only parent playout is gated.
- Unchanged in "Discreet": connection levels 1 to 3 (level 3 stays an audible alarm with Do Not Disturb bypass), stopped baby unit, battery and readiness warnings, snooze semantics.
- While "Discreet" is active, the parent monitor screen shows a persistent indicator "Discreet alerts: no sound". The style can be changed from the parent monitor screen in at most two taps. The test alert runs the active style and says which one.
- Both clients: Android and web, with the same stable ids (`alert_style_sound`, `alert_style_discreet`, `discreet_indicator`).
- The profile is a **platform presentation setting** (no core change): see `design.md`, "Where the profile lives".

## Capabilities

### New Capabilities

### Modified Capabilities

- `parent-alerts`: new "Alert style" requirement; the noise alert chain, test alert and indicator behaviour under "Discreet".
- `web-client`: the browser parent honours the alert style (no audio playout until Listen, no test tone).

## Out of scope

- A safety net that switches an ongoing discreet event to sound after N minutes: **open question for the owner**, documented in `design.md`, not implemented.
- Vibration on the web client (the Vibration API is not used by the web client today), smartwatch alerts, per-alert-type alert configuration, vibration strength or pattern (separate backlog `IDEA`s).
- Any change to the baby unit, the core, the wire protocol or the connection levels.

## Impact

`apps/android/ui-contract` (`UiContract.kt`), `apps/android/platform` (`FeedbackPlan.kt`, `ParentModel.kt`, `LinkController.kt`, `SettingsRepository.kt`), `apps/android/app` (parent monitor, alert and settings screens), `apps/web/src/lib/contract`, `apps/web/src/lib/platform`, `apps/web/src/screens`, `locales/*.json` and `locales/critical.json`. The backlog `IDEA` is deleted when this change is merged (it is on the `docs/why-guardedsleep` branch, PR #35).
