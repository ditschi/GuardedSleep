# Spec Delta

## ADDED Requirements

### Requirement: Connection alarm tone

The parent device (Android and browser) SHALL let the user choose the connection alarm tone from five built-in, code-generated tones in Settings › Alerts, with today's tone as the default. Each tone SHALL have a soft tone for the "no connection" stage and for a baby device stopped on purpose, and a looping alarm for the alarm stage (`link-supervision`). The choice SHALL be stored on the parent device and SHALL NOT be part of any pairing material. Noise alerts SHALL stay unchanged.

#### Scenario: Choosing a tone

- **WHEN** the parent selects "Beacon" in the connection alarm tone picker and later the connection is lost for more than 3 minutes
- **THEN** the alarm stage loops the Beacon alarm, and the choice is still active after the app restarts

#### Scenario: Noise alert unchanged

- **WHEN** a noise event arrives with the alert style "Sound" and any connection alarm tone selected
- **THEN** the parent device plays the child's audio and no connection alarm tone

### Requirement: Connection alarm tone is never silent

The tone recipes and the resolution of a stored choice SHALL live in the core. Any unknown or unreadable stored value SHALL resolve to the default tone. There SHALL be no silent option and no system or user ringtone, and every tone SHALL meet a minimum loudness checked by tests.

#### Scenario: Broken stored value

- **WHEN** the stored tone id is unknown, for example after a downgrade
- **THEN** the parent device uses the default tone, and the picker shows the default as selected

#### Scenario: No silent choice

- **WHEN** the parent opens the connection alarm tone picker
- **THEN** it offers only the five tones, none of them silent, and no system ringtone

### Requirement: Connection alarm tone preview and test

Each tone SHALL be playable as a preview, and a "Test connection alarm" action SHALL play the chosen tone on the alarm stream through the chosen alert output. The picker SHALL state that noise alerts still play the child's own sound. A real alert SHALL stop any preview or test at once.

#### Scenario: Preview at the real volume

- **WHEN** the parent taps "Test connection alarm"
- **THEN** the parent device plays the soft tone and then the alarm of the chosen tone on the alarm stream for about 8 seconds, and a Stop action ends it at once

#### Scenario: Real alert during a preview

- **WHEN** a noise event or a connection stage with sound starts while a preview or test plays
- **THEN** the preview or test stops at once and the alert sounds as specified
