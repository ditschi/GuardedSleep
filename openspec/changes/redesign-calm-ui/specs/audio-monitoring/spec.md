# Spec Delta

## MODIFIED Requirements

### Requirement: Listen mode

The parent SHALL start listening with one tap on Listen and stop it with another tap; while listening, the Listen control SHALL fill and say "Listening · tap to stop". Listening SHALL stop by itself after 60 minutes and SHALL be unavailable while not connected. The baby unit SHALL show "Parent is listening" only when someone wakes its screen.

#### Scenario: Forgotten open microphone

- **WHEN** the parent starts listening and forgets it
- **THEN** listening stops after 60 minutes

#### Scenario: One tap at 3 AM

- **WHEN** the parent taps Listen once while connected
- **THEN** the child's audio plays, the control reads "Listening · tap to stop", and a second tap stops it

### Requirement: Parent monitoring screen

While connected, the parent screen SHALL show the last minute of sound against the alert threshold line, the baby unit's battery and charging state, and "All quiet since {time}" when there was no recent noise. "All quiet" SHALL NOT be shown while disconnected, while the microphone is silent or the baby unit stopped, or after a recovery unless the missed-events report covers the whole outage; after a talk it SHALL restart.

#### Scenario: Reconnected after the baby process restarted

- **WHEN** the link recovers and the missed-events report is incomplete
- **THEN** the parent does not see "All quiet since" for the outage

#### Scenario: Microphone hears nothing (failure)

- **WHEN** the core reports `mic_silent` while the link is connected
- **THEN** the parent screen says it can't hear the room, shows no "All quiet", and the sound strip shows that no sound comes in

#### Scenario: After a talk

- **WHEN** the parent's talk ends at 02:10 after a quiet night
- **THEN** the screen shows "All quiet since 02:10", because detection was paused during the talk
