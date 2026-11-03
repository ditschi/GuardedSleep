# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Signaling URL configuration`
- TO: `### Requirement: Default signaling configuration`

## MODIFIED Requirements

### Requirement: Default signaling configuration

Android release builds and the official web deployment SHALL work without any configured signaling URL: the baby unit SHALL fill the invite with the project signaling server (`signaling-mailboxes` "Project signaling server"), unless "Only same Wi-Fi or my own server" is on. Other web deployments SHALL default to their own origin's `/signaling`. A user MAY add own relay URLs in Advanced; they are added to the invite as own-relay mailboxes. The relay SHALL treat every `signal` payload as an opaque sealed envelope.

#### Scenario: Fresh release install

- **WHEN** a release build with no configured signaling URL starts pairing on the baby unit with internet (in CI: against the local server of the test stack)
- **THEN** pairing completes through the project server without any manual input

#### Scenario: Self-hosted web deployment

- **WHEN** a web build without the official-deployment flag starts pairing on the baby unit
- **THEN** the invite names its own origin's `/signaling` and never the project server

#### Scenario: Own server only, nothing configured

- **WHEN** "Only same Wi-Fi or my own server" is on and no own relay is configured
- **THEN** the baby unit explains that pairing needs the same Wi-Fi or an own server, and no internet service is contacted

### Requirement: Client ICE configuration

Clients SHALL take STUN and TURN servers only from the relays of the pairing (the ICE servers a relay supplied, or otherwise the relay's built-in STUN on port 3478) and from a user-entered TURN server. With "Only same Wi-Fi or my own server" on and no own relay, they SHALL use no STUN or TURN server at all. Clients SHALL NOT contact Google or any other STUN host outside the pairing's relays and `data/stun-servers.json`. A VPN SHALL get no special handling: media MAY use VPN candidates.

#### Scenario: Default install

- **WHEN** a release build with default settings sets up an attempt after the project server supplied ICE servers
- **THEN** STUN and TURN requests go only to the project server's host

#### Scenario: Public STUN not chosen

- **WHEN** "Only same Wi-Fi or my own server" is on and no own relay is configured
- **THEN** no request goes to any STUN or TURN server

#### Scenario: Google STUN in the code

- **WHEN** a change adds a Google or other unlisted public STUN or TURN host to client source or configuration
- **THEN** `just lint` fails

#### Scenario: No direct path between the networks

- **WHEN** attempts fail because no candidate pair connects, not even through TURN
- **THEN** the level-2 troubleshooter shows the `media_blocked` cause in plain words with "Try again" and "Put both on the same Wi-Fi", the TURN setting only under Advanced, and the alarm is not replaced

### Requirement: Metadata only, no IP logging

The relay SHALL carry only connection metadata (room membership, opaque setup messages and the ICE servers it issues), never media. It SHALL NOT log client IP addresses, room ids, TURN user names or TURN credentials. Reverse proxies in front of it SHALL allow at least 35 seconds of idle time. The project's public deployment SHALL log no client IP address in the signaling server, the reverse proxy or the TURN server.

#### Scenario: Normal session

- **WHEN** a baby unit and a parent unit pair and monitor through the relay
- **THEN** the relay's logs contain neither their IP addresses nor their room id

#### Scenario: Relayed call on the public deployment

- **WHEN** a test client pairs and monitors through the project server with media relayed through its TURN server
- **THEN** no log on the host (the stack's containers, the reverse proxy, the system journal) contains the test client's IP address

## ADDED Requirements

### Requirement: ICE servers from the relay

A relay configured with STUN URLs SHALL include them in `welcome`. A relay configured with TURN SHALL send each member of a room holding at least one baby unit and one parent unit an `ice_servers` message with the URLs and a credential of its own, before the membership update that completes the room, and a fresh one at half the credential lifetime while both stay joined. It SHALL NOT send TURN credentials into a room with one role only, and SHALL NOT accept a client request for them.

#### Scenario: Second member joins

- **WHEN** a parent unit joins a room in which the baby unit waits, on a relay with TURN configured
- **THEN** each of them receives an `ice_servers` message with its own credential before it learns that the other is present

#### Scenario: Lone socket

- **WHEN** a client joins a new room and stays alone in it
- **THEN** it receives at most the STUN URLs in `welcome` and never a TURN credential

### Requirement: TURN credential caps

A relay SHALL keep one current TURN credential per member, a renewal superseding the previous one, and at most a configured number of current credentials per client address (an IPv4 address or an IPv6 /64).

#### Scenario: Many credentials from one address

- **WHEN** one client address already holds the configured number of live credentials and joins another complete room
- **THEN** the relay issues it no further credential until one expires, and the room still works without TURN

### Requirement: Host-bound ICE servers in the client

Clients SHALL accept ICE URLs from a relay only when every URL uses `stun:`, `turn:` or `turns:` and its host equals that relay's host, and SHALL drop the whole message otherwise. A new attempt SHALL get the union of the sets still valid for at least another 30 seconds, the relay's built-in STUN for each relay without a set, and nothing when the pairing has no relay mailbox.

#### Scenario: Relay names a foreign TURN host

- **WHEN** a relay sends an `ice_servers` message with a URL whose host differs from the relay's own host
- **THEN** the client drops the whole message and uses none of its URLs

#### Scenario: Credential near its expiry

- **WHEN** a new attempt starts 590 seconds after a 600-second credential arrived and no newer one exists
- **THEN** the attempt is configured without that TURN credential

### Requirement: Short-lived TURN credentials

TURN credentials SHALL follow the coturn REST scheme: the user name is the expiry time in Unix seconds, a colon and 16 random bytes in base64url, never the room id or a value derived from the PSK; the password is the base64 HMAC-SHA1 of the user name under the TURN secret shared with the TURN server. The lifetime SHALL be at most 600 seconds.

#### Scenario: Credential checked by coturn

- **WHEN** a client uses an issued credential at a coturn configured with `--use-auth-secret` and the same secret before it expires
- **THEN** coturn accepts the allocation

#### Scenario: Expired credential

- **WHEN** a client starts a new allocation with a credential whose expiry has passed
- **THEN** coturn refuses it

#### Scenario: Relayed night

- **WHEN** a relayed call runs for 15 minutes on an allocation made with a 60-second credential
- **THEN** the allocation keeps relaying, so a relayed night does not drop when its credential expires

### Requirement: TURN secret and credential handling

The TURN secret SHALL come only from the deployment environment, never from source, an image or a client, and the relay SHALL never log it. Clients SHALL never log, persist, put into an invite or a mailbox update, or forward a TURN credential.

#### Scenario: Paired web client reloads

- **WHEN** a web client that received a TURN credential is reloaded
- **THEN** no TURN credential is found in browser storage, and the log canary finds none in any log
