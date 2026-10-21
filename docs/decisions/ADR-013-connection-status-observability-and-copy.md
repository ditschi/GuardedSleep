# ADR-013: Connection-status observability and copy on both devices

- Status: DECIDED for concept and goals (owner, 2026-10-21). UI realisation and detailed implementation are re-evaluated after the rework, including a UX review, see [backlog](../backlog.md) "Connection-status UI review"
- Date: 2026-05-13
- Deciders: Product owner (user), coordinated design (UX/Usability, Critical Reviewer 2026-05)
- Companion ADR: [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md) (defines the **states** this ADR renders).

> **Context note (2026-10-20):** file references under `apps/mobile` (Capacitor-era TypeScript) are historical. Link states come from the Rust core ([ADR-017](ADR-017-link-recovery-and-connection-supervision.md)); UIs render them from the catalogue. The web client is rebuilt as `apps/web` ([ADR-020](ADR-020-toolchain-and-development-setup.md) §5).

## Context

The user requirement, verbatim: _"the connection status is clearly visible, especially if there is an issue we need to see on both devices that the other device is not connected. This is critical information and must catch the user's attention."_

The UX/Usability sub-agent verdict on the current scaffold was **NEEDS REWORK**: today the parent UI shows the developer enum (`idle / connecting / connected / failed`) as a pill -- it does not match the **ambient-bar / escalating-vibration** model that [`failure-model.md`](../concept/failure-model.md) and [`ui-design.md`](../technical/ui-design.md) already require. The baby unit shows a static "Parent not connected" line; the failure-model's "secondary observability slow red pulse after ~60 s heartbeat loss" is **not** implemented.

This ADR locks the **product behavior** (timing, copy, symmetry) so engineering can deliver it.

## Decision

### 1. Parent unit: ambient connection bar

The parent monitoring screen shows a **full-width, dimensionally constant** colored bar at the **top edge** (or near-top region in the existing layout). It is **always present** while monitoring is started (does not disappear in any state).

| State (from ADR-012)                                                | Bar color          | Bar text                                | Sub-text below bar                             | Vibration                          | Notification                                               |
| ------------------------------------------------------------------- | ------------------ | --------------------------------------- | ---------------------------------------------- | ---------------------------------- | ---------------------------------------------------------- |
| `connected`                                                         | green              | Connected to \[Name\]                   | "All quiet since hh:mm" if no recent noise     | none                               | persistent FGS                                             |
| `webrtc_negotiating` / `signaling_connecting` / `discovering_paths` | yellow             | Connecting…                             | small countdown if `reconnectEtaMs > 0`        | none                               | persistent FGS                                             |
| `reconnecting_handoff` (0--20 s)                                    | yellow, slow pulse | same as connecting                      | Brief network pause -- we keep trying          | none                               | persistent FGS                                             |
| `reconnecting_backoff` (20--60 s)                                   | orange             | Baby unit not reachable -- still trying | none                                           | gentle vibration once at 20 s mark | persistent FGS + banner-style "alert" channel notification |
| `reconnecting_backoff` (>60 s)                                      | red, slow pulse    | Baby unit unreachable for \[m:ss\]      | Check whether the baby phone is still on       | escalating vibration cycle         | screen-on + foreground via existing `bringToForeground`    |
| `failed_needs_user`                                                 | red, steady        | Not connected                           | guided-troubleshooting link + Try again button | one final vibration                | screen-on + foreground                                     |

**Existing infra reused:**

- Persistent FGS: [`apps/mobile/src/lib/android/parent-monitor-notifications.ts`](../../apps/mobile/src/lib/android/parent-monitor-notifications.ts) `ensureParentMonitorNotificationsReady` + the existing alarm channel.
- Screen-on / foreground: existing `GuardedSleepNative.showAlarmStyleNotification` + `bringToForeground`.
- Reconnect ETA / backoff: existing [`session-auto-reconnect.ts`](../../apps/mobile/src/lib/webrtc/session-auto-reconnect.ts).

**Accessibility:** color alone is **not** enough. The bar always carries:

- A **symbol** (linked / broken-link / spinner) next to the text.
- The **text label** (translated).
- A **shape change** on `connected` → `reconnecting` (e.g. bar grows a thin animated stripe).

This addresses colour-blind users + dark-room visibility.

### 2. Parent unit: copy guidance (plain, non-technical)

| State                          | Copy (EN; DE in app per i18n)                                                                                  |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| connected                      | "Connected to \[Name\] -- All quiet since hh:mm"                                                               |
| connecting                     | "Connecting..." (no IPs, no port numbers, no "Socket timeout")                                                 |
| reconnecting_handoff           | "Brief network pause -- we keep trying"                                                                        |
| reconnecting_backoff (20-60 s) | "Baby unit not reachable -- still trying"                                                                      |
| reconnecting_backoff (>60 s)   | "Baby unit unreachable for \[m:ss\]. Check that the baby phone is on and not restricted."                      |
| failed_needs_user              | "Not connected. Tap to try again, or check that both phones are on the same network or both have mobile data." |

**Forbidden in production copy:** IPs, port numbers, words like "WebSocket", "DTLS", "ICE", "Socket timeout", "EHOSTUNREACH". These appear **only** in `Advanced -> Diagnostics`.

### 3. Baby unit: symmetric observability

The baby unit's role is to **not wake the child** ([failure-model.md](../concept/failure-model.md) Core Principle). Its connection-status surface is therefore:

- **Primary affordance:** the baby unit shows a small ambient indicator **only at the screen edge** (color + symbol), mirroring the parent (green / yellow / orange / red). When the baby device is in kiosk-immersive mode with the screen dimmed/off, the indicator does not light up the screen.
- **Secondary observability (`failure-model.md` F-1):** after **~60 s of parent-heartbeat loss**, the baby unit's screen shows a **slow red pulse** -- ~10 s period, **dimmed**, **near-black** baseline (OLED-friendly), capped luminance. Designed for an adult walking past the room, **not** the sleeping child.
- **On touch-wake of the baby unit while disconnected** the screen shows plain copy: "Parent device not responding -- we keep trying". Never shows IPs or technical detail.
- **No audible alarm on the baby unit under any connection-loss state** (binding from [ADR-009](ADR-009-personas-and-failure-model.md) and [failure-model.md](../concept/failure-model.md)).

### 4. Network-mode banner (Wi-Fi / cellular / local)

**Default: OFF.** The banner shows **only** when a meaningful path change persists ≥ 30 s and was triggered by a network event (not just startup choice). Wording:

- "Now on mobile data" -- one quiet line, no sound, no vibration.
- "Local -- no data leaves your network" -- only when LAN signaling won and ICE has only host candidates; this is the "Secure local mode" UI promise from [connectivity-pairing.md](../features/connectivity-pairing.md).

If the user enables "Show connection details" in **Basic → Display**, the banner is always visible. Default is off to avoid 3 AM jitter.

### 5. Indicator-loss training (onboarding)

Already DECIDED in [user-interface.md](../features/user-interface.md). This ADR makes it specific:

- During the **30-second "connected vs disconnected" lesson**, show the **same components** (ambient bar, baby pulse, escalating copy) the user will see in reality -- so the recognition path works.
- Immediately after the lesson, prompt the user to run the **probe test** (already DECIDED in [notifications-alerts.md](../features/notifications-alerts.md)).
- Optional gentle in-app reminder ("2-second check: green bar visible?") once per **30 days of unmonitored time** -- low-friction, no push notification.

### 6. Listen-mode interaction during handoff

When network handoff drops the session while Listen is ON:

- The parent UI shows the **`reconnecting_handoff`** state for 0--20 s **without** stopping the Listen toggle.
- If the new session reaches `connected` within the handoff window, **Listen auto-resumes** (the toggle stays visually ON), and a 2 s toast confirms: "Connection restored -- Listen on".
- If reconnection takes > 60 s, the Listen toggle is reset to OFF and the user must re-enable explicitly (avoids accidental open mic in the wrong room).

## Options Considered

### Option A: Generic enum pill (current scaffold)

- **Pros:** simple.
- **Cons:** **REJECTED** -- developer language, no escalation, no symmetric baby side, no accessibility back-up.

### Option B: Loud audio alarm on parent-side disconnect

- **Pros:** maximum attention.
- **Cons:** false alarms on every router reboot would train users to ignore; alarm fatigue. **PARTIALLY ADOPTED** -- vibration escalation per timing table, audible alarm only after **3 min** sustained disconnection (already specified in [`ui-design.md`](../technical/ui-design.md) escalation table).

### Option C: Ambient bar + escalating vibration + plain copy (CHOSEN)

- **Pros:** matches `failure-model.md` "indicator-loss as alarm" model; works in dark, one-handed; non-technical.
- **Cons:** more components to build than the enum pill; needs i18n + a11y review.

### Option D: Banner on every network change

- **Pros:** users always know what path is in use.
- **Cons:** 3 AM jitter, "Now on mobile data" / "Now on Wi-Fi" ping-ponging during weak signal. **REJECTED** as default; offered as opt-in via Basic settings.

## Consequences

### Positive

- Matches the safety-adjacent intent of [`failure-model.md`](../concept/failure-model.md): if the indicator is gone, the user **notices** without needing to read a manual.
- Symmetric baby-side observability satisfies the user's "both devices must show the other is disconnected" requirement without violating the "do not wake the child" guardrail.

### Negative

- More UI surface area (ambient bar component, baby-pulse animation, banner). Owned scope is ~3 small components in `apps/mobile/src/lib/components/` plus copy in `i18n/`.
- Tests must cover state-transition rendering, not just connection logic.

### Risks

- Persona B (daily home) may experience too many "yellow" yellow states on flaky home Wi-Fi → fatigue. Mitigation: hysteresis from [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md) keeps the bar **green** through 2-5 s blips; escalation starts only after the failure-model 20-s mark.

## Discarded alternatives

| Alternative                                                    | Reason                                                                                                                                    |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Always-on "Now on Wi-Fi / mobile" mode-banner                  | Default-off; only on for sustained changes or opt-in (see §4).                                                                            |
| Baby-side audible alarm on disconnect                          | Wakes the child; binding rejection in [ADR-009](ADR-009-personas-and-failure-model.md) / [failure-model.md](../concept/failure-model.md). |
| Bright baby-screen flash on disconnect                         | Wakes the child by light; rejected in [failure-model.md](../concept/failure-model.md). Only **slow dim red pulse** survived.              |
| Loud audible parent alarm on every disconnect <20 s            | Alarm fatigue; replaced by failure-model escalation table.                                                                                |
| Technical text "WebSocket closed (1006)" / IP / port for users | Reserved for Advanced → Diagnostics.                                                                                                      |
