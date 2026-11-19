# Spec Delta

## MODIFIED Requirements

### Requirement: Forget pairing

"Forget" SHALL stop monitoring, cancel an open invite, delete the stored pairing and the peer name of the current role, cancel snooze, and restore the phone's Do Not Disturb state if the app had changed it. The pairing of the other role SHALL be kept.

#### Scenario: Forget on the baby unit

- **WHEN** the user forgets the pairing on a baby unit that silenced the phone
- **THEN** the baby pairing is deleted and the previous Do Not Disturb state is restored

#### Scenario: Other role kept

- **WHEN** the user forgets the pairing as parent unit on a phone that also has a baby pairing
- **THEN** the baby pairing is still stored

## ADDED Requirements

### Requirement: One stored pairing per role

A device SHALL store at most one pairing per role: one as baby unit and one as parent unit. Each SHALL be an independent pairing with its own PSK, invite and pins, under every other requirement of this capability, and only the pairing of the current role SHALL be used.

#### Scenario: Both directions paired

- **WHEN** phone A paired as baby unit with phone B, and later B paired as baby unit with A
- **THEN** each phone holds two pairings and either can start monitoring in either role without a new scan
