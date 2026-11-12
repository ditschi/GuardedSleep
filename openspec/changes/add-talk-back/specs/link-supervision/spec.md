# Spec Delta

## MODIFIED Requirements

### Requirement: Sans-IO interface contract

The `Link` SHALL be driven only through serde-JSON inputs and actions (snake_case, `type`-tagged); `LinkInput` and `LinkAction` in `crates/guardedsleep-core/src/link.rs` are the contract for every binding. The core SHALL export `message_keys()` so the i18n check can verify that each key exists in `locales/en.json`. Every local session description SHALL contain exactly two audio m-lines, the monitoring line first and the talk line second, otherwise the core refuses it: the baby's monitoring line SHALL be `sendonly` and its talk line `recvonly`; the parent's monitoring line SHALL be `recvonly` and its talk line `sendonly` or `inactive`. The baby core SHALL also read the talk line's direction from the accepted remote answer: `sendonly` allows talk on that attempt; `inactive`, a rejected line (port 0) or a missing line SHALL mean that this parent cannot talk on that attempt; a monitoring line in the answer that is not `recvonly` SHALL fail the attempt.

#### Scenario: Baby SDP would receive audio

- **WHEN** the baby platform reports a local description whose first audio m-line is not `sendonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Baby SDP would send on the talk line

- **WHEN** the baby platform reports a local description whose second audio m-line is not `recvonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Parent SDP would send on the monitoring line

- **WHEN** the parent platform reports a local description whose first audio m-line is not `recvonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Parent without talk support

- **WHEN** the parent's answer marks the talk line `inactive` or rejects it
- **THEN** the attempt verifies as usual and the baby unit denies talk requests on it as not supported

#### Scenario: Answer would send on the monitoring line (failure)

- **WHEN** the parent's answer marks the monitoring line `sendrecv` or `sendonly`
- **THEN** the baby core fails the attempt

### Requirement: Noise escalation timer

The parent core SHALL accept an optional escalation delay (`None` switches the timer off) and an acknowledge-noise input, and SHALL emit a `noise_escalated` event once per noise event when the delay has passed since the event start (or since the delay was set, if later) without an acknowledgment. Listen, snooze, a talk press and the acknowledge-noise input SHALL count as acknowledgment. The core SHALL take time only from its inputs and SHALL NOT know why the delay was set.

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

#### Scenario: Parent talks in time

- **WHEN** the parent presses the talk button 30 seconds after the alert
- **THEN** no `noise_escalated` event follows for that event
