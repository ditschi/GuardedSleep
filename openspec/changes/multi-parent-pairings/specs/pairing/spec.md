# Spec Delta

## MODIFIED Requirements

### Requirement: One baby unit and one parent unit

A pairing SHALL connect exactly one baby unit and one parent unit. The invite SHALL close once the baby unit has pinned the first parent, and our relay SHALL refuse a second parent in the room with `PARENT_CAP`. A second parent device SHALL join a baby unit only through its own pairing, with its own invite and PSK.

#### Scenario: Second parent in the room

- **WHEN** a second parent unit joins a room that already has a parent
- **THEN** the relay refuses it with `PARENT_CAP` and the existing pairing is unaffected

#### Scenario: Second parent device

- **WHEN** a baby unit with one saved parent pairing adds a second parent device
- **THEN** it shows a new invite with a fresh PSK, and the first pairing keeps its PSK, room and pin

## ADDED Requirements

### Requirement: Saved pairings and connection names

A device SHALL keep at most 2 saved pairings per role, each stored and encrypted as an independent pairing. Every saved pairing SHALL have a local connection name of 1 to 30 characters, by default the peer's device name where the device learns one, that the user can change. The name SHALL be stored encrypted with its pairing and SHALL NOT be sent to any peer, relay or log. Forgetting SHALL delete exactly one saved pairing.

#### Scenario: Third pairing

- **WHEN** a device already holds 2 saved pairings in its current role
- **THEN** it offers no further pairing until one is forgotten

#### Scenario: Forget one parent device

- **WHEN** the user forgets one of two parent pairings on the baby unit
- **THEN** that pairing is deleted and the other parent still connects without a new scan

#### Scenario: Default name on the parent unit

- **WHEN** a parent unit saves a pairing, whose protocol carries no baby device name
- **THEN** the connection shows a numbered default such as "Baby unit 1" until the user renames it

#### Scenario: Renamed connection

- **WHEN** the user renames a connection to "Home – old phone"
- **THEN** the name is shown in lists and status lines on this device, and no message to the peer contains it
