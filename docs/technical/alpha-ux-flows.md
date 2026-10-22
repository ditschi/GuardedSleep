# Alpha UX flows, screens and copy

> `STATUS: UNDER DISCUSSION` — UX/Usability proposal (2026-10-19) for the alpha defined in [ADR-018](../decisions/ADR-018-alpha-milestone-scope-and-trust-features.md). It renders the link model of [ADR-017](../../openspec/changes/archive/2026-10-20-adr-017-link-recovery-and-connection-supervision/design.md) and the observability rules of [ADR-013](../../openspec/changes/archive/2026-10-21-adr-013-connection-status-observability-and-copy/design.md). Where this document deviates from an ADR, the deviation is listed in [§0](#0-proposed-amendments-to-adrs) and stays a proposal until the ADR is amended.
>
> **Scope:** native Android app (baby + parent, Compose) and the web client (baby + parent, browser). Replaces the PoC UI in `apps/android/.../poc/` and the current SvelteKit routes (the web client is rebuilt as `apps/web`, see [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §5).
>
> **Status rule for this file:** every section heading carries a `STATUS` tag; rows in a table inherit the tag of their section unless a row says otherwise.

**How to read:** [§1](#1-vocabulary-and-global-rules) vocabulary → [§2](#2-screen-inventory) screens → [§3](#3-flows) flows → [§4](#4-screen-specifications) per-screen states, test IDs, message keys → [§5](#5-parent-monitoring-screen-layout-priorities) parent layout → [§6](#6-accessibility-and-dark-room-rules) accessibility and dark-room rules → [§7](#7-open-decisions-for-the-owner) open decisions.

---

## 0. Proposed amendments to ADRs

`STATUS: UNDER DISCUSSION`

| #   | ADR / doc         | Today                                                         | Proposal                                                                                                                                                             | Why                                                                                                         |
| --- | ----------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| A1  | ADR-017 §7        | Level 1 (20 s): one gentle vibration + banner                 | Level 1: **visual only** (orange bar, ongoing notification text changes), no vibration, no heads-up                                                                  | Liveness detection takes 25 s, so every Wi-Fi flap would buzz the parent at night (alarm fatigue).          |
| A2  | ADR-017 §7        | Level 2 (60 s): vibration + screen on; sound only at level 3  | Level 2: full-screen + repeating vibration **+ a short soft tone every 30 s** on the alarm stream; Level 3 (180 s): continuous alarm tone                            | A sleeping parent does not feel a phone vibrating on a nightstand; 3 min unmonitored is too long.           |
| A3  | ADR-017 §7–8      | No acknowledge semantics for the connection alarm             | "I'm checking" mutes sound + vibration for 5 min, bar stays red, re-escalates to level 3 if still down                                                               | Without it, parents kill the app to silence it and end up unmonitored.                                      |
| A4  | ADR-017 §6–7      | Deliberate stop on one side looks like an outage to the other | Link sends `stop_notice{reason}` before closing; peer shows "stopped" (not "lost"), parent gets one clear alert, no loop                                             | Babysitter stopping the baby phone must be told plainly; a parent stop must not leave the baby pulsing red. |
| A5  | ADR-018 §3        | Silence policy covers sound only                              | DND policy also suppresses **visual effects** (screen-on, peek, notification light, ambient display); status reports `screen_may_light`                              | Light wakes children too (failure-model rejected flashes).                                                  |
| A6  | ADR-018 T1        | Parent device's own ability to alert is not checked           | **Parent readiness check** (notifications, full-screen alerts, DND bypass, alarm volume > 0, battery optimisation, own battery) with a chip on the monitoring screen | "Reliably informed" fails silently if the parent's alarm volume is 0.                                       |
| A7  | ADR-018 T1        | Low battery only for the baby                                 | Add parent's own battery (< 20 % not charging) and "baby phone stopped charging"                                                                                     | F-1 (parent phone dies) is the most likely undetectable failure.                                            |
| A8  | ADR-013 §2 copy   | "Baby unit", "not restricted"                                 | Copy vocabulary of §1.1 ("baby phone", no "restricted")                                                                                                              | One term everywhere; "restricted" is jargon.                                                                |
| A9  | ADR-013 §6        | Listen auto-resumes only within the 20 s handoff window       | Follow ADR-017 §8: re-sent if outage < 60 s                                                                                                                          | Two ADRs disagree; ADR-017 is the newer source.                                                             |
| A10 | failure-model F-6 | Probe = "synthetic noise burst on the baby device"            | Probe injects a **digital** synthetic event in the baby pipeline; **no sound on the baby**                                                                           | Child-side silence guarantee (ADR-018 §3.1).                                                                |
| A11 | ADR-017 §7        | Headline semantics during outage undefined                    | "All quiet since" is never shown while not connected; after recovery it only continues if `missed_events` covered the gap                                            | Claiming "all quiet" across an outage is false reassurance.                                                 |

---

## 1. Vocabulary and global rules

`STATUS: UNDER DISCUSSION`

### 1.1 Words users see

| Concept                | Use                                                | Never use                                             |
| ---------------------- | -------------------------------------------------- | ----------------------------------------------------- |
| Device near the child  | **baby phone** (or its name, default "Baby phone") | baby unit, baby device, transmitter, host, offerer    |
| Device with the parent | **parent phone** / "this phone"                    | parent unit, receiver, client                         |
| Running session        | **monitoring**                                     | session, link, attempt                                |
| Link OK                | **connected**                                      | verified, paired-up, online                           |
| Link down, retrying    | **reconnecting** / **can't reach**                 | backoff, ICE, socket, signaling, timeout              |
| Pairing material       | **code** (QR code / pairing code)                  | PSK, secret, token, room, invite JSON                 |
| DND state              | **silent** / **may ring**                          | interruption filter, ringer mode, notification policy |
| Listen                 | **listen**                                         | stream, track, media                                  |

ADR-013 forbidden list applies (IPs, ports, WebSocket, DTLS, ICE, error codes). Those appear only in Settings → Advanced → Diagnostics.

### 1.2 Conventions

- **Test IDs:** `snake_case`, `<screen>_<element>`; identical string for Compose `Modifier.testTag` and web `data-testid` (existing kebab-case IDs are migrated).
- **Message keys:** dotted lowerCamel as in today's catalogue, living in `locales/en.json` (ADR-015). Arguments use ICU syntax `{name}`; counts use `{count, plural, one {…} other {…}}`. `{time}` and `{duration}` are formatted by the UI (ADR-015 "Formatting stays native"); `{duration}` reads "45 s", "2 min", "1 h 5 min".
- **Copy style:** sentence case, ≤ 60 characters for status lines, a fix in every warning ("what happened → what to do"), no exclamation marks except level-3 alarm.
- **Haptics:** two distinct patterns so parents can tell them apart by feel: **noise** = long–pause–long; **connection** = short ×3. Battery/info = one short pulse.

---

## 2. Screen inventory

`STATUS: UNDER DISCUSSION`

| ID  | Screen (route / Compose destination)        | Native baby | Native parent |  Web baby   | Web parent  | Purpose                                                                                                                                                            |
| --- | ------------------------------------------- | :---------: | :-----------: | :---------: | :---------: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| S01 | `role` — Which phone is this?               |      ✓      |       ✓       |      ✓      |      ✓      | First run only; picks role                                                                                                                                         |
| S02 | `baby_setup` — Get this phone ready         |      ✓      |               | ✓ (reduced) |             | Permissions, silence access, battery, safety, self-test                                                                                                            |
| S03 | `parent_setup` — Get this phone ready       |             |       ✓       |             | ✓ (reduced) | Notifications, lock-screen alerts, DND bypass, battery, honest limits                                                                                              |
| S04 | `baby_pair` — Show code                     |      ✓      |               |      ✓      |             | QR + copyable code, countdown                                                                                                                                      |
| S05 | `parent_pair` — Scan code                   |             |       ✓       |             |      ✓      | Camera scan + paste                                                                                                                                                |
| S06 | `home` — saved pairing                      |      ✓      |       ✓       |      ✓      |      ✓      | One-tap start; entry to settings / new pairing                                                                                                                     |
| S07 | `baby_monitor` — monitoring (awake)         |      ✓      |               |      ✓      |             | Status, lock, stop; shown after unlock                                                                                                                             |
| S08 | `baby_kiosk` — dark locked screen           |      ✓      |               | ✓ (overlay) |             | Near-black, touch-to-wake, hold-and-slide unlock                                                                                                                   |
| S09 | `parent_monitor` — monitoring               |             |       ✓       |             |      ✓      | Main screen (§5)                                                                                                                                                   |
| S10 | `parent_noise_alert` — full screen          |             |       ✓       |             | ✓ (in-page) | Noise alert over lock screen                                                                                                                                       |
| S11 | `parent_link_alarm` — full screen           |             |       ✓       |             | ✓ (in-page) | Connection lost level ≥ 2, baby stopped                                                                                                                            |
| S12 | `settings` (role-specific Basic / Advanced) |      ✓      |       ✓       |      ✓      |      ✓      | §3.13                                                                                                                                                              |
| —   | Sheets / dialogs                            |             |               |             |             | `sheet_silence`, `sheet_snooze`, `sheet_probe`, `sheet_readiness`, `sheet_help_reconnect`, `dialog_stop`, `dialog_forget`, `card_missed_events`, `coach_first_run` |
| —   | Notifications (native)                      |      ✓      |       ✓       |             |             | §4.13                                                                                                                                                              |

Launch routing (all clients): monitoring flag set → resume straight into S07/S09 (ADR-017 §9) · else pairing saved → S06 · else onboarding not done → S01 · else S06 with "Set up" card.

---

## 3. Flows

### 3.1 First run — baby phone

`STATUS: UNDER DISCUSSION`

| Step | Screen | User sees / does                                                                                                                                    | System                                                    | Failure path                                                                                                                     |
| ---- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 1    | S01    | Two large cards; hint "Start with the baby phone". Taps **Baby phone**.                                                                             | Role remembered.                                          | —                                                                                                                                |
| 2    | S02    | One checklist screen, each row = status + button: Microphone (required), Keep running in background, Silence this phone, Safety note, Device check. | Self-test runs in background (ADR-010).                   | Mic denied → row red + "Allow in Settings"; **Continue** disabled until granted. Others may be skipped; skipped rows stay amber. |
| 3    | S02    | Taps **Continue**.                                                                                                                                  | —                                                         | Device check "may disconnect sometimes" → amber row text only, never blocks (Persona C).                                         |
| 4    | S04    | QR code + "Code valid for 4:59". Instruction: "On your phone, open GuardedSleep and choose Parent phone."                                           | Pairing window open 5 min (ADR-018).                      | See §3.3.                                                                                                                        |
| 5    | S07    | After the parent verifies: "Connected to parent phone" + "Monitoring". Hint "The screen goes dark in 30 s".                                         | Monitoring starts automatically; silence applied (§3.12). | —                                                                                                                                |
| 6    | S08    | Screen fades to near-black over 5 s.                                                                                                                | Kiosk on.                                                 | —                                                                                                                                |

### 3.2 First run — parent phone

`STATUS: UNDER DISCUSSION`

| Step | Screen | User sees / does                                                                                                                                          | System                                                             | Failure path                                                                                                                                                             |
| ---- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1    | S01    | Taps **Parent phone**.                                                                                                                                    |                                                                    |                                                                                                                                                                          |
| 2    | S03    | Checklist: Notifications (required), Alerts on the lock screen, Alerts during Do Not Disturb, Keep running in background. Honest-limits card (F-1, F-4).  | —                                                                  | Notifications denied → **Continue** disabled with "GuardedSleep can't alert you without notifications". Others: amber, skippable; later shown as readiness chip (§3.17). |
| 3    | S05    | Camera opens after a one-line rationale; paste field visible below.                                                                                       | Camera permission requested just in time.                          | §3.3                                                                                                                                                                     |
| 4    | S09    | Connecting → connected. **First-run coach** (3 callouts on the real screen): green bar, Listen, Test alert. Last callout: **Run test alert now** / Later. | Coach marks shown once; counts as indicator training (ADR-013 §5). | Skipped test → S09 shows "Test your alerts" chip until one test succeeds.                                                                                                |
| 5    | —      | Test alert flow §3.9.                                                                                                                                     |                                                                    |                                                                                                                                                                          |

Indicator training uses the real components on the real screen, not a demo (ADR-013 §5); the existing `IndicatorTrainingDemo` is retired.

### 3.3 Pairing and its failure paths

`STATUS: UNDER DISCUSSION`

| Case                                    | Detected by                            | Baby phone (S04)                                     | Parent phone (S05)                                                                                                                   | Recovery action                     |
| --------------------------------------- | -------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------- |
| Happy path                              | parent verified                        | "Connected to parent phone" → S07                    | "Connected to {name}" → S09 + coach                                                                                                  | —                                   |
| Camera permission denied                | OS                                     | —                                                    | Inline: "Camera not allowed" + **Allow camera** (opens app settings) + **Paste code instead** (focus field)                          | either button                       |
| Not a GuardedSleep code                 | parser                                 | —                                                    | "This isn't a GuardedSleep code. Scan the code on the baby phone."                                                                   | keep scanning                       |
| Code expired (window closed by timeout) | baby window / server reject            | QR dims, "This code has expired" + **Show new code** | "This code has expired. Tap Show new code on the baby phone, then scan again."                                                       | new code on baby                    |
| Wrong code (proof fails during pairing) | `pair_proof_mismatch` on first attempt | stays on QR, no change                               | "The code doesn't match. Scan the code that is on the baby phone right now."                                                         | rescan                              |
| Baby already paired                     | window closed by first verified parent | —                                                    | "This baby phone is already paired with another phone."                                                                              | forget on baby (§3.14) then re-pair |
| Phones can't reach each other           | no verified attempt after 20 s         | "Waiting for parent phone…"                          | "Can't find the baby phone yet. Make sure both phones are on the same Wi-Fi, or both use mobile data." + **Try again**; keeps trying | wait / fix network                  |
| Pasted text incomplete                  | parser                                 | —                                                    | "This code is incomplete. Copy the whole code from the baby phone."                                                                  | paste again                         |

Baby QR rendering: inverted (light modules on near-black, our scanner handles inversion), screen brightness ≤ 50 %; **Make code brighter** button for difficult cameras. Copy-code button shares the same invite text.

### 3.4 Start and stop monitoring

`STATUS: UNDER DISCUSSION`

| Action                   | Where                                | Interaction                  | Effect on this phone                              | Effect on the other phone                                                                       |
| ------------------------ | ------------------------------------ | ---------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Start (baby)             | S06 card "Start monitoring"          | one tap                      | S07 → kiosk after 30 s idle; silence applied      | parent auto-connects if monitoring                                                              |
| Start (parent)           | S06 card "Connect to {name}"         | one tap                      | S09, yellow "Connecting to {name}…"               | —                                                                                               |
| Stop (parent)            | S09 overflow → **Stop monitoring**   | confirm dialog `dialog_stop` | S06; ongoing notification removed                 | baby: "Parent phone stopped monitoring" (grey, no red pulse), keeps waiting for a parent        |
| Stop (baby)              | S08 unlock → S07 **Stop monitoring** | confirm dialog               | S06; silence restored, toast "Sounds are back on" | parent: S11 variant "{name} stopped monitoring" — one connection haptic + tone, no loop, **OK** |
| App swiped away / killed | —                                    | —                            | next launch resumes (ADR-017 §9)                  | normal escalation §3.6                                                                          |

Stop is never adjacent to Listen and never a single tap.

### 3.5 Saved pairing — one-tap resume

`STATUS: UNDER DISCUSSION`

1. Launch → monitoring flag set? → straight to S09/S07 with "Reconnecting…" (no tap at all).
2. Else S06 shows the saved card first (ui-design "returning user"); one tap starts.
3. Not connected after 15 s → bar stays in normal escalation; S09 shows **What can I do?** opening `sheet_help_reconnect` (checklist: baby phone on and showing GuardedSleep? same Wi-Fi or both on mobile data? restart the app on the baby phone). Never a dead "Failed" screen; the link keeps trying (ADR-017 §6).
4. `failed_needs_user` (pairing no longer valid): "These phones are no longer paired. Pair them again to keep monitoring." + **Pair again** → S05.

### 3.6 Reconnect and escalation on both phones

`STATUS: UNDER DISCUSSION` — timings measured from `last_alive` (ADR-017 §5). Liveness detection needs up to 25 s, so a silent loss enters the table at level 1.

| Time since last contact | Level | Parent bar                                                    | Parent other                                                                                                                                                                        | Parent notification (ongoing)             | Baby (S08 dark)                            | Baby on touch-wake                           |
| ----------------------- | ----- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | ------------------------------------------ | -------------------------------------------- |
| 0–20 s                  | 0     | yellow, ↻ icon, "Reconnecting…"                               | headline "Last heard {time}"; Listen disabled                                                                                                                                       | text "Reconnecting to {name}…"            | link icon yellow (dim)                     | "Reconnecting to parent phone…"              |
| 20–60 s                 | 1     | orange, broken-link icon, "Can't reach {name} — still trying" | same; **no vibration** (A1)                                                                                                                                                         | text updates, silent                      | icon orange (dim)                          | "Parent phone not responding — still trying" |
| 60–180 s                | 2     | red, slow pulse, "No connection for {duration}"               | **S11 full screen** over lock screen; connection haptic every 10 s; soft tone every 30 s (A2)                                                                                       | high-priority, full-screen intent         | **slow dim red pulse** (§6.2), never sound | same + "for {duration}"                      |
| > 180 s                 | 3     | red, "No connection for {duration}"                           | continuous alarm tone (alarm stream, user's alarm volume) + haptics until acknowledged or restored                                                                                  | same                                      | unchanged                                  | unchanged                                    |
| Restored                | —     | green, "Connected to {name}"                                  | if level ≥ 1: toast "Connected again after {duration}" (5 s); if level ≥ 2: S11 turns green "Connected again" + one short haptic, auto-closes after 10 s; then `card_missed_events` | back to "Connected to {name} · All quiet" | icon green                                 | "Connected to parent phone"                  |

Acknowledge (S11): **I'm checking** → sound and haptics off for 5 min, bar stays red, notification stays; after 5 min still down → level 3 directly (A3). **Stop monitoring** (secondary, confirm). Snooze never affects this table (ADR-017 §7).

Listen intent: if outage < 60 s, Listen resumes and toast "Connected again — listening"; otherwise Listen is off (A9).

### 3.7 Noise alert — trigger to dismissal

`STATUS: UNDER DISCUSSION`

| Step | Baby                                                     | Parent                                                                                                                                                                        |
| ---- | -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | Level above threshold for the hold time (default 5 s)    | —                                                                                                                                                                             |
| 2    | Sends alert; enables audio track; **no light, no sound** | Noise haptic; screen on; if app not in front → S10 over lock screen, else S09 in alert state; child audio on alarm stream via the chosen output (ADR-011); meter live         |
| 3    | —                                                        | S10 shows "Sound detected · since {time}", live meter, **Stop sound** (primary, 72 dp+), **Snooze 10 min**, **Snooze 30 min**, **Listen** stays available                     |
| 4a   | —                                                        | **Stop sound**: audio + haptics stop for this event; visual alert state stays until quiet for 2 min                                                                           |
| 4b   | Noise stops                                              | Audio stops 15 s after the level falls below threshold; headline "Sound at {time} · lasted {duration}"; after 2 min quiet the event ends and "All quiet since {time}" resumes |
| 5    | New noise within 2 min of the event's end                | Same event continues: audio resumes, one short haptic, no new full screen                                                                                                     |
| 6    | New noise ≥ 2 min after the event ended                  | New full alert (step 2)                                                                                                                                                       |

During snooze steps 2–6 are visual only (§3.10). A noise alert never hides the connection bar.

### 3.8 Listen (push-to-listen + slide-to-lock)

`STATUS: UNDER DISCUSSION`

| State    | Trigger                          | Button                                            | Audio | Feedback                                                      |
| -------- | -------------------------------- | ------------------------------------------------- | ----- | ------------------------------------------------------------- |
| idle     | —                                | headphone icon + "Hold to listen"                 | off   | first 3 uses: hint "Slide up to keep listening"               |
| holding  | press ≥ 150 ms                   | grows, waveform, "Listening…" + lock target above | on    | tick haptic on start                                          |
| locked   | slide ≥ 72 dp up while holding   | lock icon, "Listening · tap to stop"              | on    | tick haptic on lock; "Keep screen on while listening" applies |
| stopped  | release (holding) / tap (locked) | idle                                              | off   | —                                                             |
| disabled | not connected                    | greyed, "Listen when connected"                   | off   | —                                                             |

Locked listening auto-stops after 60 min with a toast "Listening stopped after 1 h" (prevents a forgotten open mic). The baby shows "Parent is listening" only on touch-wake (no light while dark).

### 3.9 Test alert (probe)

`STATUS: UNDER DISCUSSION`

1. S09 **Test alert** (secondary button) → `sheet_probe`: "This plays a test sound **on this phone**. The baby phone stays silent." + output line per ADR-011 + **Run test**.
2. Parent requests probe → baby injects a digital test event through its alert pipeline (A10) → parent runs the full noise-alert chain with a bundled test sound, labelled **TEST** on S10.
3. After 5 s: "Did you hear and feel it?" **Yes** / **No**.
4. Yes → "Last test: today" stored, shown in settings. No → `sheet_readiness` with fixes (alarm volume, alert output, lock-screen alerts, Do Not Disturb).
5. Not connected → test runs locally only and says "Tested this phone only — the baby phone is not connected".
6. Prompted again: after app update, after 30 days, before first use on mobile data.

### 3.10 Snooze

`STATUS: UNDER DISCUSSION`

| Rule                   | Value                                                                                                                                     |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Durations              | 10 min, 30 min (ADR-018); no "forever"                                                                                                    |
| Entry                  | S09 **Snooze** button → `sheet_snooze`; S10 direct buttons                                                                                |
| What it silences       | noise-alert sound, haptics, full screen                                                                                                   |
| What it never silences | connection levels 2–3, baby stopped, battery warnings, readiness warnings                                                                 |
| While snoozed          | meter live; headline "Sound at {time} · alerts snoozed" on noise; chip "Snoozed · {minutes} min left" with **End snooze**; events counted |
| End                    | automatic or **End snooze**; if noise is above threshold at that moment → full alert immediately                                          |
| Persistence            | stored as "until" time; survives reconnects and app restarts                                                                              |
| Listen                 | unchanged by snooze                                                                                                                       |

### 3.11 Low battery (both phones)

`STATUS: UNDER DISCUSSION`

| Condition                                   | Parent shows                                                            | Haptic / sound                    | Clears when                |
| ------------------------------------------- | ----------------------------------------------------------------------- | --------------------------------- | -------------------------- |
| Baby < 15 %, not charging                   | amber chip "Baby phone battery {percent} — plug it in" + notification   | one short haptic                  | charging or > 20 %         |
| Baby < 5 % (always)                         | red chip + S11 variant once "Baby phone battery almost empty"           | connection haptic + one soft tone | charging                   |
| Baby stopped charging while monitoring (A7) | info chip "Baby phone is no longer charging"                            | one short haptic                  | charging again / dismissed |
| Parent < 20 %, not charging (A7)            | amber chip "Your phone is at {percent} — plug it in to keep monitoring" | one short haptic                  | charging                   |
| Parent < 10 %, not charging                 | same, red                                                               | one short haptic, repeated at 5 % | charging                   |

Baby side shows battery only on touch-wake. Battery warnings cannot be snoozed but never loop.

### 3.12 Baby silence status and remote silence

`STATUS: UNDER DISCUSSION`

Derived on the baby from `device_status` (ADR-018 §3.2) into one parent-visible value:

| Value              | Rule                                                                                        | Parent chip (icon + text)                                        | Tone                                                 | Action in `sheet_silence`                                                                |
| ------------------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `silent`           | DND active blocking calls, messages, reminders; visual effects suppressed; or ringer silent | moon · "Baby phone is silent"                                    | neutral                                              | sub-line "Alarm clocks on it can still ring"; **Allow sounds again** (secondary)         |
| `screen_may_light` | silent, but visual effects not suppressed                                                   | moon + sun · "Baby phone is silent, but its screen may light up" | amber                                                | **Silence it fully** (if access) / manual steps                                          |
| `may_vibrate`      | ringer vibrate, no DND                                                                      | bell · "Baby phone may buzz"                                     | amber                                                | **Silence baby phone** / manual steps                                                    |
| `may_ring`         | ringer normal, no DND                                                                       | bell · "Baby phone may ring"                                     | amber                                                | **Silence baby phone** / manual steps                                                    |
| `unknown`          | browser baby, or status not yet received                                                    | question · "Can't check if the baby phone is silent"             | neutral grey (not amber, avoids a permanent warning) | manual steps                                                                             |
| `no_access`        | native baby without DND access                                                              | as `may_*`, action replaced                                      | amber                                                | "Allow GuardedSleep to silence it: on the baby phone open Settings → Silence this phone" |

Remote silence: **Silence baby phone** → chip shows "Silencing…" (max 10 s) → result chip, or "Couldn't silence the baby phone. {reason}" with the manual steps. Baby applies DND "alarms only" + suppressed visual effects (A5), stores previous mode, restores on stop. If restore was missed (crash), the baby's next launch restores and shows "This phone was left on silent. Sounds are back on." A baby phone that is someone's everyday phone shows at start (S07): "Calls and messages on this phone stay silent until you stop monitoring."

### 3.13 Settings — Basic / Advanced

`STATUS: UNDER DISCUSSION`

| Group (Basic)                   | Parent phone                                                                                                                                                                              | Baby phone (reachable only after kiosk unlock)                                                                                         |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Alerts / Sound detection        | Alert sound plays on [On this phone's current output · Speaker only]; Alert volume boost (slider 25–150 %); Keep alert volume up during alerts (toggle); Test alert + "Last test: {date}" | Sensitivity slider with live meter + threshold line ("More sensitive ↔ Less sensitive"); Noise must last 3 / 5 / 10 / 15 s (default 5) |
| Silence                         | —                                                                                                                                                                                         | Silence this phone while monitoring (default on when access granted); status line                                                      |
| Screen                          | Theme (Dark default / Light / Match system); Keep screen on while listening                                                                                                               | While monitoring: Dim (default) / Off                                                                                                  |
| Paired phone                    | {name}: Rename · Forget this baby phone                                                                                                                                                   | Forget this parent phone                                                                                                               |
| This phone                      | Readiness checklist (§3.17); Battery & background                                                                                                                                         | Device check (run again); Battery & background; safety note                                                                            |
| Language                        | ADR-021 §2 set (~20 AI-translated locales, en master; previously en / de / fr / es, superseded, ADR-021 §2)                                                                               | same                                                                                                                                   |
| About & help                    | Limits (F-1/F-4 copy), help, version, privacy                                                                                                                                             | same                                                                                                                                   |
| **Advanced** (toggle + warning) | Connection server address, relay (TURN), Heartbeat 5–30 s, Always show connection details, Diagnostics export                                                                             | same                                                                                                                                   |

### 3.14 Forget pairing

`STATUS: UNDER DISCUSSION`

1. Settings → Paired phone → **Forget this baby phone** → `dialog_forget`: "Forget {name}? Monitoring stops and you'll need to scan a new code to connect again." [Forget] [Cancel].
2. Stops monitoring if active, deletes the key from secure storage, returns to S06 empty state.
3. Other phone on its next attempt: `failed_needs_user` → §3.5 step 4 copy. No alarm sound for this terminal state on the parent beyond one connection haptic.

### 3.15 Missed events after an outage

`STATUS: UNDER DISCUSSION`

| Summary received            | `card_missed_events` text                                                         | Headline afterwards                      |
| --------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------- |
| count > 0                   | "While disconnected: {count} sounds between {start} and {end}. Loudest: {level}." | "Sound at {lastTime}"                    |
| count = 0, gap covered      | "No sounds while disconnected."                                                   | "All quiet since {previous time}"        |
| no summary / baby restarted | "We don't know what happened between {start} and {end}."                          | "All quiet since {reconnect time}" (A11) |

Card stays until **OK**; also logged in the parent's event list (T2).

### 3.16 Web client deltas

`STATUS: UNDER DISCUSSION`

| Topic                | Web parent                                                                                                                                                                                                                                  | Web baby                                                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Sound unlock         | Monitoring starts with a **Tap to turn on alert sound** gate (browser autoplay rule)                                                                                                                                                        | Tap to allow microphone (existing)                                                        |
| Background           | Persistent banner: "Keep this page open and the screen on. Your browser can't alert you otherwise."; Screen Wake Lock requested; if hidden tab is detected → on return show "This page was in the background — alerts may have been missed" | Same banner; Fullscreen API + dark overlay instead of kiosk                               |
| Full screen alert    | In-page S10/S11, document title flashes "Sound detected", Notification API if granted                                                                                                                                                       | —                                                                                         |
| Silence              | n/a                                                                                                                                                                                                                                         | reports `unknown`; S07 shows "Turn on Do Not Disturb and lower brightness on this device" |
| Battery optimisation | rows hidden                                                                                                                                                                                                                                 | rows hidden                                                                               |

### 3.17 Parent readiness check

`STATUS: UNDER DISCUSSION` (A6)

Checked at start of monitoring, on app resume, and every 5 min. The most severe failing item shows as one chip on S09 (`parent_readiness_chip`); tapping opens `sheet_readiness` with all items and fix buttons.

| Item                                       | Severity | Chip / row text                                      | Fix button                                            |
| ------------------------------------------ | -------- | ---------------------------------------------------- | ----------------------------------------------------- |
| Notifications blocked                      | red      | "Alerts are turned off for GuardedSleep"             | Turn on alerts                                        |
| Alarm volume is 0                          | red      | "Your alarm volume is off — you won't hear alerts"   | (instruction: press volume up during test) Test alert |
| Lock-screen alerts not allowed             | amber    | "Alerts can't appear on the lock screen"             | Allow                                                 |
| DND on and bypass not allowed              | amber    | "Do Not Disturb may silence alerts"                  | Allow alerts during Do Not Disturb                    |
| Battery optimisation active                | amber    | "Your phone may stop GuardedSleep in the background" | Allow background use                                  |
| Headphones connected + Default output (T2) | info     | "Alerts will play in your headphones"                | Change                                                |
| Never tested / > 30 days                   | info     | "Test your alerts"                                   | Test alert                                            |

---

## 4. Screen specifications

Every screen: states, primary action, at-a-glance content, test IDs, message keys. English copy is the `en` master.

### 4.1 S01 `role`

`STATUS: UNDER DISCUSSION` — States: default. Primary: pick a card. At a glance: two cards, 120 dp each.

| Test ID           | Key                 | English                            |
| ----------------- | ------------------- | ---------------------------------- |
| `role_title`      | `role.title`        | Which phone is this?               |
| `role_baby`       | `role.baby.title`   | Baby phone                         |
|                   | `role.baby.hint`    | Stays with your child and listens. |
| `role_parent`     | `role.parent.title` | Parent phone                       |
|                   | `role.parent.hint`  | Stays with you and alerts you.     |
| `role_order_hint` | `role.orderHint`    | Set up the baby phone first.       |

### 4.2 S02 `baby_setup` / S03 `parent_setup`

`STATUS: UNDER DISCUSSION` — States per row: `done` (green ✓), `todo` (amber !), `blocked` (red ✕), `checking` (spinner). Primary: **Continue** (enabled when required rows are done).

| Test ID                   | Key                                | English                                                                                                                                      |
| ------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `setup_title`             | `setup.title`                      | Get this phone ready                                                                                                                         |
| `setup_row_mic`           | `setup.mic.title`                  | Microphone                                                                                                                                   |
|                           | `setup.mic.why`                    | Needed to hear your child.                                                                                                                   |
| `setup_row_background`    | `setup.background.title`           | Keep running in the background                                                                                                               |
|                           | `setup.background.why`             | Otherwise your phone may stop monitoring to save battery.                                                                                    |
| `setup_row_silence`       | `setup.silence.title`              | Silence this phone while monitoring                                                                                                          |
|                           | `setup.silence.why`                | Calls and messages won't ring or light up the screen next to your child. Alarm clocks still work.                                            |
| `setup_row_safety`        | `setup.safety.body`                | Keep this phone on the charger, on a stable surface, out of your child's reach. Don't use a phone with a swollen or damaged battery.         |
| `setup_row_device_check`  | `setup.deviceCheck.ok`             | This phone works well as a baby phone.                                                                                                       |
|                           | `setup.deviceCheck.limited`        | This phone may lose the connection now and then. It still works.                                                                             |
|                           | `setup.deviceCheck.notRecommended` | This phone may not monitor reliably. Use another phone if you can.                                                                           |
| `setup_row_notifications` | `setup.notifications.title`        | Alerts                                                                                                                                       |
|                           | `setup.notifications.why`          | Needed so you hear when your child makes noise.                                                                                              |
|                           | `setup.notifications.required`     | GuardedSleep can't alert you without this.                                                                                                   |
| `setup_row_lockscreen`    | `setup.lockScreen.title`           | Alerts on the lock screen                                                                                                                    |
| `setup_row_dnd_bypass`    | `setup.dndBypass.title`            | Alerts during Do Not Disturb                                                                                                                 |
| `setup_limits_card`       | `setup.limits.body`                | GuardedSleep can only alert you while this phone is on and charged. Keep it plugged in at night. If both phones turn off, nobody is alerted. |
| `setup_row_allow`         | `common.allow`                     | Allow                                                                                                                                        |
| `setup_continue`          | `common.continue`                  | Continue                                                                                                                                     |
| `setup_skip_row`          | `common.later`                     | Later                                                                                                                                        |

### 4.3 S04 `baby_pair`

`STATUS: UNDER DISCUSSION` — States: `showing`, `expired`, `connecting` (parent found), `connected`. Primary: none while showing (wait); **Show new code** when expired.

| Test ID                 | Key                     | English                                                                   |
| ----------------------- | ----------------------- | ------------------------------------------------------------------------- |
| `baby_pair_title`       | `pair.baby.title`       | Connect your phone                                                        |
| `baby_pair_qr`          | `pair.baby.qrAlt`       | Pairing code                                                              |
| `baby_pair_instruction` | `pair.baby.instruction` | On your phone, open GuardedSleep, choose Parent phone and scan this code. |
| `baby_pair_countdown`   | `pair.baby.validFor`    | Code valid for {duration}                                                 |
| `baby_pair_expired`     | `pair.baby.expired`     | This code has expired.                                                    |
| `baby_pair_new_code`    | `pair.baby.newCode`     | Show new code                                                             |
| `baby_pair_copy`        | `pair.baby.copy`        | Copy code                                                                 |
| `baby_pair_brighter`    | `pair.baby.brighter`    | Make code brighter                                                        |
| `baby_pair_waiting`     | `pair.baby.waiting`     | Waiting for parent phone…                                                 |
| `baby_pair_privacy`     | `pair.baby.privacy`     | Anyone with this code can listen. Only show it to people you trust.       |

### 4.4 S05 `parent_pair`

`STATUS: UNDER DISCUSSION` — States: `camera_rationale`, `scanning`, `camera_denied`, `connecting`, `error(kind)`. Primary: scan; secondary: paste.

| Test ID                     | Key                        | English                                                                                              |
| --------------------------- | -------------------------- | ---------------------------------------------------------------------------------------------------- |
| `parent_pair_title`         | `pair.parent.title`        | Scan the code on the baby phone                                                                      |
| `parent_pair_camera_why`    | `pair.parent.cameraWhy`    | GuardedSleep uses the camera only to read the code.                                                  |
| `parent_pair_scanner`       | —                          | (camera view)                                                                                        |
| `parent_pair_paste_field`   | `pair.parent.pasteLabel`   | Or paste the code                                                                                    |
| `parent_pair_paste_submit`  | `pair.parent.pasteSubmit`  | Connect                                                                                              |
| `parent_pair_camera_denied` | `pair.error.cameraDenied`  | Camera not allowed.                                                                                  |
| `parent_pair_allow_camera`  | `pair.error.allowCamera`   | Allow camera                                                                                         |
| `parent_pair_paste_instead` | `pair.error.pasteInstead`  | Paste code instead                                                                                   |
| `parent_pair_error`         | `pair.error.notOurs`       | This isn't a GuardedSleep code. Scan the code on the baby phone.                                     |
|                             | `pair.error.expired`       | This code has expired. Tap Show new code on the baby phone, then scan again.                         |
|                             | `pair.error.mismatch`      | The code doesn't match. Scan the code that is on the baby phone right now.                           |
|                             | `pair.error.alreadyPaired` | This baby phone is already paired with another phone.                                                |
|                             | `pair.error.unreachable`   | Can't find the baby phone yet. Make sure both phones are on the same Wi-Fi, or both use mobile data. |
|                             | `pair.error.incomplete`    | This code is incomplete. Copy the whole code from the baby phone.                                    |
| `parent_pair_retry`         | `common.tryAgain`          | Try again                                                                                            |
| `parent_pair_connecting`    | `pair.parent.connecting`   | Connecting to the baby phone…                                                                        |

### 4.5 S06 `home`

`STATUS: UNDER DISCUSSION` — States: `saved` (card), `empty`. Primary: the card (≥ 96 dp, full width).

| Test ID            | Key                     | English               |
| ------------------ | ----------------------- | --------------------- |
| `home_card`        | `home.parent.cardTitle` | Connect to {name}     |
|                    | `home.baby.cardTitle`   | Start monitoring      |
| `home_card_last`   | `home.lastUsed`         | Last used {time}      |
| `home_new_pairing` | `home.newPairing`       | Set up new phones     |
| `home_settings`    | `common.settings`       | Settings              |
| `home_empty`       | `home.empty`            | No phones paired yet. |

### 4.6 S07 `baby_monitor` (awake / unlocked)

`STATUS: UNDER DISCUSSION` — States: `waiting_parent`, `connected`, `reconnecting`, `parent_stopped`, `needs_pairing`. Primary: **Lock screen** (auto after 30 s idle). At a glance: big status line, parent phone, battery, silence status. Dark theme only.

| Test ID                | Key                          | English                                                                 |
| ---------------------- | ---------------------------- | ----------------------------------------------------------------------- |
| `baby_status`          | `baby.status.connected`      | Connected to parent phone                                               |
|                        | `baby.status.waiting`        | Waiting for parent phone…                                               |
|                        | `baby.status.reconnecting`   | Reconnecting to parent phone…                                           |
|                        | `baby.status.notResponding`  | Parent phone not responding — still trying                              |
|                        | `baby.status.parentStopped`  | Parent phone stopped monitoring                                         |
|                        | `link.needsPairing`          | These phones are no longer paired. Pair them again to keep monitoring.  |
| `baby_monitoring_line` | `baby.monitoring`            | Monitoring                                                              |
| `baby_listening`       | `baby.parentListening`       | Parent is listening                                                     |
| `baby_battery`         | `baby.battery`               | Battery {percent}                                                       |
|                        | `baby.batteryCharging`       | Charging · {percent}                                                    |
| `baby_silence`         | `baby.silence.on`            | This phone is silent                                                    |
|                        | `baby.silence.off`           | This phone may ring                                                     |
|                        | `baby.silence.everydayPhone` | Calls and messages on this phone stay silent until you stop monitoring. |
|                        | `baby.silence.restored`      | This phone was left on silent. Sounds are back on.                      |
| `baby_lock`            | `baby.lock`                  | Lock screen                                                             |
| `baby_autolock_hint`   | `baby.autolockHint`          | The screen goes dark in {seconds} s                                     |
| `baby_stop`            | `baby.stop`                  | Stop monitoring                                                         |
| `baby_settings`        | `common.settings`            | Settings                                                                |
| `baby_mic_level`       | `baby.micLevel`              | Sound level here                                                        |

### 4.7 S08 `baby_kiosk`

`STATUS: UNDER DISCUSSION` — States: `dark`, `woken` (3 s), `unlocking` (finger down), `pulse` (level ≥ 2). No primary action (hold-and-slide unlock). At a glance (woken only): link icon + battery + one status line from §4.6.

| Test ID                  | Key                      | English                      |
| ------------------------ | ------------------------ | ---------------------------- |
| `baby_kiosk`             | `kiosk.a11yLabel`        | Monitoring. Screen locked.   |
| `baby_kiosk_link_icon`   | (reuses `baby.status.*`) |                              |
| `baby_kiosk_battery`     | `baby.battery`           | Battery {percent}            |
| `baby_kiosk_hint`        | `kiosk.unlockHint`       | Hold and slide up to unlock  |
| `baby_kiosk_unlock_zone` | `kiosk.unlockZone`       | Slide here                   |
| `baby_kiosk_relock`      | `kiosk.relockWarning`    | Locking again in {seconds} s |

### 4.8 S09 `parent_monitor`

`STATUS: UNDER DISCUSSION` — States: `connecting`, `connected_quiet`, `connected_noise_below`, `alert` (if S10 not shown), `snoozed`, `reconnecting(level)`, `baby_stopped`, `needs_pairing`. Primary: **Listen**. Layout §5.

| Test ID                    | Key                                      | English                                                                |
| -------------------------- | ---------------------------------------- | ---------------------------------------------------------------------- |
| `parent_ambient_bar`       | `link.connected`                         | Connected to {name}                                                    |
|                            | `link.connecting`                        | Connecting to {name}…                                                  |
|                            | `link.reconnecting`                      | Reconnecting…                                                          |
|                            | `link.cantReach`                         | Can't reach {name} — still trying                                      |
|                            | `link.lostFor`                           | No connection for {duration}                                           |
|                            | `link.babyStopped`                       | {name} stopped monitoring                                              |
|                            | `link.needsPairing`                      | These phones are no longer paired. Pair them again to keep monitoring. |
| `parent_headline`          | `parent.allQuietSince`                   | All quiet since {time}                                                 |
|                            | `parent.soundSince`                      | Sound detected · since {time}                                          |
|                            | `parent.soundAt`                         | Sound at {time} · lasted {duration}                                    |
|                            | `parent.soundSnoozed`                    | Sound at {time} · alerts snoozed                                       |
|                            | `parent.lastHeard`                       | Last heard {time}                                                      |
| `parent_meter`             | `parent.meterA11y`                       | Sound level: {level}                                                   |
|                            | `parent.level.quiet` / `.some` / `.loud` | quiet / some sound / loud                                              |
| `parent_baby_battery_chip` | `battery.baby`                           | Baby phone {percent}                                                   |
| `parent_silence_chip`      | `silence.*` (§4.11)                      |                                                                        |
| `parent_quality_chip`      | `link.weak`                              | Weak connection — sound may break up                                   |
| `parent_snooze_chip`       | `snooze.active`                          | Snoozed · {minutes} min left                                           |
| `parent_readiness_chip`    | `readiness.*` (§3.17)                    |                                                                        |
| `parent_listen`            | `listen.idle`                            | Hold to listen                                                         |
|                            | `listen.holding`                         | Listening…                                                             |
|                            | `listen.locked`                          | Listening · tap to stop                                                |
|                            | `listen.lockHint`                        | Slide up to keep listening                                             |
|                            | `listen.disabled`                        | Listen when connected                                                  |
|                            | `listen.autoStopped`                     | Listening stopped after 1 h                                            |
|                            | `listen.resumed`                         | Connected again — listening                                            |
| `parent_snooze`            | `snooze.button`                          | Snooze                                                                 |
| `parent_probe`             | `probe.button`                           | Test alert                                                             |
| `parent_overflow`          | `common.more`                            | More                                                                   |
| `parent_stop`              | `parent.stop`                            | Stop monitoring                                                        |
| `parent_help_reconnect`    | `help.reconnect.open`                    | What can I do?                                                         |
| `parent_restored_toast`    | `link.restoredAfter`                     | Connected again after {duration}                                       |
| `parent_test_chip`         | `readiness.neverTested`                  | Test your alerts                                                       |

### 4.9 S10 `parent_noise_alert`

`STATUS: UNDER DISCUSSION` — Shown over the lock screen (native full-screen intent) or in page (web). States: `sounding`, `sound_stopped`, `test` (label TEST), `ended`. Primary: **Stop sound**. At a glance: dark-red background (never white), headline, live meter, connection bar on top.

| Test ID            | Key               | English              |
| ------------------ | ----------------- | -------------------- |
| `alert_title`      | `alert.title`     | Sound detected       |
| `alert_since`      | `alert.since`     | Since {time}         |
| `alert_test_badge` | `alert.testBadge` | TEST                 |
| `alert_stop_sound` | `alert.stopSound` | Stop sound           |
| `alert_snooze_10`  | `snooze.minutes`  | Snooze {minutes} min |
| `alert_snooze_30`  | `snooze.minutes`  | Snooze {minutes} min |
| `alert_listen`     | `listen.idle`     | Hold to listen       |
| `alert_open_app`   | `alert.openApp`   | Open GuardedSleep    |

### 4.10 S11 `parent_link_alarm`

`STATUS: UNDER DISCUSSION` — Variants: `lost` (level 2/3), `baby_stopped`, `battery_critical`, `restored`. Primary: **I'm checking** (lost) / **OK** (others).

| Test ID               | Key                        | English                                                                                                                  |
| --------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `link_alarm_title`    | `linkAlarm.lost.title`     | No connection to {name}                                                                                                  |
| `link_alarm_body`     | `linkAlarm.lost.body`      | For {duration}. You won't hear your child until it reconnects. Check that the baby phone is on and showing GuardedSleep. |
|                       | `linkAlarm.stopped.title`  | {name} stopped monitoring                                                                                                |
|                       | `linkAlarm.stopped.body`   | Someone stopped monitoring on the baby phone. You won't get alerts.                                                      |
|                       | `linkAlarm.battery.title`  | Baby phone battery almost empty                                                                                          |
|                       | `linkAlarm.battery.body`   | Plug in the baby phone now, or monitoring will stop.                                                                     |
|                       | `linkAlarm.restored.title` | Connected again                                                                                                          |
| `link_alarm_ack`      | `linkAlarm.ack`            | I'm checking                                                                                                             |
| `link_alarm_ack_note` | `linkAlarm.ackNote`        | Sound off for 5 min. We'll alert you again if it's still not connected.                                                  |
| `link_alarm_ok`       | `common.ok`                | OK                                                                                                                       |
| `link_alarm_stop`     | `parent.stop`              | Stop monitoring                                                                                                          |
| `link_alarm_help`     | `help.reconnect.open`      | What can I do?                                                                                                           |

### 4.11 Sheets and dialogs

`STATUS: UNDER DISCUSSION`

| Test ID                    | Key                          | English                                                                                                          |
| -------------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `sheet_silence`            | `silence.silent`             | Baby phone is silent                                                                                             |
|                            | `silence.alarmsStillRing`    | Alarm clocks on it can still ring.                                                                               |
|                            | `silence.screenMayLight`     | Baby phone is silent, but its screen may light up                                                                |
|                            | `silence.mayVibrate`         | Baby phone may buzz                                                                                              |
|                            | `silence.mayRing`            | Baby phone may ring                                                                                              |
|                            | `silence.unknown`            | Can't check if the baby phone is silent                                                                          |
|                            | `silence.manual`             | On the baby phone, turn on Do Not Disturb and turn the brightness down.                                          |
|                            | `silence.noAccess`           | To silence it from here, open Settings on the baby phone and turn on "Silence this phone while monitoring".      |
| `sheet_silence_apply`      | `silence.apply`              | Silence baby phone                                                                                               |
| `sheet_silence_apply_full` | `silence.applyFull`          | Silence it fully                                                                                                 |
| `sheet_silence_pending`    | `silence.pending`            | Silencing…                                                                                                       |
| `sheet_silence_failed`     | `silence.failed`             | Couldn't silence the baby phone.                                                                                 |
| `sheet_silence_undo`       | `silence.allowSounds`        | Allow sounds again                                                                                               |
| `sheet_snooze`             | `snooze.title`               | Snooze noise alerts                                                                                              |
|                            | `snooze.help`                | You still get alerts if the connection drops or a battery runs low.                                              |
| `sheet_snooze_10` / `_30`  | `snooze.minutes`             | Snooze {minutes} min                                                                                             |
| `sheet_snooze_end`         | `snooze.end`                 | End snooze                                                                                                       |
| `sheet_probe`              | `probe.title`                | Test your alerts                                                                                                 |
|                            | `probe.body`                 | This plays a test sound on this phone. The baby phone stays silent.                                              |
|                            | `probe.output`               | Alerts play on: {output}                                                                                         |
| `sheet_probe_run`          | `probe.run`                  | Run test                                                                                                         |
| `sheet_probe_question`     | `probe.question`             | Did you hear and feel it?                                                                                        |
| `sheet_probe_yes` / `_no`  | `common.yes` / `common.no`   | Yes / No                                                                                                         |
|                            | `probe.localOnly`            | Tested this phone only — the baby phone is not connected.                                                        |
| `sheet_readiness`          | `readiness.title`            | Can alerts reach you?                                                                                            |
|                            | `readiness.notificationsOff` | Alerts are turned off for GuardedSleep                                                                           |
|                            | `readiness.alarmVolumeZero`  | Your alarm volume is off — you won't hear alerts                                                                 |
|                            | `readiness.lockScreen`       | Alerts can't appear on the lock screen                                                                           |
|                            | `readiness.dnd`              | Do Not Disturb may silence alerts                                                                                |
|                            | `readiness.battery`          | Your phone may stop GuardedSleep in the background                                                               |
|                            | `readiness.headphones`       | Alerts will play in your headphones                                                                              |
|                            | `readiness.ok`               | All set — alerts can reach you                                                                                   |
| `sheet_help_reconnect`     | `help.reconnect.title`       | Getting connected again                                                                                          |
|                            | `help.reconnect.step1`       | Is the baby phone on and showing GuardedSleep?                                                                   |
|                            | `help.reconnect.step2`       | Are both phones on the same Wi-Fi, or both using mobile data?                                                    |
|                            | `help.reconnect.step3`       | Still stuck? Close and reopen GuardedSleep on the baby phone.                                                    |
| `dialog_stop`              | `stop.title`                 | Stop monitoring?                                                                                                 |
|                            | `stop.parentBody`            | You won't get alerts from {name}.                                                                                |
|                            | `stop.babyBody`              | The parent phone will stop getting alerts.                                                                       |
| `dialog_stop_confirm`      | `stop.confirm`               | Stop                                                                                                             |
| `dialog_stop_cancel`       | `stop.cancel`                | Keep monitoring                                                                                                  |
| `dialog_forget`            | `forget.title`               | Forget {name}?                                                                                                   |
|                            | `forget.body`                | Monitoring stops and you'll need to scan a new code to connect again.                                            |
| `dialog_forget_confirm`    | `forget.confirm`             | Forget                                                                                                           |
| `card_missed_events`       | `missed.some`                | While disconnected: {count, plural, one {1 sound} other {# sounds}} between {start} and {end}. Loudest: {level}. |
|                            | `missed.none`                | No sounds while disconnected.                                                                                    |
|                            | `missed.unknown`             | We don't know what happened between {start} and {end}.                                                           |
| `coach_first_run`          | `coach.bar`                  | Green means you're monitoring. If it turns red or disappears, you are not.                                       |
|                            | `coach.listen`               | Hold to listen. Slide up to keep listening.                                                                      |
|                            | `coach.test`                 | Run a test alert so you know what an alert feels like.                                                           |
|                            | `coach.runTest`              | Run test alert now                                                                                               |

### 4.12 Battery copy

`STATUS: UNDER DISCUSSION`

| Test ID                    | Key                       | English                                                    |
| -------------------------- | ------------------------- | ---------------------------------------------------------- |
| `parent_baby_battery_chip` | `battery.babyLow`         | Baby phone battery {percent} — plug it in                  |
|                            | `battery.babyNotCharging` | Baby phone is no longer charging                           |
| `parent_own_battery_chip`  | `battery.parentLow`       | Your phone is at {percent} — plug it in to keep monitoring |

### 4.13 Native notifications

`STATUS: UNDER DISCUSSION`

| Channel (id)       | Role   | Importance / behaviour                                                                   | Title key / English                                     | Text key / English                                                                         | Actions                    |
| ------------------ | ------ | ---------------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------ | -------------------------- |
| `monitoring`       | parent | low, ongoing, silent; text follows link state                                            | `notif.monitoring.title` GuardedSleep                   | `link.*` + `parent.allQuietSince` (e.g. "Connected to Baby phone · All quiet since 22:14") | Open                       |
| `noise_alert`      | parent | high, full-screen, bypass DND; audio played by app on alarm stream                       | `alert.title` Sound detected                            | `alert.since`                                                                              | Stop sound · Snooze 10 min |
| `connection_alert` | parent | high, full-screen, bypass DND (levels 2–3, baby stopped, battery < 5 %)                  | `linkAlarm.*.title`                                     | `linkAlarm.*.body`                                                                         | I'm checking / OK          |
| `warnings`         | parent | default, one short haptic, no sound                                                      | e.g. `battery.babyLow`                                  | —                                                                                          | Open                       |
| `info`             | parent | low, silent (restored, missed events)                                                    | `link.restoredAfter`                                    | `missed.*`                                                                                 | —                          |
| `baby_monitoring`  | baby   | **min, silent, no vibration, no lights, no heads-up, not on lock screen** (ADR-018 §3.1) | `notif.babyMonitoring.title` GuardedSleep is monitoring | —                                                                                          | —                          |

---

## 5. Parent monitoring screen layout priorities

`STATUS: UNDER DISCUSSION`

Readable from 1–2 m: **bar colour + headline**. Everything else is secondary.

| Order (top → bottom) | Element                                   | Size / rule                                                                                                                | Visible                                                                              |
| -------------------- | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 1                    | `parent_ambient_bar`                      | full width, ≥ 56 dp, icon + text, never hidden while monitoring                                                            | always                                                                               |
| 2                    | `parent_headline`                         | 24 sp+, one line (wraps at large font)                                                                                     | always; never "All quiet" while not connected                                        |
| 3                    | `parent_meter`                            | ≥ 120 dp high, 5–8 bars, threshold line                                                                                    | always (flat + "Last heard" when not connected)                                      |
| 4                    | Chip row (max 2 visible, "+N")            | priority: readiness red > baby battery critical > own battery > silence warning > snooze > weak connection > test reminder | only when not "all fine", except baby battery % and silence status (always, neutral) |
| 5                    | `parent_listen`                           | ≥ 72 dp high, ≥ 80 % width, bottom third (thumb zone)                                                                      | always (disabled when not connected)                                                 |
| 6                    | Secondary row: Snooze · Test alert · More | ≥ 48 dp each, ≥ 24 dp gap from Listen; Stop monitoring only inside More                                                    | always                                                                               |

Full-screen priority when several things happen: connection lost (S11) > noise alert (S10) > baby stopped > battery critical. Only one full-screen at a time; the lower one becomes a chip.

Tablet / landscape: two columns — left: bar + headline + meter; right: chips + Listen + secondary row.

---

## 6. Accessibility and dark-room rules

### 6.1 Accessibility

`STATUS: UNDER DISCUSSION`

| Rule                    | Requirement                                                                                                                                                                                                                                       |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Colour + symbol + text  | Every state uses all three: connected = green + link icon + text; reconnecting = yellow + ↻; can't reach = orange + broken link; lost = red + broken link + "!"; stopped = grey + pause; snoozed = amber + "zZ"                                   |
| Contrast                | WCAG 2.1 AA (4.5:1 text, 3:1 icons) in dark theme; the night palette in `ui-design.md` is raised where it fails (e.g. `#666666` secondary text on black fails)                                                                                    |
| Touch targets           | ≥ 48 dp everywhere; primary actions ≥ 72 dp; ≥ 8 dp spacing; destructive actions never next to Listen                                                                                                                                             |
| Screen reader           | `parent_ambient_bar` = live region (polite for levels 0–1, assertive for ≥ 2 and alerts); meter exposes text level (`parent.level.*`), not numbers; icons have `contentDescription` from the catalogue                                            |
| Listen without gestures | TalkBack double-tap toggles locked listening; custom action "Listen while holding" not required                                                                                                                                                   |
| Kiosk unlock            | TalkBack users get a custom action "Unlock" after a 2 s long-press confirmation                                                                                                                                                                   |
| Font scaling            | Up to 200 % without truncating bar text or headline (bar grows in height)                                                                                                                                                                         |
| Reduced motion          | Pulses become static colour + icon; meter still updates                                                                                                                                                                                           |
| Haptics                 | Distinct patterns (§1.2); every haptic also has a visual state                                                                                                                                                                                    |
| Languages               | All alert, connection and silence keys present in every shipped locale (ADR-015 CI rule, ADR-021 §2 completeness rule; de and ar are the screenshot locales per ADR-021 §2); layouts tested with the longest locale and pseudo-locale (Roborazzi) |

### 6.2 Dark-room rules (baby phone)

`STATUS: UNDER DISCUSSION`

| Rule                   | Requirement                                                                                                                                     |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| No sound, no vibration | Ever, in every state (ADR-018 §3.1); includes setup, errors, pairing, stop                                                                      |
| Kiosk brightness       | Window brightness override at the device minimum (`0.01`); background `#000000`; icons ≤ 12 dp in dim colours                                   |
| Touch-wake             | 3 s, window brightness ≤ 10 %, text in dim grey/amber, fades in and out over ≥ 1 s                                                              |
| Connection pulse       | Level ≥ 2 only; dark red (peak `#330000`) on near-black, 10 s sinusoidal period, no hard edges, brightness stays at minimum                     |
| No flashes             | No transition faster than 1 s; no white or saturated full-screen colour on the baby side; QR screen ≤ 50 % brightness, inverted code by default |
| Screen-off option      | "While monitoring: Off" turns the display off; monitoring continues; touch-wake still works                                                     |
| Other apps             | Silence policy suppresses screen-on, peek, notification light and ambient display (A5)                                                          |
| Charging               | Setup tip on S02: turn off "wake screen when plugged in" if the phone has it (cannot be set by the app)                                         |
| Web baby               | Black full-screen overlay; cannot change brightness → instruction line on S07                                                                   |

### 6.3 Dark-room rules (parent phone)

`STATUS: UNDER DISCUSSION`

- Dark theme default; alerts use dark red, never a white flash; app never forces maximum brightness.
- Sound only for: noise alerts (child audio), test alert, connection level 2–3, baby stopped, baby battery < 5 %. Everything else is visual or one short haptic.

---

## 7. Open decisions for the owner

`STATUS: UNDER DISCUSSION`

### Options for connection-alarm sound timing (A2)

- [ ] **Option A: as ADR-017** — level 2 (60 s) vibration + screen only; sound from 180 s. Pros: fewest audible false alarms. Cons: a sleeping parent may miss up to 3 min of no monitoring.
- [ ] **Option B: soft tone from 60 s, continuous alarm from 180 s** — Pros: matches "in doubt, clearly signal"; a 60 s outage is already rare after ADR-017 recovery. Cons: occasional audible alarm after a router restart.

**Recommendation:** Option B, because a vibration on a nightstand does not reliably wake a sleeping parent, and the 60 s threshold already filters normal Wi-Fi flaps.

### Options for the user-facing device term (A8)

- [ ] **Option A: "baby phone" / "parent phone"** — Pros: matches ADR-018 copy and how parents talk ("Babyphone" in German). Cons: slightly off for tablets.
- [ ] **Option B: "baby unit" / "parent unit"** — Pros: hardware-monitor wording. Cons: sounds technical; "unit" is translated awkwardly.

**Recommendation:** Option A.

### Remote sensitivity change from the parent phone

- [ ] **Option A: baby phone only (alpha)** — Pros: no protocol change. Cons: parent must walk into the nursery to tune false alerts.
- [ ] **Option B: parent can change sensitivity and hold time remotely** — Pros: 3 AM tuning without waking the child. Cons: needs a protocol message and a trust check.

**Recommendation:** Option A for the alpha, Option B as `STATUS: IDEA` for alpha 2.
