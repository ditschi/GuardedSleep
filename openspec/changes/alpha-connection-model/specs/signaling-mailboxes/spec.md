# Spec Delta

## ADDED Requirements

### Requirement: Project signaling server

Release builds SHALL use one project-run signaling server as the default internet mailbox, named once in `data/project-server.json` with its operator, country, privacy policy and imprint. The baby device SHALL put it into its invite and mailbox updates unless "Only same Wi-Fi or my own server" is on. The project server SHALL be treated as an untrusted relay carrying only sealed envelopes. No public Nostr relay or other third-party mailbox SHALL be used.

#### Scenario: Default invite

- **WHEN** a release baby device with default settings shows its invite
- **THEN** the invite names the project server as a relay mailbox and no other internet service

#### Scenario: Data file without a privacy policy

- **WHEN** the project server entry lacks an `https://` privacy policy or imprint URL
- **THEN** the build fails and names the missing field

#### Scenario: Project server down

- **WHEN** the project server is unreachable while monitoring is verified
- **THEN** monitoring continues; if the link is later lost, the parent escalates as normal and the troubleshooter names `server_unreachable`

### Requirement: Disclosure and off switch

Setup and the Basic connection settings SHALL say in one sentence that the GuardedSleep server helps the devices find each other and may relay the encrypted sound, seeing network addresses and usage times but never the baby's sound, with an expandable operator line from the data file. An Advanced switch "Only same Wi-Fi or my own server" SHALL stop the device from contacting the project server, its STUN and its TURN, with a hint if the peer still announces the project server.

#### Scenario: Privacy-minded parent

- **WHEN** a parent turns on "Only same Wi-Fi or my own server" and has no own relay
- **THEN** the parent device contacts neither the project server nor any STUN or TURN server, and explains that monitoring then works only on the same Wi-Fi or with an own server
