# Spec Delta

## Purpose

Carry the signaling between a paired baby device and parent device over several rendezvous channels at once (our relay, public Nostr relays and, with `offline-lan`, a LAN listener), so that pairing and recovery need no manual address and no single operator, while every operator learns only connection metadata and can never mute escalation or take over a pairing.

## ADDED Requirements

### Requirement: Parallel mailboxes

The core SHALL keep a set of mailboxes for a pairing (the ones named in the invite or a later mailbox update, plus configured own relays) and SHALL send every signaling envelope to all open mailboxes. Losing one mailbox SHALL NOT close an attempt or the link while another mailbox reaches the peer.

#### Scenario: One relay goes down during setup

- **WHEN** one of three mailboxes closes while an attempt is being set up
- **THEN** the offer and answer still arrive through the remaining mailboxes and the attempt verifies within the 20 s attempt limit

### Requirement: De-duplication after authentication

The core SHALL process the first copy of an envelope that authenticates and passes the session checks, from any mailbox, and drop later copies with the same envelope id. An envelope id SHALL be recorded only after those checks.

#### Scenario: Corrupted copy arrives first

- **WHEN** one relay forwards a corrupted copy of an envelope before the other relays deliver the genuine one
- **THEN** the corrupted copy is dropped and the genuine copy is processed

#### Scenario: Duplicate delivery

- **WHEN** the same envelope arrives through two mailboxes
- **THEN** the core processes it once

### Requirement: Mailboxes closed while connected

While an attempt is verified, the core SHALL close all internet mailboxes and SHALL open them briefly every 30 minutes to check reachability without publishing anything and without sending any value derived from the PSK. If no mailbox is reachable it SHALL emit the hint `recovery_path_unavailable`, which the UI shows calmly and never as an alarm. On loss of the verified attempt the core SHALL reopen every mailbox at once.

#### Scenario: A whole night of stable monitoring

- **WHEN** a pairing stays verified for eight hours
- **THEN** each device has published only the events of the first attempt to public relays, and holds no internet mailbox socket between reachability checks

#### Scenario: Every relay disappears during the night

- **WHEN** no mailbox is reachable at a reachability check while monitoring is verified
- **THEN** the parent shows the calm hint that reconnecting would not work right now, and monitoring continues

#### Scenario: Loss after the mailboxes were closed

- **WHEN** the verified attempt is lost
- **THEN** the core reopens the mailboxes immediately and recovery follows the normal attempt schedule and escalation

### Requirement: Sealed signaling envelope

Every signaling message on every mailbox SHALL be sealed with XChaCha20-Poly1305 under a key used only for the envelope and derived from the PSK, with a nonce from a core CSPRNG seeded by the platform; the nonce SHALL be the envelope id. Sender role and message class SHALL be authenticated, and the plaintext SHALL be padded to a size bucket per message class. The pairing proof and the DTLS fingerprint binding SHALL remain required.

#### Scenario: Operator reads the traffic

- **WHEN** a mailbox operator stores and inspects every message of a session
- **THEN** it finds no SDP, ICE candidate, fingerprint, LAN address, parent id or device name, and all messages of one class have the same length

### Requirement: Session confirmation

Each device SHALL draw a fresh session nonce for every `Link` run and announce it in `here`. A session SHALL become the peer's current session only through a `here` that echoes a fresh, single-use challenge the receiver issued for it within the last 60 seconds; until then it is a candidate, answered with at most one `here` carrying that challenge, and messages from it are dropped.

#### Scenario: Old presence replayed

- **WHEN** a relay replays a `here` from an earlier session of the peer
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

### Requirement: Rotating Nostr address

On public relays, every signaling event SHALL have the fixed ephemeral kind 24861, and the tag and signing keys SHALL be derived from the PSK and a daily epoch that changes at 12:00 UTC. Both devices SHALL subscribe to both epochs during the hour around the change and SHALL re-subscribe at the change. Subscriptions SHALL look back 15 minutes. The epoch SHALL be checked only for Nostr addressing, never on relay or LAN mailboxes, and a device without a known wall clock SHALL open no Nostr mailbox.

#### Scenario: Two days in a row

- **WHEN** a pairing monitors on two consecutive days
- **THEN** the tags and public keys used on the public relays differ between the days

#### Scenario: Clock five minutes off at the epoch change

- **WHEN** one device's clock is five minutes ahead around 12:00 UTC and the link is lost
- **THEN** the devices still exchange envelopes and the attempt verifies

#### Scenario: Clocks of the two devices disagree

- **WHEN** the two devices' wall clocks differ by more than ten minutes
- **THEN** both devices show that the internet services need a correct clock, and pairing and recovery through a relay mailbox still work

#### Scenario: Relay refuses an event as too old

- **WHEN** a relay refuses an event because of its creation time
- **THEN** the device shows the clock hint and does not park the relay

### Requirement: Relay-visible data

A public relay SHALL receive only the event kind, event id, creation time, daily public key, daily tag and signature, random subscription ids and filter fields, and the envelope header (version, role, class), nonce and ciphertext. Our relay SHALL receive only its protocol fields (derived room id, role, protocol version, client ids, error codes, ping) and sealed envelopes.

#### Scenario: No secret leaves through a mailbox

- **WHEN** a device's mailbox traffic is searched for the PSK, every derived key, the seed, the parent secret and plaintext SDP in any encoding
- **THEN** none of them is found

#### Scenario: Captured relay traffic

- **WHEN** the bytes a device sends to public relays and to our relay during pairing, recovery and reachability checks are captured
- **THEN** every field is on the allowed list, and no reachability check contains the tag or a public key of the pairing

### Requirement: Event budget on public relays

Public relays SHALL carry presence and attempt setup only, never heartbeats. A device SHALL send at most 2 events per second per relay, at most 6 events per attempt and side with ICE candidates bundled, and at most 120 events per hour per mailbox, after which that mailbox carries only presence until the hour ends. Over Nostr an attempt SHALL start only after a fresh `here` from the peer. Reaching a limit SHALL NOT stop escalation.

#### Scenario: One-hour outage

- **WHEN** the baby device is off for an hour while the parent monitors
- **THEN** the parent sends no more than 120 events per relay in that hour, keeps escalating, and recovers through presence when the baby returns

### Requirement: Relay parking

A relay that refuses events (blocked, restricted, authentication or proof of work required, rate limited) or delivers forged or out-of-scope events SHALL be left out with a backoff of 1, 5 and 30 minutes and then retried.

#### Scenario: Rate-limited relay

- **WHEN** a relay answers an event with a rate-limit notice
- **THEN** the device stops using it for one minute, carries on with the other relays, and retries it afterwards

### Requirement: Replay-proof inbound limits

Repeated or re-delivered events SHALL be dropped by event id before counting or verifying, and an excess of distinct, verified in-scope events SHALL be dropped for the rest of the minute without parking the relay, so a third party replaying captured events cannot isolate a pairing.

#### Scenario: Third party replays captured events

- **WHEN** someone without the PSK republishes a pairing's captured events at 200 per minute on one of its relays
- **THEN** the relay is not parked and pairing and recovery through it still work

#### Scenario: Malicious relay floods forged events

- **WHEN** a relay delivers hundreds of events with invalid signatures in a minute
- **THEN** the device parks the relay and its monitoring is not disturbed

### Requirement: Delivery is acknowledged by the peer

A relay's acceptance of an event SHALL NOT count as delivery. An offer, answer or `request_offer` without a reply within 6 seconds SHALL be resent once, with the same envelope id, to every open mailbox; the attempt SHALL still be abandoned after 20 seconds.

#### Scenario: Relay accepts but never delivers

- **WHEN** a relay acknowledges the parent's events but never delivers them and a second relay works
- **THEN** the attempt verifies through the second relay and the parent shows no error

### Requirement: Mailbox update

After verification the baby device MAY send its current mailbox set over the DataChannel, and the parent device SHALL replace its stored set with it. Each device SHALL additionally use up to two relays from its own shipped list, chosen deterministically from the PSK, and SHALL use a received relay host even if its own list does not contain it.

#### Scenario: A relay of the invite shuts down

- **WHEN** a relay named at pairing time stops working and the baby's app update ships a new list
- **THEN** the baby sends the new set after the next verification, and later recoveries use it without re-pairing

### Requirement: Curated public services

Public relays and STUN servers SHALL come only from curated lists shipped with the app. Each entry SHALL name the operator, the country, the privacy policy where one exists, and the date it was last vetted. Entries SHALL be operated in the EU, EEA or Switzerland. A build SHALL fail with fewer than 6 usable relays from at least 4 operators or fewer than 2 STUN servers from distinct operators. The baby device SHALL pick relays from different operators.

#### Scenario: List too small

- **WHEN** the relay list has usable entries from only three operators
- **THEN** the build fails and names the shortfall

#### Scenario: Two relays of one operator

- **WHEN** two list entries share an operator
- **THEN** a pairing never gets both of them

### Requirement: Disclosure and off switch

Setup and the Basic connection settings SHALL say in one sentence that independent services in Europe help the devices find each other, seeing the internet address and when the device is in use but never the baby's sound, with an expandable list of operators from the data files. An Advanced switch "Only same Wi-Fi or my own server" SHALL stop the device from contacting any public relay or STUN server; if the peer still announces public mailboxes the device SHALL show a hint.

#### Scenario: Privacy-minded parent

- **WHEN** a parent turns on "Only same Wi-Fi or my own server" and has no own relay
- **THEN** the parent device contacts no public relay or STUN server, and explains that monitoring then works only on the same Wi-Fi or with an own server
