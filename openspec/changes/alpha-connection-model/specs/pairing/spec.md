# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Invite payload v3`
- TO: `### Requirement: Invite payload v4`

## MODIFIED Requirements

### Requirement: Invite payload v4

The invite SHALL be UTF-8 JSON `{"v":4,"k":<PSK>,"m":[<mailbox>…],"x":<expiry>}` of at most 300 bytes: the PSK as unpadded base64url of 32 random bytes, one to six mailbox strings (`n:<host>` for a Nostr relay and `r:<host[:port][/path]>` for an own relay, both reached over `wss://`, and `l:<address>:<port>` for a LAN mailbox with an RFC 1918 or RFC 4193 address), and the expiry in epoch seconds. It SHALL NOT carry a room id, tag, fingerprint, parent id, device name or any public IP address. A payload with another version (including v3), a PSK of the wrong length, an invalid mailbox, more than six mailboxes or a missing expiry SHALL be rejected as not a GuardedSleep code. When the mailboxes would exceed the size limit, the baby device SHALL drop extra own relays first and then Nostr relays down to two, never a LAN mailbox or the first own relay.

#### Scenario: Foreign QR code

- **WHEN** the parent scans a QR code that is not a v4 GuardedSleep invite
- **THEN** pairing is rejected as "not our code" and nothing is stored

#### Scenario: Default invite with internet

- **WHEN** a baby device with default settings and internet shows its invite
- **THEN** the invite carries three Nostr relays from different operators and its QR code decodes when shown inverted at 50 % brightness

#### Scenario: Link-local address

- **WHEN** an invite names a LAN mailbox with an `fe80::` address
- **THEN** it is rejected as not a GuardedSleep code

### Requirement: One baby unit and one parent unit

A pairing SHALL connect exactly one baby unit and one parent unit. The parent unit SHALL draw a random parent id at pairing and send it only inside sealed envelopes and over the verified DataChannel. After its first verified attempt the baby unit SHALL pin that parent id, close the invite, and refuse requests, offers and answers carrying any other parent id until the pairing is forgotten. On our relay a second parent SHALL still be refused with `PARENT_CAP`.

#### Scenario: Second parent in the room

- **WHEN** a second parent unit joins a room that already has a parent
- **THEN** the relay refuses it with `PARENT_CAP` and the existing pairing is unaffected

#### Scenario: Photographed QR code during an outage

- **WHEN** someone who photographed the invite tries to connect to the baby device while the real parent device is offline
- **THEN** the baby device refuses every attempt from that device, and the real parent device still escalates the outage

#### Scenario: Forget on the baby device

- **WHEN** the user forgets the pairing on the baby device
- **THEN** the pinned parent id is deleted and a new invite can pair a new parent device

### Requirement: Encrypted storage of pairing material

On Android the PSK, the derived room id, the mailbox set and the parent id SHALL be stored encrypted with an AES-GCM key held in the Android Keystore (StrongBox when available). Pairing material SHALL NOT appear in logs, Intents, Bundles, URLs or saved UI state, and app backup SHALL be disabled.

#### Scenario: Pasted code in the UI

- **WHEN** the parent pastes an invite into the text field and the activity is recreated
- **THEN** the pasted text is not restored from saved instance state

#### Scenario: Mailbox update stored

- **WHEN** the parent receives a mailbox update from the baby device
- **THEN** the new set is stored in the same encrypted record and never in plain preferences
