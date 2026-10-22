# Spec Delta

## Purpose

Bring one baby unit and one parent unit into the same signaling room and forward their WebRTC connection setup, while seeing only connection metadata and treating every client as untrusted. Migrated from ADR-003 §2, ADR-004 §4 and ADR-017 §10, as implemented by the current relay.

## ADDED Requirements

### Requirement: Joining a room

A client SHALL join with `hello` carrying its role, the room id and its protocol version, and SHALL receive `welcome` with a server-assigned client id. A client SHALL complete `hello` within 30 seconds of connecting, otherwise the relay closes the socket. Any other message before `hello` SHALL be answered with `NOT_JOINED`, and a second `hello` with `INVALID_MESSAGE`. For protocol v3 the room id SHALL have the shape of a derived room id; other room ids SHALL be refused.

#### Scenario: Slow client

- **WHEN** a client connects and sends no `hello` for 30 seconds
- **THEN** the relay closes the socket

### Requirement: Roles and room capacity

A room SHALL hold at most one baby unit and at most `SIGNALING_MAX_PARENTS` parent units (default 1). A second baby unit SHALL be refused with `BABY_TAKEN` and SHALL NOT take over the room. A parent beyond the cap SHALL be refused with `PARENT_CAP`.

#### Scenario: Reconnecting baby while the old socket is still alive

- **WHEN** a baby unit joins a room whose previous baby socket has not been removed yet
- **THEN** the relay answers `BABY_TAKEN` and the new socket is closed, until liveness removes the old one

### Requirement: Protocol version pinning

A room SHALL be pinned to the protocol version of its first member. A client with an unsupported version, or with a version different from the room's, SHALL be refused with `UNSUPPORTED_PROTOCOL` in reply to `hello`.

#### Scenario: v2 client joins a v3 room

- **WHEN** a v2 client sends `hello` for a room whose first member uses v3
- **THEN** it is refused with `UNSUPPORTED_PROTOCOL`

### Requirement: Membership updates

After every join and leave the relay SHALL send `members` (client ids and roles) to everyone left in the room, and SHALL tell existing members about a newcomer with `peer`. An empty room SHALL be deleted.

#### Scenario: Parent leaves

- **WHEN** the parent's socket closes
- **THEN** the baby unit receives a `members` list without the parent

### Requirement: Signal forwarding

The relay SHALL forward a `signal` payload only within the sender's room, to the member named in `to`, with `from` set by the relay to the sender's client id. Without `to` it SHALL forward to the only other member, and SHALL answer `SIGNAL_TARGET_REQUIRED` when more than two members are present. A `to` naming an unknown member or the sender itself, or a `signal` for another room, SHALL be answered with `INVALID_MESSAGE` and not forwarded. The payload SHALL be opaque to the relay.

#### Scenario: Spoofed sender

- **WHEN** a client sends a `signal` and claims another `from`
- **THEN** the forwarded message carries the sender's real client id

### Requirement: Socket liveness

The relay SHALL send a WebSocket ping to every socket every 10 seconds and terminate a socket that did not answer the previous ping, running the normal leave path (cleanup and `members`). It SHALL answer an application `ping` with `pong`.

#### Scenario: Half-open TCP connection

- **WHEN** a baby unit's Wi-Fi drops without closing its socket
- **THEN** the relay removes it within 20 seconds and the parent receives updated `members`

### Requirement: Resource and rate limits

The relay SHALL reject messages larger than 64 KiB and answer unparsable messages with `BAD_JSON`. It SHALL limit `hello` attempts (default 20 per client IP and minute) and new rooms (default 10 per client IP and minute), close the socket with `RATE_LIMITED` when a limit is exceeded, and take the client IP from `X-Forwarded-For` only for a configured number of trusted proxy hops.

#### Scenario: Room stuffing

- **WHEN** one client IP tries to create an eleventh room within a minute
- **THEN** the relay answers `RATE_LIMITED` and closes the socket

### Requirement: Origin allowlist

In production the relay SHALL refuse to start without an allowlist of browser origins, and SHALL refuse WebSocket upgrades from browser origins not on it. Clients without an `Origin` header (native apps) SHALL be accepted.

#### Scenario: Foreign website

- **WHEN** a page on another origin opens a WebSocket to the production relay
- **THEN** the upgrade is refused

### Requirement: Metadata only, no IP logging

The relay SHALL carry only connection metadata (room membership and opaque setup messages), never media. It SHALL NOT log client IP addresses or room ids. Reverse proxies in front of it SHALL allow at least 35 seconds of idle time.

#### Scenario: Normal session

- **WHEN** a baby unit and a parent unit pair and monitor through the relay
- **THEN** the relay's logs contain neither their IP addresses nor their room id

### Requirement: Client ICE configuration

The Android client SHALL use the public EU STUN servers `stun.sipgate.de:3478` and `stun.linphone.org:3478` and no Google STUN. Release builds SHALL ship without a default signaling URL; the user enters the signaling URL list on the baby unit, and the invite carries it to the parent. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Fresh release install

- **WHEN** a release build without a configured signaling URL starts pairing on the baby unit
- **THEN** pairing fails as unreachable until a signaling URL is entered in the settings
