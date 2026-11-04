# Design

## Context

`NoiseGate::sample` returns `Loud` once, on the transition, and stays loud while the level stays above the threshold. `Link::on_audio_level` (baby) logs the transition in `alert_log` and calls `send_app(noise_alert)`. `send_app` drops every message when there is no live attempt or the parent has not proven itself yet (`proof_due`). `on_trusted` sends `missed_events` from `alert_log` and flushes the status, but it does not look at the gate. Result: a loud state that started before trust is never announced. `noise_streaming` stays true, so the audio track does open on trust (`apply_mic`), but the parent has no alert, so it does not play it.

Layer: the decision belongs in the core (AGENTS.md §5.1: the noise gate and alerts live once, in the core). Platforms only see a `noise_alert` event, as today.

## Options

### A. Re-announce an ongoing loud state on trust (core, baby)

In `on_trusted`, after `send_missed_events`, if the gate is loud, send `noise_alert{rms: gate.last_rms(), t: now, probe: false}`.

- Pros: smallest change; the fix is in the one place that knows both the gate and trust; covers the first pairing and every recovery; no protocol or platform change; the parent's existing lifecycle (new vs. continue, snooze in the parent's core) applies unchanged.
- Cons: the parent may get both a missed-events report that counts this event and an alert for it. That is correct: one says what happened while the parent was away, the other that it is still going.
- Recommendation: **take A.**

### B. Queue the dropped `noise_alert` and send it on trust

- Pros: delivers exactly the message that was lost.
- Cons: a stale alert for noise that may have ended; duplicates the missed-event buffer; needs a queue with an expiry rule.

### C. Re-arm the gate on trust (force a quiet→loud transition)

- Pros: reuses the transition path, including `log_alert`.
- Cons: makes the parent wait a second hold time while the baby is already crying; logs a second alert in `alert_log` for one event.

## Discarded alternatives

- **B** (queue the dropped alert): reports stale noise and duplicates the missed-event buffer.
- **C** (re-arm the gate): delays a running alert by another hold time.
- **Fix only the test** (shorter pairing, wait for connection before sound, or longer timeout): hides a real missed-alarm path.
- **Loosen the threshold or hold time:** would raise false alarms and still miss the case.

## Critical review

- _Does this create a false alarm?_ Only if the gate is loud at trust time, which means the level has been above the threshold for at least the hold time and still is. That is the definition of a noise event.
- _Repeated alerts on a flapping link?_ Each recovery while loud sends one alert; the parent's lifecycle turns a repeat within 2 minutes into "continue" (audio, one short vibration, no new full screen). Acceptable, and better than silence.
- _Snooze?_ The parent's core applies snooze to every non-probe `noise_alert`, so a snoozed parent stays quiet.
- _Privacy:_ no new data; the level already goes in heartbeats and `missed_events`.

## Risks

- `guardedsleep-sim` traces that pair while the baby is loud will gain one `noise_alert`; re-bless and review the diff by hand.
