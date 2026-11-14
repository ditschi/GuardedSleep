# Spec Delta

## MODIFIED Requirements

### Requirement: Escalation feedback per device

The parent SHALL render level 0 blue and silent; level 1 orange, visual only, with a low-priority notification; level 2 red with repeating vibration, a full-screen alert and a soft tone every 10 s; level 3 as an audible alarm on the alarm stream with DND bypass. The baby SHALL show no mark at level 0, a dashed edge at level 1 and a slow dim frame pulse from level 2 (`connection-status`), and SHALL never use sound or vibration.

#### Scenario: Baby side during a long outage

- **WHEN** the baby's link reaches level 3
- **THEN** the baby still shows only the dim frame pulse, without sound or vibration

#### Scenario: Short drop stays calm

- **WHEN** the parent's link has been down for 10 seconds
- **THEN** the parent shows the blue reconnecting state without sound, vibration or an alert notification, and the baby shows no mark

### Requirement: Link states

The `Link` SHALL expose exactly the states `connecting` (never verified in this run), `connected`, `reconnecting`, `peer_stopped`, `failed_needs_user{key}` and `stopped`, and SHALL expose `last_alive_ms`, `down_since_ms` and `next_level_at_ms` so the UI can show how long the peer has been unreachable and when the next escalation level starts, without its own thresholds.

#### Scenario: First start

- **WHEN** a link starts and has not verified any attempt in this run
- **THEN** its state is `connecting`, not `reconnecting`

## ADDED Requirements

### Requirement: Outage timing fields

`down_since_ms` SHALL be the last contact, or the run start before any contact. `next_level_at_ms` SHALL be when the next level starts from the configured thresholds, and `None` while connected, at level 3, while acknowledged (`acknowledged_until` applies) and in `peer_stopped`, `failed_needs_user` and `stopped`. Both SHALL use the core clock, come from stored times and stay constant within a level.

#### Scenario: Status not re-emitted per tick

- **WHEN** the link stays at level 1 for 30 seconds of ticks
- **THEN** the core emits no new status, because `down_since_ms` and `next_level_at_ms` do not change

#### Scenario: Non-default alarm threshold

- **WHEN** `alarm_after_ms` is 45 s and the link reaches level 1 after a contact at time T
- **THEN** `down_since_ms` is T and `next_level_at_ms` is T + 45 s

#### Scenario: Acknowledged

- **WHEN** the parent acknowledges a level-2 alarm
- **THEN** `next_level_at_ms` is `None` and the UI shows when sound returns from `acknowledged_until`

#### Scenario: Never connected

- **WHEN** a link has run for 25 seconds without any contact
- **THEN** `down_since_ms` is the run start and `next_level_at_ms` is the run start plus `alarm_after_ms`
