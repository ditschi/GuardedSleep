# Spec Delta

## MODIFIED Requirements

### Requirement: Client ICE configuration

The Android and web clients SHALL use the STUN service of their configured relay by default and SHALL NOT contact any public STUN server by default. A third-party STUN server SHALL be used only after the user opts in, with copy that names who sees the device's IP address. A VPN SHALL get no special handling: media MAY use VPN candidates, and signaling SHALL always go through the configured relay.

#### Scenario: Public STUN not chosen

- **WHEN** the user has not opted in to a third-party STUN server
- **THEN** no request goes to any STUN server other than the configured relay
