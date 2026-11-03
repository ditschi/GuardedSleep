# Spec Delta

## MODIFIED Requirements

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

### Requirement: Offer direction

Only the baby SHALL create offers, and only when it has no live verified attempt and a parent is present. A parent is present when the pinned parent (or, before pinning, any parent) has a confirmed current session (announced with `here` on any mailbox) and, for a relay mailbox, is a member of that relay's room with the client id the session last spoke from. The parent SHALL send `request_offer` to the baby's current session when it starts or recovers without a live attempt. An offer answering a request SHALL carry `re` set to that request's id, and the parent SHALL adopt an offer only for its current request. A baby that sees a parent present SHALL wait a short grace period (2.5 s) for that request before it offers unasked, so that a clean setup builds one attempt per side.

#### Scenario: Both sides start at the same time

- **WHEN** the parent and the baby start within the same tick and the parent's request crosses an earlier offer
- **THEN** the parent adopts only the offer whose `re` matches its request, and the two devices converge on one verified attempt

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

### Requirement: Never give up while monitoring

The `Link` SHALL keep retrying while monitoring is on. From escalation level 2 it SHALL surface a troubleshooter while retrying, with the cause hint `server_unreachable` (no mailbox opens), `peer_absent` (at least one mailbox open, no current `here` or room membership of the peer), or `media_blocked` (offer and answer done, no verification after 3 attempts). "Try now" SHALL reset the backoff.

#### Scenario: Peer never arrives

- **WHEN** mailboxes are open but the other device announces no session past level 2
- **THEN** the parent sees the troubleshooter with hint `peer_absent` while the link keeps retrying

#### Scenario: Project server unreachable

- **WHEN** the project server is the only mailbox and it cannot be reached
- **THEN** the parent sees the troubleshooter with hint `server_unreachable` and the link keeps retrying it on the normal backoff

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

### Requirement: Signaling transport liveness

On a relay mailbox, clients SHALL send an application `ping` every 15 s and treat a socket without any message for 35 s as dead. Mailbox loss while an attempt is verified SHALL NOT count as monitoring loss.

#### Scenario: Signaling drops, media continues

- **WHEN** a mailbox socket dies while the attempt is verified
- **THEN** the link state stays `connected`

#### Scenario: Half-open relay socket in a browser

- **WHEN** a relay socket in the browser stops delivering without closing while the link is down
- **THEN** after 35 s without any message the core treats it as dead and reopens that mailbox

### Requirement: Clock and time jumps

The core clock SHALL be monotonic milliseconds that include deep sleep (Android `SystemClock.elapsedRealtime()`; web `Date.now()` clamped never to go backwards), ticked at least every 500 ms while monitoring. Doze gaps SHALL be treated as time jumps. Separately, the platform SHALL report the wall clock in Unix seconds at start, every 60 seconds and after a time change; it SHALL be used only for the invite expiry, the pin window and the clock comparison with the peer, never for liveness, escalation or any mailbox.

#### Scenario: Ten-minute Doze gap

- **WHEN** the next tick arrives 10 minutes after the previous one and the peer is still alive
- **THEN** the link stays connected once the peer's next message arrives within the liveness window, and escalates as normal if it does not

#### Scenario: Wall clock jumps by an hour

- **WHEN** the user changes the device time by an hour during monitoring
- **THEN** liveness and escalation timing are unaffected, and no mailbox is reopened because of it

#### Scenario: Clocks of the two devices disagree

- **WHEN** a `here` of the peer's confirmed session carries a wall clock more than 10 minutes from this device's
- **THEN** both devices raise the `clock_skew` hint, and signaling and escalation are unaffected

### Requirement: Loss detection

A verified attempt SHALL be considered lost when its peer connection is `failed` or `closed`, its DataChannel closes, or no DataChannel message arrives for `liveness_ms = 2 × interval + 5 s` (25 s default, 35 s maximum). The `disconnected` state alone, absence from a relay room and the closing of mailboxes SHALL NOT count as loss.

#### Scenario: Peer goes silent

- **WHEN** no DataChannel message arrives from the peer for `liveness_ms`
- **THEN** the `Link` closes the attempt and moves to `reconnecting`

#### Scenario: Signaling server restarts while connected

- **WHEN** the signaling server restarts and the peer briefly disappears from `members`
- **THEN** the verified attempt and the link state stay unchanged
