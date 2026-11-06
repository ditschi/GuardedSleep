# Failure Model

> STATUS: PROPOSED -- to be finalized via [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md) (in preparation).
>
> **Why this document exists**: GuardedSleep is a safety-adjacent app. Until 2026-05, the project documented the happy path in detail but left the "what happens when X fails?" answers scattered across feature documents. The [concept review 2026-05](../reviews/concept-review-2026-05.md) (CRIT-5) surfaced that the most important failure case -- **parent device dead/crashed** -- was undocumented and accidentally led to a Baby-Side acoustic-alarm proposal that contradicts the core purpose of the app. This document is the single source of truth for failure handling.

## Core principle

GuardedSleep follows the **classic baby-monitor failure model**:

> If the parent receiver becomes unobservable, the parent is the one who must notice, not the child.

In a hardware baby monitor, when the receiver unit dies or loses signal, the receiver itself shows a missing-status light or a soft alert tone. The transmitter unit (near the child) does **not** start screaming. GuardedSleep mirrors that contract.

This means our entire failure-mitigation strategy on the parent-device-dead case rests on three pillars:

1. **Observability** -- the parent must visually/haptically notice when an indicator stops being there.
2. **One-tap recovery** -- when the parent does notice, reconnecting must require zero physical access to the baby device.
3. **Backup parents (Phase 2)** -- if a second parent device is paired, the baby unit also escalates there.

## Explicitly rejected mitigations

> STATUS: REJECTED -- do not propose these in any feature document without superseding this section via an ADR.

| Proposal                                                     | Why rejected                                                                                                                                                                                                          |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Acoustic siren on the baby unit when connection is lost      | Wakes the sleeping child -- directly contradicts the core purpose of the app. Captured in [vision.md Non-Goals](vision.md).                                                                                           |
| Cloud push (FCM / APNs) to a backup phone                    | Adds a mandatory cloud dependency in the alarm path -- violates [ADR-005](../../openspec/changes/archive/2026-04-15-adr-005-parent-alerts-product-naming-and-distribution/design.md) and the privacy-first principle. |
| Outbound SMS via a gateway service                           | Same cloud-dependency violation; also adds carrier cost and a new provider trust relationship.                                                                                                                        |
| Outbound VoIP call (call the parent or an emergency contact) | Same cloud dependency; also unreliable on muted phones.                                                                                                                                                               |
| Visible always-on bright LED / flashing screen on baby unit  | Wakes the child by light, not by sound. Same root violation.                                                                                                                                                          |
| Demand a second device as hardware fallback                  | Excludes single-parent setups; mismatches Persona A.                                                                                                                                                                  |

The only acoustic-on-baby-side variant that survived discussion is: a **very dim, slow visual pulse** on the baby device when disconnected (see "Secondary observability"), specifically because (a) the baby is asleep and oriented away from the screen, and (b) an adult walking past the room would notice the lit screen.

## Failure cases

The following cases are exhaustive for the MVP scope. Each case lists: trigger, observability on parent side, observability on baby side, what GuardedSleep does, what the user is expected to do.

### F-1: Parent device dead (battery, crash, OS-kill)

**Trigger**: parent's phone battery fully drained, OS killed the app process, kernel panic, hardware fault, or user accidentally swiped GuardedSleep away.

**Observability on parent side**:

- The persistent foreground notification disappears from the lock screen / notification shade.
- The ambient-glow status indicator at the top of the parent screen is gone (because the app is gone).
- "Always quiet since…" timer freezes.
- When the parent next picks up their phone, they see the absence -- this is the trained recognition path (see [Indicator Training](../../openspec/specs/device-setup/spec.md)).

**Observability on baby side**:

- The baby unit detects heartbeat loss after 2 missed beats (~20 s with default 10 s interval).
- After 60 s, the baby unit screen shows a slow red pulse (very dim, no sound). This is the **secondary observability** signal for an adult who walks past the baby's room.

**What the app does**:

- Baby unit continues to record noise events locally (see [audio-monitoring spec](../../openspec/specs/audio-monitoring/spec.md)).
- Baby unit keeps trying to reconnect in the background.
- In Phase 2 multi-parent, baby unit escalates to other paired parent devices (see F-1a below).

**What the user does**:

- Recharge / restart the parent phone.
- Open GuardedSleep -> Saved Connection card -> one tap -> reconnected. **No physical trip to the baby device required.**
- After reconnect: the buffered noise events from the disconnected period are delivered as a single "missed events" summary (Phase 2 polish; MVP just shows "Connected -- you may have missed alerts").

**Why the user must learn this**: GuardedSleep cannot alarm a turned-off phone. This is the same limitation a hardware baby monitor has. Onboarding must communicate this honestly (see [NEW-2 in the concept review](../reviews/concept-review-2026-05.md)).

### F-1a: Parent-1 dead, Parent-2 available (Phase 2 multi-parent failover)

> STATUS: DECIDED for Phase 2 -- documented here, implemented in [device-setup spec](../../openspec/specs/device-setup/spec.md) and [parent-alerts spec](../../openspec/specs/parent-alerts/spec.md).

**Trigger**: Parent-1 unobservable (per F-1). Baby unit holds a second active DataChannel to Parent-2.

**What the app does**:

- Baby unit detects Parent-1 heartbeat loss > 60 s.
- Baby unit sends a "you are now primary" message to Parent-2 (lightweight DataChannel message, no audio).
- Parent-2 device shows an additional banner: "Phone-1 [name] not responding -- you are now the primary receiver."
- Parent-2 starts receiving the full alert chain (vibration + screen + audio) instead of merely the secondary copy.
- If Parent-2 confirms (taps "OK"), the banner persists until Parent-1 reconnects.

This is the **only privacy-conformant software backup** for the parent-dead case.

### F-2: Baby device dead

**Trigger**: baby unit's battery fully drained, OS killed the foreground service, kernel panic, hardware fault, or someone physically removed the device.

**Observability on parent side**:

- Heartbeat loss -- escalating alarm sequence per the table in [ui-design.md](../technical/ui-design.md):
  - 0-20 s: silent reconnect attempt, indicator yellow.
  - 20-60 s: gentle vibration + banner "Connection interrupted".
  - 60 s – 3 min: urgent vibration + screen wake.
  - > 3 min: full alarm (audible).
- This case is **well-handled today**.

**Observability on baby side**: none. The device is off.

**What the user does**: physically check the baby device. This is the one failure case where physical trip is unavoidable -- and acceptable, because the alternative is the child being unmonitored.

### F-3: Network completely lost (e.g. router reboot, mobile data outage)

**Trigger**: WiFi router rebooted, ISP outage, mobile data went out of coverage.

**Observability on parent side**: heartbeat loss escalation per F-2 table.

**Observability on baby side**: same, mirrored.

**What the app does**:

- Both devices continue trying to reconnect via all known IP paths (saved local IP, signaling URL, VPN IP), using the **path-selector** introduced in [ADR-012](../../openspec/changes/archive/2026-10-21-adr-012-connection-model-offline-lan-and-handoff/design.md): primary signaling URL first, then each entry in `Saved Connections → Fallback signaling URLs`.
- IP-sync mechanism ensures the latest known IPs are used (see [pairing spec](../../openspec/specs/pairing/spec.md)).
- Noise events on the baby side are buffered locally for delivery on reconnect.
- UI escalation per [ADR-013](../../openspec/changes/archive/2026-10-21-adr-013-connection-status-observability-and-copy/design.md): ambient bar yellow → orange (20-60 s) → red (> 60 s) with gentle, then escalating vibration on the parent, slow red pulse on the baby (does not wake the child).

**What the user does**: wait for the network to come back, or trigger manual reconnect from the parent app. If it stays down: physically check both devices and restart the network.

### F-4: Both devices dead simultaneously

**Trigger**: extremely rare -- power outage that kills both phones (only relevant if the parent phone is also unplugged). Or simultaneous unrelated failures.

**Observability**: none on either side.

**What the user does**: this is the irreducible failure case. As with a hardware baby monitor, GuardedSleep cannot alarm anyone if no device is alive. Onboarding must call this out (see NEW-2).

### F-5: OS-kill of the app (battery optimization, OEM aggressive Doze)

> STATUS: DECIDED MVP mitigations exist (foreground service, battery-optimization deep-link) but Persona C (old OEMs) remains exposed.

**Trigger**: Android battery optimization, MIUI/EMUI background-kill, OEM Doze mode.

**Observability on parent side**: looks identical to F-1 from the parent's view -- the indicator disappears. The user cannot tell whether the phone died, the app died, or the OS killed it.

**What the app does**:

- Foreground service with persistent notification mitigates this on most devices.
- Battery-optimization deep-link in onboarding asks the user to exempt the app.
- OEM-specific helper text (deferred -- see IMP-8 in concept review).
- "Lakmus test" idea: app counts heartbeats over a 30-min window after onboarding to verify the OS isn't silently killing it; warns the user if the count is wrong.

**What the user does**: follow the battery-optimization deep-link. On stubborn OEMs (Persona C territory), repeat the OEM-specific steps.

### F-6: WebRTC connection succeeds but audio is silent / corrupted

**Trigger**: codec mismatch, WebView bug on old Android, microphone permission silently revoked.

**Observability on parent side**: heartbeat OK, indicator green, but listen-mode reveals silence even though noise should be present.

**What the app does**: not currently mitigated. Documented here as an open vulnerability.

**Mitigation (proposed)**: probe-alarm functionality (SUG-2) lets the parent trigger a synthetic noise burst on the baby device and verify reception. Should be done after onboarding, after OS updates, and before Persona A's event-mode use.

### F-7: Pairing material compromised (lost phone, swapped SIM, stolen device)

**Trigger**: a previously paired device is no longer trusted (sold, stolen, family conflict).

**Observability**: none -- this is a trust-model failure, not an observability failure.

**What the app does**:

- Saved Connections can be deleted on the user's own device.
- Phase 2 ([pairing spec](../../openspec/specs/pairing/spec.md)): **Pair revocation** -- any cluster member can remove a device, triggers re-keying.
- "Panic button" in settings deletes all connections + keys on the local device.

**What the user does**: revoke the lost device via any other cluster member; or if they cannot, re-pair from scratch (which invalidates the old PSK if re-keying is implemented).

### F-8: Untrusted signaling infrastructure (`alpha-connection-model`)

**Trigger**: the project server, an own relay, a LAN relay, the project TURN or a STUN operator misbehaves (drops, delays, replays, floods), is operated badly, or simply observes.

**What it learns**: the project operator sees the IP address and the timing of every default family: when each device sets up or recovers a connection, and, while a call is relayed, when it runs and how much traffic it carries. A relayed call also reveals the peers' LAN addresses to the operator (the ICE candidates of the call), and TURN relays the encrypted media (DTLS-SRTP, so never audio in the clear). Signaling envelopes are sealed (no SDP, candidates, fingerprints or names), but there is no forward secrecy: a later PSK compromise (F-7) decrypts retained envelopes (SDP and IPs, never audio). The 6 s resend shows a relay that an envelope went unanswered; this is inherent and accepted. An own relay or a LAN relay sees the same connection metadata for its own families.

**What it cannot do**: hear the baby, fake "monitored" (liveness runs on the DataChannel), mute escalation (stops are bound to confirmed sessions), or take over the baby after the first pairing (parent secret over DTLS). A PSK holder can still squat the single parent slot on a relay: denial of service only, escalation still runs.

**Operator-side risks that stay** (`alpha-connection-model`, project server):

- **Single point of failure:** one project server carries every default family. It is down for all of them at once; a family with an own relay or LAN keeps its other mailboxes, and a verified link does not need any mailbox.
- **Domain lapse or DNS account takeover:** whoever controls `gs.mydata.rocks` receives the signaling and the TURN traffic of every default family until the apps ship another address. Sealed envelopes and DTLS limit this to metadata and denial of service; it is a reason to keep the domain and DNS accounts under strong authentication and long renewal.
- **TURN exhaustion by many addresses:** an attacker with many source addresses can use up relay bandwidth or allocations; credentials are short-lived, issued per room member and capped per address, which limits but does not remove this.

**Residual risk**: at the first pairing, whoever verifies first within the 5-minute invite is pinned; the baby shows the parent device's name with "Not my device".

## Reconnection specification (one-tap requirement)

The "indicator-loss as alarm" model only works if reconnection is trivial. The following are **hard requirements** for the parent-side app:

1. **PSK persistence**: the PSK is stored in Android Keystore (TEE) and survives reboots, app updates, and OS unlocks (already DECIDED in [pairing spec](../../openspec/specs/pairing/spec.md)).
2. **Saved-Connection persistence**: device name, known IPs, signaling URL, PSK reference -- persisted in encrypted SharedPreferences, key-wrapped by Android Keystore.
3. **Cold-start path**: after a parent-phone reboot, the user unlocks the OS, opens GuardedSleep, sees the Saved-Connection card, taps once -- reconnection completes within 15 s using the persisted credentials and the same WebRTC ICE bootstrap as the original pairing.
4. **No QR re-scan required** in the F-1, F-3, F-5 reconnection paths. QR re-scan is reserved for: (a) first-ever pairing, (b) pairing a new device, (c) post-revocation re-pair.
5. **Reconnect timeout**: 15 s default. On failure, the app shows the troubleshooter from [ux-review-setup-and-error-states.md](../reviews/ux-review-setup-and-error-states.md) Recommendation 5.5.

### Auto-resume after reboot (Persona B requirement)

> STATUS: DECIDED MVP, Advanced setting -- see [device-setup spec](../../openspec/specs/device-setup/spec.md).

For Persona B (Daily Home), reconnect should ideally happen without any user action:

- **Baby-side**: "Auto-Resume Baby-Mode after reboot" setting. When enabled, the app boots into Baby-Mode after the phone is unlocked, with the kiosk lock already in place. Default ON for the daily profile.
- **Parent-side**: "Auto-Start Parent-Mode after reboot" -- STATUS: IDEA Phase 2. More cautious because Persona A may not want monitoring to auto-start after a regular phone reboot.

If auto-resume fails (e.g. permission revoked after an OS update), the parent observes the missing indicator -- F-1 path applies.

## Observability building blocks

For the indicator-loss model to actually work, the parent UI must make the "Connected" state distinctive enough that its absence is noticed. The following are **hard UI requirements**:

- **Ambient-glow status indicator**: dimensionally constant element at the top edge of the parent screen, green when connected, yellow when reconnecting, red when disconnected. Specified in [ui-design.md](../technical/ui-design.md).
- **Persistent foreground-service notification**: visible in the system notification shade even when the app is backgrounded or the phone is locked. Format: "GuardedSleep -- connected to [Babyname]". Specified in [parent-alerts spec](../../openspec/specs/parent-alerts/spec.md).
- **Indicator Training in onboarding**: new users explicitly see the "connected" look and the "disconnected" look during a 30 s onboarding step. They are told: "When the green indicator disappears, you are not monitoring." Specified in [device-setup spec](../../openspec/specs/device-setup/spec.md).
- **Probe-alarm button**: one-tap synthetic alarm to verify the end-to-end chain still works (especially after OS updates and for Persona A pre-departure). Specified in [parent-alerts spec](../../openspec/specs/parent-alerts/spec.md).

## Discarded alternatives

| Alternative                                                                              | Reason for rejection                                                                                            |
| ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Acoustic siren on baby unit at connection-loss                                           | Wakes the child -- contradicts the core purpose.                                                                |
| Mandatory second device as hardware fallback                                             | Excludes single-parent setups; mismatches Persona A.                                                            |
| Cloud-based dead-man-switch (e.g. periodic "I'm alive" ping that triggers SMS if missed) | Adds cloud dependency, opens new attack surface, privacy contradiction.                                         |
| Pager-style external hardware (e.g. BLE beacon)                                          | Adds proprietary hardware -- contradicts the cross-platform / device-recycling vision.                          |
| Visual flash on baby unit at connection-loss                                             | Wakes the child via light. Variant survives only as a dim slow pulse (see "Secondary observability" under F-1). |
