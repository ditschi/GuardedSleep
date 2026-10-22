# Spec Delta

## MODIFIED Requirements

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

### Requirement: Client ICE configuration

The Android client SHALL use the STUN service of its configured relay by default and SHALL NOT contact any public STUN server by default. A third-party STUN server SHALL be used only after the user opts in, with copy that names who sees the device's IP address (the choice among vetted servers is `add-stun-server-choice`). The web client follows in `rebuild-web-client`. Release builds SHALL ship without a default signaling URL; the user enters the signaling URL list on the baby unit, and the invite carries it to the parent. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Fresh release install

- **WHEN** a release build without a configured signaling URL starts pairing on the baby unit
- **THEN** pairing fails as unreachable until a signaling URL is entered in the settings

#### Scenario: Public STUN not chosen

- **WHEN** the user has not opted in to a third-party STUN server
- **THEN** no request goes to any STUN server other than the configured relay

### Requirement: Resource and rate limits

The relay SHALL reject messages larger than 64 KiB and answer unparsable messages with `BAD_JSON`. Per client IP it SHALL limit join attempts (default 20 per minute), new rooms (10 per minute) and open sockets (16); per socket it SHALL limit frames (120 per minute before joining, 600 after). Exceeding a limit SHALL close the socket. It SHALL take the client IP from `X-Forwarded-For` only for a configured number of trusted proxy hops.

#### Scenario: Room stuffing

- **WHEN** one client IP tries to create an eleventh room within a minute
- **THEN** the relay answers `RATE_LIMITED` and closes the socket

#### Scenario: Socket hoarding

- **WHEN** one client IP opens a seventeenth socket while sixteen are open
- **THEN** the relay closes the new socket as `server full`, and sockets from other client IPs are unaffected

#### Scenario: Frame flood

- **WHEN** a socket sends more than 120 frames in a minute without joining, or more than 600 once joined
- **THEN** the relay answers `RATE_LIMITED` and closes the socket

#### Scenario: Behind a reverse proxy

- **WHEN** the server runs behind one trusted proxy with the trust setting on
- **THEN** the limits count the address the proxy appended as the last `X-Forwarded-For` hop, and never log it

## ADDED Requirements

### Requirement: Built-in STUN

The signaling server SHALL answer STUN Binding requests only, on UDP port 3478, enabled by default; an operator MAY disable it or move its port. A response SHALL be at most 44 bytes and at most 2.2 times the size of its request, and each source address SHALL be limited by a per-IP rate limit (default 60 per minute), so the server is useless as a traffic amplifier while bare 20-byte Binding requests from browsers and libwebrtc are answered.

#### Scenario: Browser Binding request

- **WHEN** a browser sends a 20-byte Binding request without attributes
- **THEN** the server answers with its XOR-MAPPED-ADDRESS

#### Scenario: Amplification attempt

- **WHEN** a client sends many small STUN requests with a spoofed source address
- **THEN** each response is at most 2.2 times its request and at most 44 bytes, and the per-IP rate limit stops answering after a few requests

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
