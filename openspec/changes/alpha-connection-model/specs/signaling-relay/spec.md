# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Signaling URL configuration`
- TO: `### Requirement: Default signaling configuration`

## MODIFIED Requirements

### Requirement: Default signaling configuration

Android release builds and the official web deployment SHALL work without any configured signaling URL: the baby unit SHALL fill the invite with the project signaling server (`signaling-mailboxes` "Project signaling server"), unless "Only same Wi-Fi or my own server" is on. Other web deployments SHALL default to their own origin's `/signaling`. A user MAY add own relay URLs in Advanced; they are added to the invite as own-relay mailboxes. The relay SHALL treat every `signal` payload as an opaque sealed envelope.

#### Scenario: Fresh release install

- **WHEN** a release build with no configured signaling URL starts pairing on the baby unit with internet (in CI: against the local server of the test stack)
- **THEN** pairing completes through the project server without any manual input

#### Scenario: Self-hosted web deployment

- **WHEN** a web build without the official-deployment flag starts pairing on the baby unit
- **THEN** the invite names its own origin's `/signaling` and never the project server

#### Scenario: Own server only, nothing configured

- **WHEN** "Only same Wi-Fi or my own server" is on and no own relay is configured
- **THEN** the baby unit explains that pairing needs the same Wi-Fi or an own server, and no internet service is contacted
