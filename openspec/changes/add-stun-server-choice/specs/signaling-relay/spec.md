# Spec Delta

## ADDED Requirements

### Requirement: Third-party STUN choice

The client SHALL offer, in its advanced settings, a choice of STUN service: the configured relay (default), a server from a vetted list of privacy-respecting European STUN servers, or a manually entered `host:port`. Selecting a third-party server SHALL first show a warning that names the operator and states that it sees the device's IP address and when the app connects. Google STUN servers SHALL NOT appear in the list, in defaults or in code. The client SHALL never switch to a third-party server without the user's choice.

#### Scenario: Choosing a listed server

- **WHEN** the user selects a server from the vetted list
- **THEN** the warning names the operator, and the server is used only after the user confirms

#### Scenario: Relay STUN does not answer

- **WHEN** the relay's STUN service does not answer
- **THEN** the app explains the situation and offers the setting, without switching servers on its own

#### Scenario: Google STUN in code

- **WHEN** a change adds a `stun.l.google.com` or other Google STUN host
- **THEN** lint fails
