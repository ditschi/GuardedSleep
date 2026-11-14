# Spec Delta

## MODIFIED Requirements

### Requirement: Escalation feedback per device

The parent SHALL render level 0 blue and silent; level 1 orange, visual only, with a low-priority notification; level 2 red with repeating vibration, a full-screen alert and a soft tone every 10 s; level 3 as an audible alarm on the alarm stream with DND bypass. The baby SHALL show no mark at level 0, a dashed edge at level 1 and a slow dim frame pulse from level 2 (`connection-status`), and SHALL never use sound or vibration.

#### Scenario: Baby side during a long outage

- **WHEN** the baby's link reaches level 3
- **THEN** the baby still shows only the dim frame pulse, without sound or vibration

#### Scenario: Short drop stays calm

- **WHEN** the parent's link has been down for 10 seconds
- **THEN** the parent shows the blue reconnecting state without sound, vibration or notification, and the baby shows no mark
