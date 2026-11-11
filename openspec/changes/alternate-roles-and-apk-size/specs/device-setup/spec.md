# Spec Delta

## MODIFIED Requirements

### Requirement: Role choice

The role, baby unit or parent unit, SHALL be chosen per session from two large choices. A device SHALL have exactly one role at a time and SHALL keep the pairing of each role when the user switches; a role SHALL never be changed remotely. The setting "Start mode" SHALL offer _Ask every time_, _Baby unit_ and _Parent unit_, and the role screen SHALL offer "Use as default", which sets it. When the app starts with monitoring not active, it SHALL open in the default role, or show the role screen when there is none. When it starts with monitoring active, it SHALL resume the running role.

#### Scenario: Start without a default

- **WHEN** the app starts on a device whose start mode is _Ask every time_ and monitoring is not active
- **THEN** it shows the role screen

#### Scenario: Second start

- **WHEN** the app starts on a device whose start mode is _Parent unit_ and monitoring is not active
- **THEN** it opens as parent unit without asking

#### Scenario: Restart during monitoring

- **WHEN** the process restarts while the baby unit is monitoring and the default is _Parent unit_
- **THEN** the app resumes as baby unit

#### Scenario: Update from a single-role install

- **WHEN** a device that chose a role before this change starts for the first time after the update
- **THEN** its start mode is that role and its pairing belongs to that role

## ADDED Requirements

### Requirement: Switching mode

While monitoring is not active, the home and pairing screens SHALL offer "Switch mode", which cancels an open invite and shows the role screen. Switching SHALL NOT change the start mode and SHALL NOT delete a pairing. While monitoring is active, including the baby kiosk, no switch SHALL be offered.

#### Scenario: Swap for one night

- **WHEN** a phone whose default is _Parent unit_ switches to baby unit on its home screen
- **THEN** it opens as baby unit with its baby pairing, and the next start without monitoring opens as parent unit again with the parent pairing intact

#### Scenario: Monitoring running

- **WHEN** the parent unit is monitoring
- **THEN** no "Switch mode" action is shown
