# Spec Delta

## ADDED Requirements

### Requirement: TURN server entry

Advanced settings on Android and web SHALL let the user enter one TURN server with an address, a user name and a password. The address SHALL start with `turn:` or `turns:` and name a host, with an optional port and `transport=udp` or `transport=tcp`; user name and password SHALL be both set or both empty. Both clients SHALL offer the entry to WebRTC as one ICE server next to the relay STUN servers.

#### Scenario: Incomplete credentials

- **WHEN** the user saves a TURN address with a user name but no password
- **THEN** the entry is rejected with an error and not saved

#### Scenario: Used for the next attempt

- **WHEN** a TURN server is saved and a new attempt starts
- **THEN** the media engine is configured with the relay STUN servers and the TURN server with its user name and password

### Requirement: TURN credential protection

A saved TURN password SHALL never be shown again, logged, or kept in saved UI state; the screen SHALL only say that a password is saved. Android SHALL store the TURN entry encrypted with an Android Keystore key, separate from the pairing. The web client SHALL keep it in memory only, never in browser storage.

#### Scenario: Password not shown again

- **WHEN** the user opens Advanced after saving a TURN server with a password
- **THEN** the address and user name are shown, and the empty password field says a password is saved

#### Scenario: Keep the saved password

- **WHEN** the user saves the same address and user name with an empty password field
- **THEN** the saved password is kept

#### Scenario: Web reload

- **WHEN** the user reloads the web client after entering a TURN server
- **THEN** no TURN user name or password is found in browser storage
