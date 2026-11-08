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
- **Safety net (owner decision 2026-11-08):** a discreet noise event that stays unacknowledged switches to sound after a configurable time, default 2 minutes (choices 1, 2, 5 minutes and Off; Off shows a warning). Acknowledgment is Listen, Stop sound or Stop vibration, or Snooze. The switch is visible on the alert. The timer lives in the Rust core (new input and event), not in the platform.
- **Android only for now (owner decision 2026-11-08):** the browser parent does not offer the discreet style because a browser cannot vibrate; it always uses "Sound". Stable ids on Android: `alert_style_sound`, `alert_style_discreet`, `discreet_indicator`, `discreet_escalation_1m`, `discreet_escalation_2m`, `discreet_escalation_5m`, `discreet_escalation_off`.
- The style and the playout gate are a **platform presentation setting**; only the escalation timer is core: see `design.md`, "Where the profile lives".
- Review fixes: switching from Discreet back to Sound during a running event unmutes at once (unless snoozed); the Android noise notification offers "Listen" while discreet.

## Capabilities

### New Capabilities

### Modified Capabilities

- `parent-alerts`: new "Alert style" and "Discreet escalation to sound" requirements; the noise alert chain, test alert and indicator behaviour under "Discreet".
- `link-supervision`: the core tracks the parent's noise event and emits `noise_escalated` when it stays unacknowledged for the configured delay.
- `web-client`: the browser parent does not offer the discreet style until it can vibrate.

## Out of scope

- The discreet style in the browser (backlog `IDEA` under web-client: once vibration is available), vibration on the web client, smartwatch alerts, per-alert-type alert configuration, vibration strength or pattern (separate backlog `IDEA`s).
- Any change to the baby unit, the wire protocol or the connection levels.

## Impact

`crates/guardedsleep-core` (`link.rs`, vectors and traces), `crates/guardedsleep-ffi` and `-wasm` (binding surface, `api.txt`, `core.d.ts`), `crates/guardedsleep-sim` (scenario), `apps/android/ui-contract`, `apps/android/platform`, `apps/android/app`, `locales/*.json` and `locales/critical.json`, `docs/backlog.md`. The web client only adapts to the new core event and input (it never sends the escalation delay). The backlog `IDEA` is deleted when this change is merged (it is on the `docs/why-guardedsleep` branch, PR #35).
