# Spec Delta

## Purpose

Carry the signaling between a paired baby device and parent device over several rendezvous channels at once (our relay, a LAN listener or local PC relay, and public Nostr relays), so that pairing and recovery work without a manual address, without internet on one network, and across networks, while every channel operator learns only connection metadata.

## ADDED Requirements

### Requirement: Parallel mailboxes

The core SHALL keep a set of mailboxes for a pairing: the ones named in the invite and any user-configured own relay. It SHALL send every signaling envelope to all open mailboxes and SHALL accept the first copy of an envelope from any mailbox, dropping later copies with the same envelope id. Losing one mailbox SHALL NOT close an attempt or the link while another mailbox reaches the peer.

#### Scenario: One relay goes down during setup

- **WHEN** one of three mailboxes closes while an attempt is being set up
- **THEN** the offer and answer still arrive through the remaining mailboxes and the attempt verifies

#### Scenario: Duplicate delivery

- **WHEN** the same envelope arrives through two mailboxes
- **THEN** the core processes it once

#### Scenario: All mailboxes unreachable

- **WHEN** no mailbox reaches the peer while the link is down
- **THEN** link supervision escalates to the parent exactly as for a relay outage, and the cause names the connection to the servers

### Requirement: Sealed signaling envelope

Every signaling message (presence, offer, answer, candidates, stop) on every mailbox SHALL be sealed with an authenticated cipher under a key derived from the PSK, with the sender role, the epoch and the attempt id authenticated as associated data and the plaintext padded to a fixed size bucket. The pairing proof and the DTLS fingerprint binding SHALL remain required.

#### Scenario: Operator reads the traffic

- **WHEN** a mailbox operator stores and inspects every message of a session
- **THEN** it finds no SDP, ICE candidate, fingerprint, LAN address or device name, and all messages of a session have the same length

### Requirement: Envelope rejection

A receiver SHALL reject envelopes that fail authentication, carry its own role, lie outside the current epoch window, or repeat an envelope id it has already seen, without changing state.

#### Scenario: Replayed envelope

- **WHEN** an attacker resends a captured envelope
- **THEN** the receiver drops it without changing state

#### Scenario: Reflected envelope

- **WHEN** an attacker sends a parent's envelope back to the parent
- **THEN** it is rejected because it carries the receiver's own role

### Requirement: Rotating mailbox address

On public relays the address used to find a pairing's messages (tag and signing keys) SHALL be derived from the PSK and the UTC day, and SHALL change every day. Both devices SHALL also accept the neighbouring day for 15 minutes around midnight. Nothing derived from the PSK other than these daily values SHALL be sent to a public relay.

#### Scenario: Two nights in a row

- **WHEN** a pairing monitors on two consecutive nights
- **THEN** the tags and public keys used on the public relays differ between the nights

#### Scenario: Clock skew at midnight

- **WHEN** one device's clock is five minutes ahead at midnight
- **THEN** the devices still exchange envelopes

### Requirement: Event budget on public relays

Public relays SHALL carry presence and attempt setup only. A device SHALL send at most six events per attempt and side, SHALL bundle ICE candidates, SHALL announce presence only at start, after a network change and with backoff while waiting for the peer, and SHALL NOT send heartbeats through public relays. A relay that refuses events (blocked, restricted, authentication or proof-of-work required, rate limited) SHALL be left out for the rest of the session.

#### Scenario: A whole night of monitoring

- **WHEN** a pairing monitors for eight hours without a reconnect
- **THEN** each device has sent only its start-up events to the public relays

#### Scenario: Rate-limited relay

- **WHEN** a relay answers an event with a rate-limit notice
- **THEN** the device stops using that relay for the session and carries on with the others

### Requirement: Delivery is acknowledged by the peer

A relay's acceptance of an event SHALL NOT count as delivery. An offer or presence announcement that gets no answer from the peer within the core's timeout SHALL be resent through the remaining mailboxes as part of the core's recovery.

#### Scenario: Relay accepts but never delivers

- **WHEN** a relay acknowledges the parent's events but never delivers them
- **THEN** the attempt is set up through the other mailboxes, and the parent shows no error while one mailbox works

### Requirement: Curated public services

Public relays and STUN servers SHALL come only from curated lists shipped with the app. Each entry SHALL name the operator, the country, the privacy policy where one exists, and the date it was last vetted. Entries SHALL be operated in the EU, EEA or Switzerland, and entries older than twelve months SHALL be dropped. The baby device SHALL pick three relays from different operators for a pairing.

#### Scenario: Stale entry

- **WHEN** a list entry was last vetted more than twelve months ago
- **THEN** it is not shipped in the build

#### Scenario: Two relays of one operator

- **WHEN** two list entries share an operator
- **THEN** a pairing never gets both of them

### Requirement: Disclosure and own-server-only

The setup screen and the connection settings SHALL name the operators of the public relays and STUN servers in use, saying that they see the device's internet address and when it connects and never hear the baby. A switch "Use only my own server" SHALL turn off all public relays and public STUN, after which only configured own relays and LAN mailboxes are used.

#### Scenario: Privacy-minded parent

- **WHEN** a parent turns on "Use only my own server" and has no own relay configured
- **THEN** no public relay or STUN server is contacted, and setup explains that pairing then works only on the same Wi-Fi or with an own server
