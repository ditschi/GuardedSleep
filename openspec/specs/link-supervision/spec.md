# link-supervision Specification

## Purpose

Keep one monitoring relationship between a baby unit and a parent unit alive from "start" to "stop": detect loss on both sides, recover with fresh attempts, and escalate to the parent so that monitoring never stops silently. Migrated from ADR-017 §2–§9 and §11 (decided 2026-10-20); pairing proof and room derivation (§1) belong to `pairing`, the signaling server (§10) to `signaling-relay`.

## Requirements

### Requirement: Supervision lives only in the core Link

The Rust core `Link` SHALL own the state of every mailbox, attempts, liveness, escalation, listen intent, snooze, peer status and the missed-event buffer. Platforms SHALL talk only to the `Link` and SHALL NOT run their own reconnect loops, mailbox failover or timer-based alarms; they open, close and feed the mailboxes the `Link` asks for. A `Link` SHALL NOT be constructed without a PSK.

#### Scenario: Link without a PSK

- **WHEN** a platform constructs a `Link` with an empty or malformed PSK
- **THEN** construction fails with a configuration error and no attempt is ever created

#### Scenario: Platform loses the network

- **WHEN** the platform observes a network change
- **THEN** it only reports `network_available` to the `Link` and performs whatever actions the `Link` returns

#### Scenario: Wi-Fi to mobile data

- **WHEN** the parent device switches from Wi-Fi to mobile data during verified monitoring
- **THEN** the `Link` detects the loss on the DataChannel, reopens the mailboxes, and verifies a new attempt before escalation level 2 when the networks allow a direct path

#### Scenario: Parent process restarts while the baby is verified

- **WHEN** the parent restarts and sends `request_offer` while the baby still holds a verified attempt
- **THEN** the baby ignores the request until its liveness timeout (25 s by default) closes the old attempt, and recovery completes below escalation level 2

### Requirement: Recovery is a new attempt, at most one live

Each recovery SHALL create a fresh `Attempt` (one peer connection with new DTLS certificates and its own pairing proof); an `Attempt` SHALL never retry itself. The `Link` SHALL keep at most one live attempt, always emitting `close_attempt` before `create_attempt`, and SHALL drop inputs that carry the handle of a closed attempt.

#### Scenario: Late callback from a closed attempt

- **WHEN** a WebRTC callback arrives for an attempt handle the `Link` already closed
- **THEN** the input is dropped without changing state or emitting actions

### Requirement: A verified attempt is closed only by its own failure

A verified attempt SHALL be closed only when its peer connection reports `failed` or `closed`, its DataChannel closes, its liveness timeout expires, or an authenticated stop notice arrives. Nothing received through signaling (a new offer, a new `aid`, a membership change, unauthenticated stops) SHALL close a verified attempt. Remote ICE candidates SHALL be ignored once an attempt is verified.

#### Scenario: Forged offer against a verified attempt

- **WHEN** a room member sends an offer with a new `aid` while an attempt is verified
- **THEN** the verified attempt stays open and monitoring continues

#### Scenario: Parent process restarts while the baby is verified

- **WHEN** the parent restarts and sends `request_offer` while the baby still holds a verified attempt
- **THEN** the baby ignores the request until its liveness timeout (25 s by default) closes the old attempt, and recovery completes below escalation level 2

### Requirement: Offer direction

Only the baby SHALL create offers, and only when it has no live verified attempt and a parent is present: the pinned parent (or, before pinning, any parent) has a confirmed current session (announced with `here` on any mailbox) and, for a relay mailbox, is a member of that relay's room with the client id the session last spoke from.

#### Scenario: Parent session not confirmed yet

- **WHEN** a parent's `here` has arrived but its session has not confirmed the baby's challenge
- **THEN** the baby creates no offer for it

### Requirement: Offer request

The parent SHALL send `request_offer` to the baby's current session when it starts or recovers without a live attempt. An offer answering a request SHALL carry `re` set to that request's id, and the parent SHALL adopt an offer only for its current request.

#### Scenario: Both sides start at the same time

- **WHEN** the parent and the baby start within the same tick and the parent's request crosses an earlier offer
- **THEN** the parent adopts only the offer whose `re` matches its request, and the two devices converge on one verified attempt

### Requirement: Offer grace period

A baby that sees a parent present SHALL wait a short grace period (2.5 s) for that parent's `request_offer` before it offers unasked, so that a clean setup builds one attempt per side.

#### Scenario: Clean setup builds one attempt

- **WHEN** a baby and a parent start together over the project server alone, or over the project server and an own relay, and nothing fails
- **THEN** each core creates exactly one attempt and the link verifies without a replaced attempt

### Requirement: Accepted signaling senders

Offers and candidates SHALL be accepted only from envelopes that authenticate under the pairing's envelope key, carry the peer's role and the receiver's current session, and, once the pairing is pinned, a valid pin tag of the peer role. The wire `aid` SHALL only filter stale candidates and SHALL never be a trust decision.

#### Scenario: Stranger in the room

- **WHEN** a client without the pairing key sends an offer to the parent through any mailbox
- **THEN** the parent ignores it

### Requirement: Deterministic attempt identity

Attempts SHALL be identified by a local `u32` handle between core and platform and by a 128-bit wire `aid` drawn from a PRNG seeded by `start{entropy}`, so that recorded traces replay deterministically. Envelope nonces, session nonces and the parent secret SHALL come from a separate cryptographic generator seeded by 32 random bytes in the `Link` configuration, freshly drawn by the platform for every run; only simulation traces with test keys SHALL record that seed.

#### Scenario: Trace replay

- **WHEN** a simulation trace is replayed with its recorded `start` entropy and seed
- **THEN** the replay produces the same `aid` values, envelope nonces and actions

#### Scenario: Missing seed

- **WHEN** a platform constructs a `Link` without a 32-byte seed
- **THEN** construction fails with a configuration error

### Requirement: Bidirectional heartbeat

Both sides SHALL send `heartbeat` over the DataChannel every `heartbeat_interval_ms` (default 10 s, configurable 5–15 s). The heartbeat `t` field SHALL be opaque to the receiver. `last_alive` SHALL be the time of the last DataChannel message from the peer, and all escalation timing SHALL be measured from it.

#### Scenario: Baby learns whether a parent listens

- **WHEN** the parent's heartbeats stop arriving at the baby
- **THEN** the baby's `last_alive` stops advancing and the baby detects the loss like the parent would

### Requirement: Loss detection

A verified attempt SHALL be considered lost when its peer connection is `failed` or `closed`, its DataChannel closes, or no DataChannel message arrives for `liveness_ms = 2 × interval + 5 s` (25 s default, 35 s maximum). The `disconnected` state alone, absence from a relay room and the closing of mailboxes SHALL NOT count as loss.

#### Scenario: Peer goes silent

- **WHEN** no DataChannel message arrives from the peer for `liveness_ms`
- **THEN** the `Link` closes the attempt and moves to `reconnecting`

#### Scenario: Signaling server restarts while connected

- **WHEN** the signaling server restarts and the peer briefly disappears from `members`
- **THEN** the verified attempt and the link state stay unchanged

### Requirement: Configuration invariants

The `Link` SHALL reject a configuration where `heartbeat_interval_ms` is outside 5–15 s, where `liveness_ms > alarm_after_ms − 20 s`, or where `alarm_after_ms` is not below the level-3 threshold (180 s).

#### Scenario: Alarm at the level-3 threshold

- **WHEN** a platform configures `alarm_after_ms = 180 s`
- **THEN** the configuration is rejected

### Requirement: Attempt scheduling

Delays before a new attempt SHALL be 0, 1, 2, 4, 8 s and then 15 s (cap); after 10 minutes without success the cap SHALL rise to 30 s. The backoff SHALL reset after a verified attempt and SHALL reset with an immediate retry on `network_available`, when a mailbox reopens, and when the peer announces a new session. An attempt not verified within 20 s SHALL be abandoned.

#### Scenario: Wi-Fi returns

- **WHEN** the platform reports `network_available` during a 15 s backoff wait
- **THEN** the `Link` starts a new attempt immediately

### Requirement: Error classification

A relay's errors `BABY_TAKEN`, `PARENT_CAP`, `NOT_JOINED` and `SIGNAL_TARGET_REQUIRED` SHALL be retryable at the mailbox layer. `UNSUPPORTED_PROTOCOL` SHALL be terminal (`failed_needs_user`, key `update_required`) only as the reply to `hello` before `welcome` and only when no other mailbox works; with a verified attempt it SHALL only close that mailbox and set a sticky `update_required` hint.

#### Scenario: Stale baby socket

- **WHEN** a reconnecting baby is refused with `BABY_TAKEN` because its old socket is still registered
- **THEN** the `Link` retries and joins once server liveness has removed the old socket

#### Scenario: Outdated own relay while the project server works

- **WHEN** an own relay answers `hello` with `UNSUPPORTED_PROTOCOL` while the project server reaches the peer
- **THEN** only that mailbox closes, with the sticky `update_required` hint, and the link continues on the project server

### Requirement: Pairing mismatch handling

On a first pairing (the PSK never verified, `paired_before = false`) two consecutive pairing-proof mismatches SHALL end in `failed_needs_user` with key `pairing_key_mismatch`. After a successful pairing a mismatch SHALL never be terminal: it SHALL close that attempt, emit `security_warning`, and delay the next attempt by at least 15 s. `paired_before` SHALL be persisted by the platform so the rule survives a process restart.

#### Scenario: Wrong code at first pairing

- **WHEN** the first two attempts with a new PSK both fail the pairing proof
- **THEN** the link reaches `failed_needs_user{pairing_key_mismatch}` and asks to scan the code again

#### Scenario: Spoofed mismatch after pairing

- **WHEN** an attacker causes a proof mismatch on a link that was verified before
- **THEN** monitoring keeps retrying, the parent sees a security warning, and the link is never terminal

### Requirement: Never give up while monitoring

The `Link` SHALL keep retrying while monitoring is on. From escalation level 2 it SHALL surface a troubleshooter while retrying, with the cause hint `server_unreachable` (no mailbox opens), `peer_absent` (at least one mailbox open, no current `here` or room membership of the peer), or `media_blocked` (offer and answer done, no verification after 3 attempts). "Try now" SHALL reset the backoff.

#### Scenario: Peer never arrives

- **WHEN** mailboxes are open but the other device announces no session past level 2
- **THEN** the parent sees the troubleshooter with hint `peer_absent` while the link keeps retrying

#### Scenario: Project server unreachable

- **WHEN** the project server is the only mailbox and it cannot be reached
- **THEN** the parent sees the troubleshooter with hint `server_unreachable` and the link keeps retrying it on the normal backoff

### Requirement: Link states

The `Link` SHALL expose exactly the states `connecting` (never verified in this run), `connected`, `reconnecting`, `peer_stopped`, `failed_needs_user{key}` and `stopped`, and SHALL expose `last_alive_ms` so the UI can show how long the peer has been unreachable.

#### Scenario: First start

- **WHEN** a link starts and has not verified any attempt in this run
- **THEN** its state is `connecting`, not `reconnecting`

### Requirement: Escalation levels

While not connected, the core SHALL emit an escalation level from `now − last_alive`: level 0 from 0 s, level 1 from 20 s, level 2 from `alarm_after_ms` (default 60 s) with the `link_lost` alarm, and level 3 from 180 s. Platforms SHALL only map the emitted level to feedback.

#### Scenario: Short outage

- **WHEN** the baby's Wi-Fi is off for 20 s
- **THEN** the link recovers without reaching level 2

#### Scenario: Long outage

- **WHEN** the baby's Wi-Fi is off for 90 s
- **THEN** level 2 is raised within `alarm_after_ms` + 2 s of `last_alive`, and `link_restored` follows recovery

#### Scenario: Grey zone

- **WHEN** an outage lasts between 30 s and 60 s
- **THEN** either recovering without level 2 or raising level 2 is a valid outcome

### Requirement: Escalation feedback per device

The parent SHALL render level 0 yellow and silent; level 1 orange, visual only, with a low-priority notification; level 2 red with repeating vibration, a full-screen alert and a soft tone every 10 s; level 3 as an audible alarm on the alarm stream with DND bypass. The baby SHALL show a yellow or orange edge indicator at levels 0 and 1 and a slow dim red pulse from level 2, and SHALL never use sound or vibration.

#### Scenario: Baby side during a long outage

- **WHEN** the baby's link reaches level 3
- **THEN** the baby still shows only the dim red pulse, without sound or vibration

### Requirement: Deliberate stop is not an outage

`stop` SHALL send a stop notice over the DataChannel and over every open mailbox before closing. On an authenticated stop notice for the current session the peer link SHALL close the verified attempt and go to `peer_stopped`: the parent raises one non-repeating alert, the baby shows a calm grey state, and no escalation runs. The link SHALL then reopen its mailboxes and wait until the peer announces a new session that confirms.

#### Scenario: Baby is stopped on purpose

- **WHEN** the baby user stops monitoring
- **THEN** the parent shows one "baby unit stopped monitoring" alert and does not escalate

### Requirement: Authenticated signaling stop

A stop notice sent through a mailbox SHALL be a sealed envelope bound to the receiver's current session (`signaling-mailboxes`). Stop notices that fail authentication, name another recipient session, come from a sender session that is not the peer's current confirmed session, or repeat an envelope id SHALL be ignored, so a mailbox cannot mute escalation, including by replaying a stored stop after a restart.

#### Scenario: Relay forges a stop

- **WHEN** a mailbox injects a stop notice that does not authenticate
- **THEN** the parent ignores it and escalation continues as normal

#### Scenario: Stored stop after a restart

- **WHEN** a relay serves an authentic stop notice from an earlier session to a restarted parent
- **THEN** the parent ignores it and escalation continues as normal

#### Scenario: Stored stop from a retired sender session

- **WHEN** a mailbox injects an authentic stop from a baby session that has since been replaced by a new one
- **THEN** the parent ignores it and escalation continues as normal

### Requirement: Acknowledge and snooze

The parent input `acknowledge_alarm` SHALL silence sound and vibration of levels 2 and 3 for 5 minutes while the state stays red; if the link is still down afterwards the level SHALL jump straight to 3. Snooze SHALL affect noise alerts only, never escalation levels 1–3.

#### Scenario: Parent is checking

- **WHEN** the parent acknowledges a level-2 alarm and the link is still down 5 minutes later
- **THEN** the alarm resumes at level 3

### Requirement: Restoration

After a verified attempt following level ≥ 1 the `Link` SHALL emit `link_restored{down_ms}` and stop alarm sound and vibration, and the parent SHALL show how long the baby was not monitored. Listen intent SHALL be re-sent after recovery if the outage was under 60 s and reset to off otherwise.

#### Scenario: Listen mode after a short outage

- **WHEN** the parent was listening and the link recovers after 30 s
- **THEN** listening resumes without a new tap

### Requirement: Missed events

The baby SHALL buffer noise events while not connected (count, maximum level, relative offsets; never audio) and send `missed_events{complete}` after verification, with `complete = false` when the buffer did not cover the whole outage. The parent SHALL show a count only when `complete`.

#### Scenario: Baby process restarted during the outage

- **WHEN** the baby process restarts while the link is down
- **THEN** the missed-events report is sent with `complete = false` and the parent shows only the unmonitored duration

### Requirement: Signaling transport liveness

On a relay mailbox, clients SHALL send an application `ping` every 15 s and treat a socket without any message for 35 s as dead. Mailbox loss while an attempt is verified SHALL NOT count as monitoring loss.

#### Scenario: Signaling drops, media continues

- **WHEN** a mailbox socket dies while the attempt is verified
- **THEN** the link state stays `connected`

#### Scenario: Half-open relay socket in a browser

- **WHEN** a relay socket in the browser stops delivering without closing while the link is down
- **THEN** after 35 s without any message the core treats it as dead and reopens that mailbox

### Requirement: Clock and time jumps

The core clock SHALL be monotonic milliseconds that include deep sleep (Android `SystemClock.elapsedRealtime()`; web `Date.now()` clamped never to go backwards), ticked at least every 500 ms while monitoring. Doze gaps SHALL be treated as time jumps.

#### Scenario: Ten-minute Doze gap

- **WHEN** the next tick arrives 10 minutes after the previous one and the peer is still alive
- **THEN** the link stays connected once the peer's next message arrives within the liveness window, and escalates as normal if it does not

### Requirement: Wall clock input

The platform SHALL report the wall clock in Unix seconds at start, every 60 seconds and after a time change. The core SHALL use it only for the invite expiry, the pin window and the clock comparison with the peer, never for liveness, escalation or any mailbox.

#### Scenario: Wall clock jumps by an hour

- **WHEN** the user changes the device time by an hour during monitoring
- **THEN** liveness and escalation timing are unaffected, and no mailbox is reopened because of it

#### Scenario: Clocks of the two devices disagree

- **WHEN** a `here` of the peer's confirmed session carries a wall clock more than 10 minutes from this device's
- **THEN** both devices raise the `clock_skew` hint, and signaling and escalation are unaffected

### Requirement: Process death

The parent foreground service SHALL be sticky and resume monitoring from the persisted, encrypted pairing. When the OS kills the baby process, the baby SHALL post a "tap to resume monitoring" notification where possible, and the parent SHALL escalate as usual meanwhile. Onboarding SHALL state that the parent phone is a single point of failure for alerting.

#### Scenario: OS kills the baby process

- **WHEN** Android kills the baby process and cannot restart the microphone service from the background
- **THEN** the parent escalates through levels 1–3 until the baby app is reopened

### Requirement: Sans-IO interface contract

The `Link` SHALL be driven only through serde-JSON inputs and actions (snake_case, `type`-tagged); `LinkInput` and `LinkAction` in `crates/guardedsleep-core/src/link.rs` are the contract for every binding. The core SHALL export `message_keys()` so the i18n check can verify that each key exists in `locales/en.json`. The baby's local SDP SHALL contain a `sendonly` audio m-line, otherwise the core refuses it.

#### Scenario: Baby SDP would receive audio

- **WHEN** the baby platform reports a local description whose audio m-line is not `sendonly`
- **THEN** the core refuses it and the attempt does not proceed

### Requirement: Noise escalation timer

The parent core SHALL accept an optional escalation delay (`None` switches the timer off) and an acknowledge-noise input, and SHALL emit a `noise_escalated` event once per noise event when the delay has passed since the event start (or since the delay was set, if later) without an acknowledgment. Listen, snooze and the acknowledge-noise input SHALL count as acknowledgment. The core SHALL take time only from its inputs and SHALL NOT know why the delay was set.

#### Scenario: Noise nobody reacts to

- **WHEN** the delay is 2 minutes, a noise alert arrives and nothing acknowledges it for 2 minutes
- **THEN** the core emits `noise_escalated` once

#### Scenario: Parent listens in time

- **WHEN** the parent starts Listen 30 seconds after the alert
- **THEN** no `noise_escalated` event follows for that event

#### Scenario: Already listening

- **WHEN** Listen is on when a noise event starts
- **THEN** the event counts as acknowledged and no `noise_escalated` event follows

#### Scenario: Timer off (failure)

- **WHEN** the delay is `None` and noise continues for 10 minutes
- **THEN** no `noise_escalated` event is emitted

### Requirement: Noise event lifecycle in the core

The parent core SHALL track its current noise event from non-probe noise alerts and loud peer heartbeats; the event SHALL end after 2 minutes without a loud signal, and the next noise alert SHALL start a new event with a fresh count and no acknowledgment.

#### Scenario: New event after quiet

- **WHEN** an escalated event ended after 2 quiet minutes and noise returns
- **THEN** a new event starts and escalates only after the delay passes again
