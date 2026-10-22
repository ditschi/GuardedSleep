# signaling-relay Specification

## Purpose

Bring one baby unit and one parent unit into the same signaling room and forward their WebRTC connection setup, while seeing only connection metadata and treating every client as untrusted. Migrated from ADR-003 §2, ADR-004 §4 and ADR-017 §10, as implemented by the current relay.

## Requirements

### Requirement: Joining a room

A client SHALL join a room within 30 seconds of connecting, with `hello` (role, room id, protocol version), and receive `welcome` with a server-assigned client id. On a relay that requires admission it SHALL instead answer the relay's `challenge` with `admit`. Any other message before joining SHALL be answered with `NOT_JOINED`, a second join message with `INVALID_MESSAGE`. A protocol v3 room id SHALL have the shape of a derived room id; other room ids SHALL be refused.

#### Scenario: Slow client

- **WHEN** a client connects and sends no `hello` for 30 seconds
- **THEN** the relay closes the socket

#### Scenario: Join on a relay without admission

- **WHEN** a client sends `hello` to a relay that runs in Open mode, such as the public server
- **THEN** it receives `welcome`, with no `challenge` before it

#### Scenario: Hello on a relay that requires admission

- **WHEN** a client sends `hello` to a relay that requires admission
- **THEN** the relay answers `ADMISSION_REQUIRED` and closes the socket

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

The relay SHALL reject messages larger than 64 KiB and answer unparsable messages with `BAD_JSON`. Per client IP it SHALL limit join attempts (default 20 per minute), new rooms (10 per minute) and open sockets (16); per socket it SHALL limit frames (120 per minute before joining, 600 after). Exceeding a limit SHALL close the socket. It SHALL take the client IP from `X-Forwarded-For` only for a configured number of trusted proxy hops.

#### Scenario: Room stuffing

- **WHEN** one client IP tries to create an eleventh room within a minute
- **THEN** the relay answers `RATE_LIMITED` and closes the socket

#### Scenario: Socket hoarding

- **WHEN** one client IP opens a seventeenth socket while sixteen are open
- **THEN** the relay closes the new socket as `server full`, and sockets from other client IPs are unaffected

#### Scenario: Frame flood

- **WHEN** a socket sends frames faster than its budget allows (a burst of 120 that refills at 120 per minute before joining; 600 once joined)
- **THEN** the relay answers `RATE_LIMITED` and closes the socket

#### Scenario: Behind a reverse proxy

- **WHEN** the server runs behind one trusted proxy with the trust setting on
- **THEN** the limits count the address the proxy appended as the last `X-Forwarded-For` hop, and never log it

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

The Android client SHALL use the STUN service of its configured relay by default and SHALL NOT contact any public STUN server by default. A third-party STUN server SHALL be used only after the user opts in, with copy that names who sees the device's IP address. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Public STUN not chosen

- **WHEN** the user has not opted in to a third-party STUN server
- **THEN** no request goes to any STUN server other than the configured relay

### Requirement: Signaling URL configuration

Release builds SHALL ship without a default signaling URL; the user enters the signaling URL list on the baby unit, and the invite carries it to the parent.

#### Scenario: Fresh release install

- **WHEN** a release build without a configured signaling URL starts pairing on the baby unit
- **THEN** pairing fails as unreachable until a signaling URL is entered in the settings

### Requirement: Built-in STUN

The signaling server SHALL answer STUN Binding requests only, on UDP port 3478, enabled by default; an operator MAY disable it or move its port. A response SHALL be at most 44 bytes and at most 2.2 times the size of its request, and each source address SHALL be limited by a per-IP rate limit (default 60 per minute) and a global cap bounds the total answer rate, so the server is useless as a traffic amplifier while bare 20-byte Binding requests from browsers and libwebrtc are answered.

#### Scenario: Browser Binding request

- **WHEN** a browser sends a 20-byte Binding request without attributes
- **THEN** the server answers with its XOR-MAPPED-ADDRESS

#### Scenario: Amplification attempt

- **WHEN** a client sends many small STUN requests with a spoofed source address
- **THEN** each response is at most 2.2 times its request and at most 44 bytes, and once the address's budget (default 60 per minute) is spent, no further answers are sent

#### Scenario: Other STUN traffic

- **WHEN** the server receives anything other than a well-formed Binding request
- **THEN** it sends no reply

### Requirement: Relay admission

A relay in Required mode (the LAN relays) SHALL send each new connection a `challenge` with a fresh single-use nonce and take a role slot only after a valid `admit` for that connection. It SHALL hold only the admission key, never the PSK, and a room without a registered key SHALL admit nobody. Unauthenticated sockets SHALL be closed after 5 seconds. The public server runs in Open mode, without a challenge.

#### Scenario: Challenge and admit

- **WHEN** a client connects to a relay in Required mode
- **THEN** the relay sends `{"type":"challenge","nonce":<22 base64url characters>}`, and the client joins with `{"type":"admit","role","roomId","protocolVersion","mac"}`, where `mac` is 64 lowercase hex characters

#### Scenario: Exact MAC input

- **WHEN** a client computes `mac`
- **THEN** it is the lowercase hex of `HMAC-SHA256(key, "gs-relay-admit/1|" + nonce + "|" + role + "|" + roomId)`, where `key` is `HKDF-SHA256(PSK, empty salt, info "guardedsleep relay admission v1")` (32 bytes) and `role` is the joining role, `baby` or `parent`, which is the direction

#### Scenario: Wrong answer

- **WHEN** an `admit` fails verification, or names a room without a registered key
- **THEN** the relay answers `ADMISSION_FAILED` and closes the socket, and the nonce is spent, so each connection has one attempt

#### Scenario: Replayed first message

- **WHEN** an attacker on the LAN replays a captured admission answer
- **THEN** it fails against the new nonce and the parent slot stays free

#### Scenario: Unauthenticated socket

- **WHEN** a connection has not been admitted 5 seconds after connecting
- **THEN** the relay closes it

#### Scenario: Open mode on the public server

- **WHEN** a client joins the public signaling server
- **THEN** it is admitted with `hello` and no challenge, `admit` is refused with `INVALID_MESSAGE`, and trust rests on the end-to-end pairing proof, the DTLS fingerprint binding and the unguessable derived room ids
