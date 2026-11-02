# Spec Delta

## Purpose

Carry the signaling between a paired baby device and parent device over several rendezvous channels at once (the project signaling server, own relays and, with `offline-lan`, a LAN listener), so that pairing and recovery need no manual address, while every operator learns only connection metadata and can never mute escalation or take over a pairing.

## ADDED Requirements

### Requirement: Parallel mailboxes

The core SHALL keep a set of mailboxes for a pairing (the ones named in the invite or a later mailbox update, plus configured own relays) and SHALL send every signaling envelope to all open mailboxes. Losing one mailbox SHALL NOT close an attempt or the link while another mailbox reaches the peer.

#### Scenario: One relay goes down during setup

- **WHEN** a pairing uses the project server and an own relay, and one of them closes while an attempt is being set up
- **THEN** the offer and answer still arrive through the other one and the attempt verifies within the 20 s attempt limit

#### Scenario: A failing relay mailbox does not stall the setup

- **WHEN** the own relay mailbox opens and closes repeatedly while the project server reaches the peer
- **THEN** the parent's outstanding `request_offer` stays valid, the baby answers it and the attempt verifies in seconds, not after the attempt timeout

### Requirement: De-duplication after authentication

The core SHALL process the first copy of an envelope that authenticates and passes the session checks, from any mailbox, and drop later copies with the same envelope id. An envelope id SHALL be recorded only after those checks.

#### Scenario: Corrupted copy arrives first

- **WHEN** one mailbox forwards a corrupted copy of an envelope before another mailbox delivers the genuine one
- **THEN** the corrupted copy is dropped and the genuine copy is processed

#### Scenario: Duplicate delivery

- **WHEN** the same envelope arrives through two mailboxes
- **THEN** the core processes it once

### Requirement: Mailboxes closed while connected

While an attempt is verified, the core SHALL close all internet mailboxes and SHALL open them briefly every 30 minutes to check reachability without publishing anything and without sending any value derived from the PSK. If no mailbox is reachable it SHALL emit the hint `recovery_path_unavailable`, which the UI shows calmly and never as an alarm. On loss of the verified attempt the core SHALL reopen every mailbox at once.

#### Scenario: A whole night of stable monitoring

- **WHEN** a pairing stays verified for eight hours
- **THEN** each device has sent signaling to the project server only for the first attempt, and holds no internet mailbox socket between reachability checks

#### Scenario: The project server disappears during the night

- **WHEN** no mailbox is reachable at a reachability check while monitoring is verified
- **THEN** the parent shows the calm hint that reconnecting would not work right now, and monitoring continues

#### Scenario: Loss after the mailboxes were closed

- **WHEN** the verified attempt is lost
- **THEN** the core reopens the mailboxes immediately and recovery follows the normal attempt schedule and escalation

### Requirement: Sealed signaling envelope

Every signaling message on every mailbox SHALL be sealed with XChaCha20-Poly1305 under a key used only for the envelope and derived from the PSK, with a nonce from a core CSPRNG seeded by the platform; the nonce SHALL be the envelope id. Sender role and message class SHALL be authenticated, and the plaintext SHALL be padded to a size bucket per message class. The pairing proof and the DTLS fingerprint binding SHALL remain required.

#### Scenario: Operator reads the traffic

- **WHEN** a signaling relay operator stores and inspects every signaling message of a session
- **THEN** it finds no SDP, ICE candidate, fingerprint, LAN address, parent id or device name, and all messages of one class have the same length

### Requirement: Session confirmation

Each device SHALL draw a fresh session nonce for every `Link` run and announce it in `here`. A session SHALL become the peer's current session only through a `here` that echoes a fresh, single-use challenge the receiver issued for it within the last 60 seconds; until then it is a candidate, answered with at most one `here` carrying that challenge, and messages from it are dropped.

#### Scenario: Old presence replayed

- **WHEN** a mailbox replays a `here` from an earlier session of the peer
- **THEN** the receiver sends at most one reply and starts no attempt, because the old session never confirms

#### Scenario: Delayed presence from a superseded session

- **WHEN** a `here` from the peer's previous session, naming the receiver's current session, arrives after the peer's newer session was confirmed
- **THEN** the newer session stays current and keeps working

### Requirement: Session-bound freshness

Every message other than `here` SHALL carry the recipient's current session and the sender's session and sequence number inside the ciphertext; a receiver SHALL drop messages whose recipient session is not its own current session, whose sender session is not the peer's current confirmed session, or whose sequence number or envelope id was already seen in this run. A processed stop SHALL retire the sender's session for good.

#### Scenario: Stop replayed after a parent restart

- **WHEN** a relay serves the baby's earlier stop notice to a parent that has just restarted
- **THEN** the parent ignores it and escalation continues as normal

#### Scenario: Stored baby stop after the baby restarted

- **WHEN** the baby stopped on purpose, started again with a new session, and later loses the link while a relay injects the old stop to the unchanged parent
- **THEN** the parent ignores it because the stop names a retired baby session, and escalation continues as normal

#### Scenario: Stored parent stop after the parent restarted

- **WHEN** the parent stopped on purpose, started again, and a relay later injects the old parent stop to the unchanged baby
- **THEN** the baby ignores it

### Requirement: Replay state per run

The replay state (envelope ids and sequence numbers) SHALL cover the whole `Link` run across all mailboxes and SHALL NOT be cleared when a mailbox reopens.

#### Scenario: Replayed envelope

- **WHEN** an attacker resends a captured envelope within the same run
- **THEN** the receiver drops it without changing state, also after the mailbox reopened

#### Scenario: Reflected envelope

- **WHEN** an attacker sends a parent's envelope back to the parent
- **THEN** it is rejected because it carries the receiver's own role

### Requirement: Relay-visible data

A relay (the project server or an own relay) SHALL receive only its protocol fields (derived room id, role, protocol version, client ids, error codes, ping) and sealed envelopes (header with version, role and class; nonce; ciphertext). A TURN server SHALL receive only the TURN user name, which never contains the room id or a value derived from the PSK, the permitted peer addresses (including private host-candidate addresses), and the addresses, timing and volume of the packets it relays.

#### Scenario: No secret leaves through a mailbox

- **WHEN** a device's mailbox traffic is searched for the PSK, every derived key, the seed, the parent secret and plaintext SDP in any encoding
- **THEN** none of them is found

#### Scenario: Captured relay traffic

- **WHEN** the bytes a device sends to its relays during pairing, recovery and reachability checks are captured
- **THEN** every field is on the allowed list, and no reachability check contains the room id or an envelope

### Requirement: Delivery is acknowledged by the peer

A mailbox accepting a message SHALL NOT count as delivery. An offer, answer or `request_offer` without a reply within 6 seconds SHALL be resent once, with the same envelope id, to every open mailbox; the attempt SHALL still be abandoned after 20 seconds.

#### Scenario: Relay accepts but never delivers

- **WHEN** one relay of the pairing accepts the parent's messages but never delivers them and a second relay works
- **THEN** the attempt verifies through the second relay and the parent shows no error

### Requirement: Mailbox update

After verification the baby device MAY send its current mailbox set over the DataChannel, and the parent device SHALL replace its stored set with it, after validating it with the invite grammar.

#### Scenario: The project server moves

- **WHEN** a release moves the project server to another host and the baby device updates first
- **THEN** the baby sends the new set after the next verification, and later recoveries use it without re-pairing

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
