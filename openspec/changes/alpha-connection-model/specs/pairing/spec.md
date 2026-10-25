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

A pairing SHALL connect exactly one baby unit and one parent unit. At the first verification the parent unit SHALL send a random 32-byte parent secret and its device name only over the verified DTLS DataChannel, never through a mailbox, and the baby unit SHALL pin that secret and close the invite. On every later attempt the baby unit SHALL open its microphone path for the parent only after the parent proves the secret, bound to both DTLS fingerprints, over the DataChannel, and SHALL refuse every other parent until the pairing is forgotten, raising a security warning. The parent unit SHALL count a pinned pairing's attempt as connected only after the baby unit proves the same secret, bound to both fingerprints, over the DataChannel. The platform SHALL durably persist the parent secret before executing any later action that could send it, and the secret SHALL be sent only while the invite is valid. Once pinned, both units SHALL refuse envelopes from the peer role that do not carry a valid pin tag over the whole envelope. After pinning, the baby unit SHALL show the parent device's name with a "Not my device" action that forgets the pairing. On our relay a second parent SHALL still be refused with `PARENT_CAP`.

#### Scenario: Second parent in the room

- **WHEN** a second parent unit joins a room that already has a parent
- **THEN** the relay refuses it with `PARENT_CAP` and the existing pairing is unaffected

#### Scenario: Attacker holds the PSK and every relayed envelope

- **WHEN** someone who photographed the invite and recorded every envelope ever relayed tries to connect to the baby unit while the real parent unit is offline
- **THEN** the baby unit never opens its microphone path for them, shows a security warning, and the real parent unit still escalates the outage

#### Scenario: Man in the middle after pairing

- **WHEN** a PSK holder relays between two separate DTLS sessions to the baby unit and the parent unit
- **THEN** the parent proof fails because the fingerprints differ, and the baby unit refuses the attempt

#### Scenario: Man in the middle keeps cycling the parent

- **WHEN** a PSK holder repeatedly gets the parent unit's DTLS session verified during an outage
- **THEN** the parent unit never shows connected without a valid baby proof, and escalation reaches level 2 on schedule

#### Scenario: Acknowledgement lost before a parent restart

- **WHEN** the baby unit's acknowledgement of the parent secret is lost and the parent unit restarts
- **THEN** the parent unit still holds the persisted secret and reconnects without forgetting the pairing

#### Scenario: Someone else paired first

- **WHEN** the baby unit shows "Connected to" a device name the user does not recognise
- **THEN** "Not my device" forgets the pairing and a new invite can be shown

#### Scenario: PSK holder poses as the baby during an outage

- **WHEN** someone with the PSK sends presence and a stop notice to a pinned parent unit while the real baby unit is offline
- **THEN** the parent unit refuses them, escalation reaches level 2 on schedule, and the real baby unit reconnects afterwards

#### Scenario: Secret never reached the baby

- **WHEN** the DataChannel fails right after the first verification and the parent unit restarts after the invite expired
- **THEN** both units show that they need to be paired again, and neither sends the secret again

#### Scenario: Forget on the baby device

- **WHEN** the user forgets the pairing on the baby device
- **THEN** the pinned parent secret is deleted and a new invite can pair a new parent device

### Requirement: Encrypted storage of pairing material

On Android the PSK, the derived room id, the mailbox set and the parent secret SHALL be stored encrypted with an AES-GCM key held in the Android Keystore (StrongBox when available). Pairing material SHALL NOT appear in logs, Intents, Bundles, URLs or saved UI state, and app backup SHALL be disabled.

#### Scenario: Pasted code in the UI

- **WHEN** the parent pastes an invite into the text field and the activity is recreated
- **THEN** the pasted text is not restored from saved instance state

#### Scenario: Mailbox update stored

- **WHEN** the parent receives a mailbox update from the baby device
- **THEN** the new set is stored in the same encrypted record and never in plain preferences
