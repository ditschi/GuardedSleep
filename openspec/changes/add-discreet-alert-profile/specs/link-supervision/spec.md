# Spec Delta

## ADDED Requirements

### Requirement: Noise escalation timer

The parent core SHALL accept an optional escalation delay (`None` switches the timer off) and an acknowledge-noise input, SHALL track the parent's current noise event from non-probe noise alerts and loud peer heartbeats, and SHALL emit a `noise_escalated` event once per event when the delay has passed since the event start (or since the delay was set, if later) without an acknowledgment. Listen, snooze and the acknowledge-noise input SHALL count as acknowledgment. A noise event SHALL end after 2 minutes without a loud signal, and the next noise alert SHALL start a new event with a fresh count. The core SHALL take time only from its inputs and SHALL NOT know why the delay was set.

#### Scenario: Noise nobody reacts to

- **WHEN** the delay is 2 minutes, a noise alert arrives and nothing acknowledges it for 2 minutes
- **THEN** the core emits `noise_escalated` once

#### Scenario: Parent listens in time (failure of the net must not fire)

- **WHEN** the parent starts Listen 30 seconds after the alert
- **THEN** no `noise_escalated` event follows for that event

#### Scenario: Timer off

- **WHEN** the delay is `None` and noise continues for 10 minutes
- **THEN** no `noise_escalated` event is emitted

#### Scenario: New event after quiet

- **WHEN** an escalated event ended after 2 quiet minutes and noise returns
- **THEN** a new event starts and escalates only after the delay passes again
