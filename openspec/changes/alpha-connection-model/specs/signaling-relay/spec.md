# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Signaling URL configuration`
- TO: `### Requirement: Default signaling configuration`

## MODIFIED Requirements

### Requirement: Default signaling configuration

Release builds SHALL work without any configured signaling URL: the baby unit SHALL fill the invite with Nostr relays from the curated list (`signaling-mailboxes`), unless "Only same Wi-Fi or my own server" is on. A user MAY add own relay URLs in Advanced; they are added to the invite as own-relay mailboxes. The relay SHALL treat every `signal` payload as an opaque sealed envelope.

#### Scenario: Fresh release install

- **WHEN** a release build with no configured signaling URL starts pairing on the baby unit with internet (in CI: against the local Nostr test relay)
- **THEN** pairing completes through the public relays without any manual input

#### Scenario: Own server only, nothing configured

- **WHEN** "Only same Wi-Fi or my own server" is on and no own relay is configured
- **THEN** the baby unit explains that pairing needs the same Wi-Fi or an own server, and no public service is contacted

### Requirement: Client ICE configuration

With an own relay mailbox configured, clients SHALL use only that relay's STUN service. Otherwise they SHALL use the first two STUN servers of distinct operators from the curated EU list, named in the connection settings, unless "Only same Wi-Fi or my own server" is on, which allows no public STUN at all. Clients SHALL NOT contact Google or other STUN servers outside the list, and SHALL use TURN only when the user has entered a TURN server. A VPN SHALL get no special handling: media MAY use VPN candidates.

#### Scenario: Public STUN not chosen

- **WHEN** "Only same Wi-Fi or my own server" is on
- **THEN** no request goes to any STUN server other than a configured own relay

#### Scenario: Default install

- **WHEN** a release build with default settings and no own relay sets up an attempt
- **THEN** STUN requests go only to the two listed operators named in the connection settings

#### Scenario: Google STUN in the code

- **WHEN** a change adds a Google or other unlisted public STUN host to client source or configuration
- **THEN** `just lint` fails

#### Scenario: No direct path between the networks

- **WHEN** attempts fail because no candidate pair connects and no TURN server is configured
- **THEN** the level-2 troubleshooter shows the `media_blocked` cause in plain words with "Try again" and "Put both on the same Wi-Fi", the TURN setting only under Advanced, and the alarm is not replaced
