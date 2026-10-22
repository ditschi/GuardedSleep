# ADR-011: Parent-side alert audio routing (speaker vs headphones) and call interruption

- Status: DECIDED (owner, 2026-10-21; UX re-evaluation after wave 3, see [backlog](../../../../docs/backlog.md) "Alert audio routing UX")
- Date: 2026-05-13
- Deciders: Product owner (user), coordinated design (Critical Review 2026-05)

## Context

[ADR-005](../2026-04-15-adr-005-parent-alerts-product-naming-and-distribution/design.md) locks the **alert content** to child audio (no synthetic siren). It does not fully resolve **where** that audio plays on the parent device.

The notifications feature doc previously left open: force speaker even when headphones are connected ("likely yes for safety"). That default is **wrong for discreet social use** (e.g. wedding: in-ear monitoring). Night home use often **wants** optional forced speaker so a muted phone still wakes the caregiver -- without forking the whole app into "persona modes" ([ADR-009](../2026-10-21-adr-009-personas-and-failure-model/design.md), [personas.md](../../../../docs/concept/personas.md)).

Incoming **phone calls** can steal audio focus and interrupt WebRTC playback -- behavior must be specified and PoC-tested.

## Options Considered

### Option A: Always route alert/child audio to built-in speaker

- **Pros**: Maximum audibility; simple implementation story.
- **Cons**: Fails Persona A social contexts; may violate user expectation when wearing headphones intentionally.

### Option B: Always follow system routing (headphones if connected)

- **Pros**: Natural for event users.
- **Cons**: Risk that parent does not hear alert if headset unplugged or volume low; weaker match for "baby monitor while phone on silent" home story.

### Option C: Persona-aware default + Advanced override

- **Pros**: Best fit when each user picks a stable "home vs event" profile.
- **Cons**: Same install crosses contexts (home today, wedding tomorrow); forked defaults contradict that reality; extra onboarding and parallel test matrices.

### Option D: Per-session prompt every time

- **Pros**: Maximum explicit consent.
- **Cons**: Fatigue; unacceptable at 3 AM.

### Option E: Unified app + system routing default + Advanced override (supersedes C for onboarding)

- **Pros**: One primary experience; headphones at a wedding and speaker on the nightstand both work without switching "persona mode"; matches [ADR-009](../2026-10-21-adr-009-personas-and-failure-model/design.md) "no forked UX" and [personas.md](../../../../docs/concept/personas.md) unified-app section.
- **Cons**: **Dual-route** ("Speaker + default") and per-OEM audio policy add engineering + QA cost; mitigated by PoC gate before exposing the setting broadly.

## Decision

**Adopt Option E (PROPOSED),** superseding the persona-profile table from Option C for **first-run and primary UI**.

**Default (all users, all contexts):** **Follow system audio output** for noise alerts and alert-triggered child audio -- if the user wears headphones, alerts arrive there; built-in speaker when nothing else is connected.

**Onboarding:** one-time plain-language warning during first **probe test**: _You hear alerts only through your current audio output (speaker, wired, or Bluetooth), unless you change "Alert audio output" below._ No "mainly home vs events" question.

### Parent setting: alert audio output (Basic -- Notifications)

> Exact strings TBD; behavior is binding at ADR level.

| Value                                 | Behavior                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Default**                           | Same as "follow system": Bluetooth / wired headphones if the OS routes media there; otherwise **built-in speaker**.                                                                                                                                                                                                                                                                                                                    |
| **Speaker only**                      | Alert-triggered child audio plays on the **built-in speaker** only, even if a headset is connected (discreet users switch back to Default).                                                                                                                                                                                                                                                                                            |
| **Speaker + default device** ("Both") | Duplicate the **same** alert audio to (1) the OS **default communication/media output** (often BT/wired) **and** (2) the **built-in speaker** simultaneously so a silent headset does not block wake-up. **Implementation:** subject to Android OEM capabilities -- **PoC gate** on at least two OEMs + stock; if dual physical routing is unreliable, ship with setting hidden or degraded to "Speaker only" with clear release note. |

Optional checkbox (Basic): **Warn if headphones connected** -- non-blocking banner when starting monitoring: "Alerts may be in-ear only unless you pick Speaker + default or Speaker only."

### Parent setting: alert playback volume / audibility

- **Stream class:** Alert-triggered playback on the parent MUST use **`AudioAttributes.USAGE_ALARM`** (or platform equivalent) so volume follows the **alarm / clock** volume curve and is **not** silently tied to "media" mute-only paths -- see [notifications-alerts.md](../../../../docs/features/notifications-alerts.md) (DND + alarm stream).
- **In-app gain:** Basic setting **slider** (e.g. 25 %--150 % of decoded level, default 100 %) applied **in software** to the alert playback path so parents can tune perceived loudness **without** changing the global system alarm index (avoids needing invasive `MODIFY_AUDIO_SETTINGS` in MVP).
- **Optional "force minimum gain during alert"** (toggle, default off): clamps in-app gain to a **floor** (e.g. never below 80 %) while a noise alert is active, with copy that the user must still ensure **hardware alarm volume** is not at zero. **IDEA:** one-shot OS prompt "raise alarm volume" using public APIs only -- only if a future ADR accepts any extra permission.

**Listen mode (non-alert) continuous audio** may remain on `USAGE_MEDIA` unless product decides to unify; alert path above applies specifically to **threshold-triggered alert chain** and **probe test**.

**Connection type (Wi-Fi vs metered cellular):** does **not** switch the app layout; it may adjust **non-safety** alert emphasis or codec defaults per [notifications-alerts.md](../../../../docs/features/notifications-alerts.md) (IDEA: connection-context alert policy) and [communication.md](../../../../docs/features/communication.md) (data honesty).

**Phone calls:** Document and PoC-test: on incoming call, monitoring/alerts may pause; on call end, app **must** attempt automatic resume of the monitoring session and surface a non-dismissible banner if resume fails.

## Consequences

### Positive

- Resolves CRIT-7 without weakening ADR-005's child-audio-as-alert principle.

### Negative

- More test matrix (headset wired, BT, speaker, call interruption, **dual-route**). Users who never open Advanced might miss speaker-only alerts while BT is connected -- mitigated by probe + onboarding line + **Speaker + default** option in Basic settings.

### Risks

- OEM-specific audio focus quirks -- mitigated by PoC on at least two OEMs plus stock Android.
- **Dual-route** ("Speaker + default") may be flaky on some stacks (A2DP vs sco, exclusive focus) -- ship behind PoC or feature flag until validated.

## Discarded alternatives

| Alternative                               | Reason                                                                                            |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Option A only                             | Rejected -- breaks discreet headphone use at events.                                              |
| Option B only                             | Rejected as **sole** story -- some users want forced speaker; kept as **default** under Option E. |
| Option C (persona-profile-driven routing) | Superseded by Option E -- persona fork at onboarding rejected per product direction.              |
| Option D                                  | Rejected -- alert fatigue / 3 AM friction.                                                        |
