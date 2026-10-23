# Spec Delta

## MODIFIED Requirements

### Requirement: Supervision lives only in the core Link

The Rust core `Link` SHALL own the signaling state of every mailbox, attempts, liveness, escalation, listen intent, snooze, peer status and the missed-event buffer. Platforms SHALL talk only to the `Link` and SHALL NOT run their own reconnect loops, mailbox failover or timer-based alarms; they open, close and feed the mailboxes the `Link` asks for. A `Link` SHALL NOT be constructed without a PSK.

#### Scenario: Link without a PSK

- **WHEN** a platform constructs a `Link` with an empty or malformed PSK
- **THEN** construction fails with a configuration error and no attempt is ever created

#### Scenario: Platform loses the network

- **WHEN** the platform observes a network change
- **THEN** it only reports `network_available` to the `Link` and performs whatever actions the `Link` returns

#### Scenario: Wi-Fi to mobile data

- **WHEN** the parent device switches from Wi-Fi to mobile data during monitoring
- **THEN** the `Link` reopens the internet mailboxes, starts one fresh attempt through them and escalates by the normal schedule if it does not verify
