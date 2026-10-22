# Spec Delta

## MODIFIED Requirements

### Requirement: Signaling transport security

Signaling to the internet SHALL use `wss://`. Release builds SHALL allow cleartext `ws://` only toward private LAN literal addresses (RFC 1918, RFC 4193 and link-local), checked in one place in the platform layer before every connection; 100.64/10 and host names SHALL be refused. Debug builds MAY use cleartext toward any address.

#### Scenario: Release build with a ws:// URL

- **WHEN** a release build is given a `ws://` signaling URL on the internet
- **THEN** the connection is refused by the platform

#### Scenario: Offline baby unit on the home Wi-Fi

- **WHEN** a release build connects to `ws://192.168.1.20:<port>`
- **THEN** the connection is allowed
