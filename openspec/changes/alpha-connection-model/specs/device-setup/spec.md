# Spec Delta

## MODIFIED Requirements

### Requirement: Settings in Basic and Advanced

Basic settings SHALL contain the noise threshold and hold time, alert output and gain, theme (dark by default), language, on the baby device Quiet mode while monitoring, and a Connection section with the one-sentence disclosure of the project server and an expandable operator line with its privacy policy and imprint. Settings that can break monitoring SHALL live only in Advanced: "Only same Wi-Fi or my own server", own signaling relay URLs (validated as `wss://` URLs with a host), an optional own TURN server and the heartbeat interval (5–15 s). A local diagnostics view SHALL show the connection type of the last attempt (direct on the LAN, direct over the internet, or relayed through TURN) and SHALL never send it anywhere.

#### Scenario: Invalid server address

- **WHEN** the user enters an own relay URL that is not a `wss://` URL with a host
- **THEN** the URL is rejected with an error and not saved

#### Scenario: Checking why a connection is slow

- **WHEN** a parent opens the connection details after monitoring
- **THEN** they see whether the last connection was direct on the LAN, direct over the internet or relayed, and nothing leaves the device

### Requirement: Baby device check

The baby unit SHALL check its microphone level, battery, the reachability of its mailboxes (at least one reachable, and which ones), permissions and Do Not Disturb access, and show the result as OK, limited or not recommended with plain explanations. The result SHALL NOT block monitoring; the user decides.

#### Scenario: Old phone with a weak battery

- **WHEN** the device check finds a low battery that does not charge
- **THEN** it shows "limited" or "not recommended" with the reason, and monitoring can still start

#### Scenario: No mailbox reachable

- **WHEN** the device check reaches none of the pairing's mailboxes
- **THEN** it shows "not recommended" with the reason that the devices could not find each other again after a disconnection
