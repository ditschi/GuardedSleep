# Spec Delta

## MODIFIED Requirements

### Requirement: Client ICE configuration

The client SHALL use the STUN service of its configured relay by default. Public STUN servers SHALL be used only after the user opts in, with copy that names who sees the device's IP address. Release builds SHALL ship without a default signaling URL; the user enters the signaling URL list on the baby unit, and the invite carries it to the parent. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Fresh release install

- **WHEN** a release build without a configured signaling URL starts pairing on the baby unit
- **THEN** pairing fails as unreachable until a signaling URL is entered in the settings

#### Scenario: Public STUN not chosen

- **WHEN** the user has not opted in to public STUN
- **THEN** no request goes to a public STUN server

## ADDED Requirements

### Requirement: Built-in STUN

The signaling server SHALL answer STUN Binding requests only, with a response no larger than the request and a per-IP rate limit, on UDP port 3478.

#### Scenario: Amplification attempt

- **WHEN** a client sends a small STUN request with a spoofed source address
- **THEN** the response is no larger than the request

### Requirement: Relay admission

A LAN relay SHALL send each joiner a fresh nonce and assign a role slot only after the answer `HMAC(HKDF(PSK, "guardedsleep relay admission v1"), nonce ‖ direction ‖ room id)` verifies. Sockets that do not authenticate within 5 seconds SHALL be closed. The relay SHALL hold only the admission key, never the PSK.

#### Scenario: Replayed first message

- **WHEN** an attacker on the LAN replays a captured admission answer
- **THEN** it fails against the new nonce and the parent slot stays free
