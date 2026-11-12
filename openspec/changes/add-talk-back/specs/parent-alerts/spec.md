# Spec Delta

## MODIFIED Requirements

### Requirement: Escalation acknowledgment

Listen, Stop sound or Stop vibration, Snooze and a talk press SHALL acknowledge a discreet noise event, and a new event after the 2-minute quiet rule SHALL restart the count.

#### Scenario: Escalated event ends listening

- **WHEN** the event switched to sound and the parent then ends Listen
- **THEN** the audio keeps playing until the event ends

#### Scenario: Parent reacts in time

- **WHEN** the parent taps Stop vibration after 1 minute
- **THEN** the audio does not start for that event

#### Scenario: Parent talks to the crying child

- **WHEN** a discreet noise event is running and the parent presses the talk button
- **THEN** the event counts as acknowledged and does not escalate to sound
