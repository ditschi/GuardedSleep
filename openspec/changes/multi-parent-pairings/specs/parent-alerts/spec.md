# Spec Delta

## ADDED Requirements

### Requirement: Shared noise acknowledgment

When a parent stops the sound or vibration of a noise alert, it SHALL send `noise_ack` with the event id, and the baby unit SHALL forward it to the other trusted parent link only for the current event. The other parent SHALL stop ringing for that event and show that it was acknowledged on the other parent device; this SHALL NOT count as its own acknowledgment, so its noise escalation timer keeps running and noise that continues alerts it again.

#### Scenario: Mother acknowledges

- **WHEN** both parents get a noise alert and parent A stops its sound
- **THEN** parent B's alert stops and shows "Acknowledged on the other parent device"

#### Scenario: Noise continues

- **WHEN** parent A acknowledged and the noise continues past parent B's noise escalation timer
- **THEN** parent B is alerted again

### Requirement: Limits of shared acknowledgment

Snooze and listening SHALL NOT send `noise_ack`. Acknowledgments for an unknown or earlier event id SHALL be ignored. Connection alarms and test alerts SHALL never be acknowledged through the other parent.

#### Scenario: Stale acknowledgment

- **WHEN** an acknowledgment for an earlier event reaches the baby unit during a new event
- **THEN** it is not forwarded and the new alert keeps ringing on both parents

#### Scenario: Snooze is not shared

- **WHEN** parent A snoozes noise alerts
- **THEN** parent B keeps receiving noise alerts

#### Scenario: Connection alarm stays

- **WHEN** parent B's link is down and parent A acknowledges a noise alert
- **THEN** B's connection alarm keeps escalating on B's phone
