# link-supervision Specification

## Purpose

Keep one monitoring relationship between a baby unit and a parent unit alive from "start" to "stop": detect loss on both sides, recover with fresh attempts, and escalate to the parent so that monitoring never stops silently. Migrated from ADR-017 §2–§9 and §11 (decided 2026-10-20); pairing proof and room derivation (§1) belong to `pairing`, the signaling server (§10) to `signaling-relay`.

## Requirements

### Requirement: Supervision lives only in the core Link

The Rust core `Link` SHALL own the signaling-transport state, attempts, liveness, escalation, listen intent, snooze, peer status and the missed-event buffer. Platforms SHALL talk only to the `Link` and SHALL NOT run their own reconnect loops or timer-based alarms. A `Link` SHALL NOT be constructed without a PSK.

#### Scenario: Link without a PSK

- **WHEN** a platform constructs a `Link` with an empty or malformed PSK
- **THEN** construction fails with a configuration error and no attempt is ever created

#### Scenario: Platform loses the network

- **WHEN** the platform observes a network change
- **THEN** it only reports `network_available` to the `Link` and performs whatever actions the `Link` returns

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

Only the baby SHALL create offers, and only when it has no live verified attempt and a parent is present. The parent SHALL send `request_offer` to the baby's client id when it joins or recovers without a live attempt. An offer answering a request SHALL carry `re` set to that request's id, and the parent SHALL adopt an offer only for its current request.

#### Scenario: Both sides start at the same time

- **WHEN** the parent and the baby start within the same tick and the parent's request crosses an earlier offer
- **THEN** the parent adopts only the offer whose `re` matches its request, and the two devices converge on one verified attempt

### Requirement: Accepted signaling senders

Offers and candidates SHALL be accepted only from the client id that `members` lists as the room's baby (on the parent) or from the parent of the most recent `request_offer` (on the baby), always addressed with `to`. The wire `aid` SHALL only filter stale candidates and SHALL never be a trust decision.

#### Scenario: Stranger in the room

- **WHEN** a client that is not the listed baby sends an offer to the parent
- **THEN** the parent ignores it

### Requirement: Deterministic attempt identity

Attempts SHALL be identified by a local `u32` handle between core and platform and by a 128-bit wire `aid` drawn from a PRNG seeded by `start{entropy}`, so that recorded traces replay deterministically.

#### Scenario: Trace replay

- **WHEN** a recorded trace is replayed with its recorded `start` entropy
- **THEN** the replay produces the same `aid` values and the same actions

### Requirement: Bidirectional heartbeat

Both sides SHALL send `heartbeat` over the DataChannel every `heartbeat_interval_ms` (default 10 s, configurable 5–15 s). The heartbeat `t` field SHALL be opaque to the receiver. `last_alive` SHALL be the time of the last DataChannel message from the peer, and all escalation timing SHALL be measured from it.

#### Scenario: Baby learns whether a parent listens

- **WHEN** the parent's heartbeats stop arriving at the baby
- **THEN** the baby's `last_alive` stops advancing and the baby detects the loss like the parent would

### Requirement: Loss detection

A verified attempt SHALL be considered lost when its peer connection is `failed` or `closed`, its DataChannel closes, or no DataChannel message arrives for `liveness_ms = 2 × interval + 5 s` (25 s default, 35 s maximum). The `disconnected` state alone and absence from `members` SHALL NOT count as loss.

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

Delays before a new attempt SHALL be 0, 1, 2, 4, 8 s and then 15 s (cap); after 10 minutes without success the cap SHALL rise to 30 s. The backoff SHALL reset after a verified attempt and SHALL reset with an immediate retry on `network_available`, on signaling reopening, and when the peer reappears in `members`. An attempt not verified within 20 s SHALL be abandoned.

#### Scenario: Wi-Fi returns

- **WHEN** the platform reports `network_available` during a 15 s backoff wait
- **THEN** the `Link` starts a new attempt immediately

### Requirement: Error classification

Server errors `BABY_TAKEN`, `PARENT_CAP`, `NOT_JOINED` and `SIGNAL_TARGET_REQUIRED` SHALL be retryable at the signaling layer. `UNSUPPORTED_PROTOCOL` SHALL be terminal (`failed_needs_user`, key `update_required`) only as the reply to `hello` before `welcome`; with a verified attempt it SHALL only close signaling and set a sticky `update_required` hint.

#### Scenario: Stale baby socket

- **WHEN** a reconnecting baby is refused with `BABY_TAKEN` because its old socket is still registered
- **THEN** the `Link` retries and joins once server liveness has removed the old socket

### Requirement: Pairing mismatch handling

On a first pairing (the PSK never verified, `paired_before = false`) two consecutive pairing-proof mismatches SHALL end in `failed_needs_user` with key `pairing_key_mismatch`. After a successful pairing a mismatch SHALL never be terminal: it SHALL close that attempt, emit `security_warning`, and delay the next attempt by at least 15 s. `paired_before` SHALL be persisted by the platform so the rule survives a process restart.

#### Scenario: Wrong code at first pairing

- **WHEN** the first two attempts with a new PSK both fail the pairing proof
- **THEN** the link reaches `failed_needs_user{pairing_key_mismatch}` and asks to scan the code again

#### Scenario: Spoofed mismatch after pairing

- **WHEN** an attacker causes a proof mismatch on a link that was verified before
- **THEN** monitoring keeps retrying, the parent sees a security warning, and the link is never terminal

### Requirement: Never give up while monitoring

The `Link` SHALL keep retrying while monitoring is on. From escalation level 2 it SHALL surface a troubleshooter while retrying, with the cause hint `server_unreachable` (signaling never opens), `peer_absent` (signaling open, peer not in the room) or `media_blocked` (offer and answer done, no verification after 3 attempts). "Try now" SHALL reset the backoff.

#### Scenario: Peer never arrives

- **WHEN** signaling is open but the other device stays absent past level 2
- **THEN** the parent sees the troubleshooter with hint `peer_absent` while the link keeps retrying

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

`stop` SHALL send a stop notice over the DataChannel and over signaling before closing. On an authenticated stop notice the peer link SHALL close the verified attempt and go to `peer_stopped`: the parent raises one non-repeating alert, the baby shows a calm grey state, and no escalation runs. The link SHALL then wait until the peer leaves and rejoins the room.

#### Scenario: Baby is stopped on purpose

- **WHEN** the baby user stops monitoring
- **THEN** the parent shows one "baby unit stopped monitoring" alert and does not escalate

### Requirement: Authenticated signaling stop

A stop notice sent over signaling SHALL carry `mac = HMAC(PSK, "gs-stop/3|role|room|receiver_client_id|nonce")` with a fresh nonce. Stop notices without a valid MAC, or with a nonce already seen on this signaling connection, SHALL be ignored, so a relay cannot mute escalation.

#### Scenario: Relay forges a stop

- **WHEN** the signaling relay injects a stop notice without a valid MAC
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

Clients SHALL send an application `ping` every 15 s and treat a socket without any message for 35 s as dead. Signaling loss while an attempt is verified SHALL NOT count as monitoring loss; the `Link` SHALL reconnect signaling in the background.

#### Scenario: Signaling drops, media continues

- **WHEN** the signaling socket dies while the attempt is verified
- **THEN** the link state stays `connected` and signaling reconnects in the background

### Requirement: Clock and time jumps

The core clock SHALL be monotonic milliseconds that include deep sleep (Android `SystemClock.elapsedRealtime()`; web `Date.now()` clamped never to go backwards), ticked at least every 500 ms while monitoring. Doze gaps SHALL be treated as time jumps.

#### Scenario: Ten-minute Doze gap

- **WHEN** the next tick arrives 10 minutes after the previous one and the peer is still alive
- **THEN** the link stays connected once the peer's next message arrives within the liveness window, and escalates as normal if it does not

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
