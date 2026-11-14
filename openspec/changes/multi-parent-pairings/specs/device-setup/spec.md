# Spec Delta

## ADDED Requirements

### Requirement: Connection choice per role

With more than one saved pairing in the current role, the parent home screen SHALL preselect the connection used last in this app start, else the default connection, by name and offer "Change" for this session; Settings SHALL list the connections with rename, set and remove default, and forget. With one saved pairing no list SHALL be shown. A default connection on the parent starts automatically at app start (`parent-pairing-entry`, owner 2026-11-14).

#### Scenario: Travelling

- **WHEN** a parent whose default is "Home – old phone" picks "Travel – partner's phone"
- **THEN** monitoring uses that pairing for this session and the next app start connects to the default again

### Requirement: Adding a second parent device

While not monitoring and with fewer than 2 saved parent pairings, the Android baby unit SHALL offer "Add parent device", which shows a new invite. While monitoring it SHALL NOT be offered.

#### Scenario: Add the partner's phone

- **WHEN** a baby unit with one saved parent pairing selects "Add parent device"
- **THEN** it shows a new invite, and after the second parent verified both pairings are saved
