# Spec Delta

## MODIFIED Requirements

### Requirement: Settings in Basic and Advanced

Basic settings SHALL contain the noise threshold and hold time, alert output and gain, theme (dark by default), language and, on the baby device, Quiet mode while monitoring. A Connection section SHALL name the public relays and STUN operators in use and offer "Use only my own server". Settings that can break monitoring SHALL live only in Advanced: own signaling relay URLs (validated as `wss://` URLs, or `ws://` toward private LAN addresses), an optional TURN server and the heartbeat interval (5–15 s). A local diagnostics view SHALL show the connection type of the last attempt (direct on the LAN, direct over the internet, or relayed) and SHALL never send it anywhere.

#### Scenario: Invalid server address

- **WHEN** the user enters a signaling URL that is not a `wss://` URL with a host, or a `ws://` URL toward a public address
- **THEN** the URL is rejected with an error and not saved

#### Scenario: Checking why a connection is slow

- **WHEN** a parent opens the connection details after monitoring
- **THEN** they see whether the last connection was direct on the LAN, direct over the internet or relayed, and nothing leaves the device
