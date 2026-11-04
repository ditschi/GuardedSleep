# Proposal

## Why

A noise alert can be lost for good. The baby's noise gate reports only the quiet-to-loud transition, and the core sends that single `noise_alert` only over a trusted attempt. When the gate turns loud while no trusted link exists (the first pairing takes longer than the hold time, or the link is down), the alert goes only into the missed-event buffer. When the link then becomes trusted while the noise is still going, the parent gets an incomplete missed-events report but no alert, and none will follow: the gate is already loud, so it never fires again until the room has been quiet first. A baby that cries continuously through a reconnect never alerts the parent. That is the worst failure in the product ("a missed alarm is the worst failure").

The flaky web E2E test "noise alert screen" (issue #17) is this bug: on busy CI runners, pairing took about 20 s, longer than the 5 s hold, while the fixture plays a continuous tone. The parent showed "We don't know what happened between 07:34 PM and 07:34 PM", which is the first-trust missed-events report of exactly that dropped alert, and never showed an alert.

Tier **T0** (alpha). A bug fix in the core's alert path. It does not change the architecture, the security model or the scope. It adds one rule to a core safety concept (when a noise event is sent), so the Critical Reviewer perspective is applied in design.md.

**Persona impact:** serves every parent persona (docs/concept/personas.md): an alert that is going on when the link comes up now reaches them. Nothing is traded off; the parent's event lifecycle still decides whether this is a new full alert or the continuation of an event.

## What Changes

- **Core (`Link`, baby role):** when an attempt becomes trusted (`on_trusted`, the first link and every recovery) while the noise gate is loud, the baby sends `noise_alert{probe: false}` with the current level right after the missed-events report. No protocol change: the message exists.
- **Spec:** `audio-monitoring` "Local noise detection" gains a failure scenario for noise that is still going when the link comes up.
- **Test:** a core unit test reproduces the bug (fails before the fix). The quarantined web E2E test "noise alert screen" is re-enabled.
- **Copy (side fix):** the "unknown" missed-events notice reads sensibly when start and end are the same minute (a new critical key under `native.ui.missed`, all locales, web and Android).

Out of scope: why CI pairing sometimes takes 20 s (the product handles it; a separate issue if it persists), parent-side changes to the noise event lifecycle, and T1/T2 items.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audio-monitoring`: "Local noise detection" gains the scenario "Baby still crying when the link comes up".

## Impact

- `crates/guardedsleep-core/src/link.rs` (baby `on_trusted`), core unit test, possibly a `guardedsleep-sim` trace re-bless if a scenario's output changes.
- `apps/e2e/tests/31-web-screens-a11y.spec.ts` (quarantine removed).
- `locales/*.json`, `apps/web/src/screens/ParentMonitorScreen.svelte`, `apps/android/app/.../ParentMonitorScreen.kt` (copy side fix).
- Privacy guardrails: unchanged. The alert goes only over a trusted, proven attempt, the same as today; the mic path rules ("Audio only on demand") are unchanged because the loud gate already opens it on trust.
