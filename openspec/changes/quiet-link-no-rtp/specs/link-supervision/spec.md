# Spec Delta

## MODIFIED Requirements

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

The core SHALL rate a verified link as `good`, `slow` with an expected alert delay in whole seconds, or `poor`, over a 2-minute window. Its inputs SHALL be heartbeat round-trip times, late heartbeats (gap above interval + 2 s), the ICE candidate-pair round-trip time and consent replies that the platform reports every 5 s, and RTP loss while audio flows; it SHALL NOT need audio to rate the link.

#### Scenario: Quiet night on a good network

- **WHEN** no audio flows for an hour and heartbeats return within 100 ms
- **THEN** the link stays rated `good`

### Requirement: Link quality thresholds

`good` SHALL need a p90 round-trip time below 400 ms and no late heartbeat. `poor` SHALL follow from a p90 above 2 s, two late heartbeats, consent replies below 70 % or RTP loss of 10 % or more. Everything between SHALL be `slow`. A better rating SHALL apply only after 60 s in it; a worse one at once.

#### Scenario: Congested mobile cell

- **WHEN** heartbeat round-trip times rise to 1.5 s without any loss
- **THEN** the link is rated `slow` with an expected delay of 2 s within two heartbeats

#### Scenario: Short recovery

- **WHEN** a `slow` link returns to fast heartbeats for 30 s
- **THEN** it stays rated `slow` until 60 s have passed
