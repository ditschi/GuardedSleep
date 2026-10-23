# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Signaling URL configuration`
- TO: `### Requirement: Default signaling configuration`

## MODIFIED Requirements

### Requirement: Default signaling configuration

Release builds SHALL work without any configured signaling URL: the baby device SHALL fill the invite with its LAN mailbox when one runs and three public relays from the curated list (`signaling-mailboxes`), unless "Use only my own server" is on. A user MAY add own relay URLs in Advanced; they are added to the invite as `relay` mailboxes. Our relay SHALL treat the `signal` payload as an opaque sealed envelope.

#### Scenario: Fresh release install

- **WHEN** a release build with no configured signaling URL starts pairing on the baby device with internet
- **THEN** pairing works through the curated public relays without any manual input

#### Scenario: Own server only, nothing configured

- **WHEN** "Use only my own server" is on and no own relay is configured
- **THEN** pairing works only through the LAN mailbox, and without one the baby device explains that an own server or the same Wi-Fi is needed

### Requirement: Client ICE configuration

Clients SHALL use the STUN service of a configured own relay when there is one, plus at most two STUN servers of different operators from the curated EU list, unless "Use only my own server" is on. They SHALL NOT contact Google or other STUN servers outside the list. The operators in use SHALL be named in setup and settings. Clients SHALL use TURN only when the user has entered a TURN server. A VPN SHALL get no special handling: media MAY use VPN candidates.

#### Scenario: Default install

- **WHEN** a release build with default settings sets up an attempt
- **THEN** STUN requests go only to the two listed operators named in the connection settings

#### Scenario: Public STUN not chosen

- **WHEN** "Use only my own server" is on
- **THEN** no request goes to any STUN server other than the configured own relay

#### Scenario: No direct path between the networks

- **WHEN** an attempt fails because no candidate pair connects and no TURN server is configured
- **THEN** the parent device says that there is no direct connection between these networks and offers "use the same Wi-Fi" and "add a TURN server"
