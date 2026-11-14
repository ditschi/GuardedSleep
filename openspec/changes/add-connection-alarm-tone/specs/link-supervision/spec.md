# Spec Delta

## MODIFIED Requirements

### Requirement: Escalation feedback per device

The parent SHALL render level 0 yellow and silent; level 1 orange, visual only, with a low-priority notification; level 2 red with repeating vibration, a full-screen alert and the soft tone of the chosen connection alarm tone every 10 s; level 3 as an audible alarm that loops the alarm of the chosen connection alarm tone on the alarm stream with DND bypass (`parent-alerts` "Connection alarm tone"). The baby SHALL show a yellow or orange edge indicator at levels 0 and 1 and a slow dim red pulse from level 2, and SHALL never use sound or vibration.

#### Scenario: Baby side during a long outage

- **WHEN** the baby's link reaches level 3
- **THEN** the baby still shows only the dim red pulse, without sound or vibration

#### Scenario: Chosen tone at level 2

- **WHEN** the parent chose "Chime" and the link reaches level 2
- **THEN** the parent repeats the soft Chime tone every 10 s together with the vibration and the full-screen alert
