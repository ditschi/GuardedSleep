# Spec Delta

## MODIFIED Requirements

### Requirement: Client ICE configuration

The Android client SHALL use the STUN service of its configured relay by default and SHALL NOT contact any public STUN server by default. A third-party STUN server SHALL be used only after the user opts in, with copy that names who sees the device's IP address (the choice among vetted servers is `add-stun-server-choice`). The web client follows in `rebuild-web-client`. Release builds SHALL ship without a default signaling URL; the user enters the signaling URL list on the baby unit, and the invite carries it to the parent. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Fresh release install

- **WHEN** a release build without a configured signaling URL starts pairing on the baby unit
- **THEN** pairing fails as unreachable until a signaling URL is entered in the settings

#### Scenario: Public STUN not chosen

- **WHEN** the user has not opted in to a third-party STUN server
- **THEN** no request goes to any STUN server other than the configured relay

## ADDED Requirements

### Requirement: Built-in STUN

The signaling server SHALL answer STUN Binding requests only, on UDP port 3478, enabled by default. A response SHALL be at most 44 bytes and at most 2.2 times the size of its request, and each source address SHALL be limited by a per-IP rate limit, so the server is useless as a traffic amplifier while bare 20-byte Binding requests from browsers and libwebrtc are answered.

#### Scenario: Browser Binding request

- **WHEN** a browser sends a 20-byte Binding request without attributes
- **THEN** the server answers with its XOR-MAPPED-ADDRESS

#### Scenario: Amplification attempt

- **WHEN** a client sends many small STUN requests with a spoofed source address
- **THEN** each response is at most 2.2 times its request and at most 44 bytes, and the per-IP rate limit stops answering after a few requests

### Requirement: Relay admission

A LAN relay SHALL send each joiner a fresh nonce and assign a role slot only after the answer `HMAC(HKDF(PSK, "guardedsleep relay admission v1"), nonce ‖ direction ‖ room id)` verifies. Sockets that do not authenticate within 5 seconds SHALL be closed. The relay SHALL hold only the admission key, never the PSK.

#### Scenario: Replayed first message

- **WHEN** an attacker on the LAN replays a captured admission answer
- **THEN** it fails against the new nonce and the parent slot stays free
