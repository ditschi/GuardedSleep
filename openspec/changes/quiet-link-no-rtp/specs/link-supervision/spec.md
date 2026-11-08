# Spec Delta

## MODIFIED Requirements

### Requirement: Supervision lives only in the core Link

The Rust core `Link` SHALL own the state of every mailbox, attempts, liveness, escalation, listen intent, snooze, peer status, the link quality rating and the missed-event buffer. Platforms SHALL talk only to the `Link` and SHALL NOT run their own reconnect loops, mailbox failover, timer-based alarms or quality ratings; they open, close and feed the mailboxes the `Link` asks for, and report measurements to it. A `Link` SHALL NOT be constructed without a PSK.

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
- **THEN** the baby ignores the request until its liveness timeout (15 s by default) closes the old attempt, and recovery completes below escalation level 2

### Requirement: Loss detection

A verified attempt SHALL be considered lost when its peer connection is `failed` or `closed`, its DataChannel closes, or no DataChannel message arrives for `liveness_ms = 2 × interval + 5 s` (15 s default, 35 s maximum). The `disconnected` state alone, absence from a relay room and the closing of mailboxes SHALL NOT count as loss.

#### Scenario: Peer goes silent

- **WHEN** no DataChannel message arrives from the peer for `liveness_ms`
- **THEN** the `Link` closes the attempt and moves to `reconnecting`

#### Scenario: Signaling server restarts while connected

- **WHEN** the signaling server restarts and the peer briefly disappears from `members`
- **THEN** the verified attempt and the link state stay unchanged

### Requirement: Bidirectional heartbeat

Both sides SHALL send `heartbeat` over the DataChannel every `heartbeat_interval_ms` (default 5 s, configurable 5–15 s). Each heartbeat SHALL carry the `seq` of the last heartbeat received from the peer (`ack`) and the milliseconds it held that value before sending (`ack_hold_ms`). The heartbeat `t` field SHALL be opaque to the receiver. `last_alive` SHALL be the time of the last DataChannel message from the peer, and all escalation timing SHALL be measured from it.

#### Scenario: Baby learns whether a parent listens

- **WHEN** the parent's heartbeats stop arriving at the baby
- **THEN** the baby's `last_alive` stops advancing and the baby detects the loss like the parent would

#### Scenario: Round-trip time without synchronised clocks

- **WHEN** a heartbeat arrives whose `ack` names a heartbeat this side sent
- **THEN** the core computes the round-trip time as now minus that send time minus `ack_hold_ms`

## ADDED Requirements

### Requirement: Link quality from the alert path

The core SHALL rate a verified link as `good`, `slow` with an expected alert delay in whole seconds, or `poor`, over a 2-minute window, from heartbeat round-trip times and late heartbeats (gap above interval + 2 s). Where the platform reports them every 5 s, the ICE candidate-pair round-trip time, consent replies and the local candidate type (never addresses) SHALL be added, and RTP loss while audio flows. The rating SHALL NOT need audio or ICE statistics.

#### Scenario: Quiet night on a good network

- **WHEN** no audio flows for an hour and heartbeats return within 100 ms
- **THEN** the link stays rated `good`

#### Scenario: Browser without ICE statistics

- **WHEN** the platform reports no ICE round-trip time or consent counters
- **THEN** the core rates the link from heartbeats alone

### Requirement: Link quality thresholds

`good` SHALL need a p90 round-trip time below 400 ms and no late heartbeat. `poor` SHALL follow from a p90 above 2 s, two late heartbeats, consent replies below 70 % or RTP loss of 10 % or more. Everything between SHALL be `slow`. A better rating SHALL apply only after 60 s in it; a worse one at once.

#### Scenario: Congested mobile cell

- **WHEN** heartbeat round-trip times rise to 1.5 s without any loss
- **THEN** the link is rated `slow` with an expected delay of 2 s within two heartbeats

#### Scenario: Short recovery

- **WHEN** a `slow` link returns to fast heartbeats for 30 s
- **THEN** it stays rated `slow` until 60 s have passed
