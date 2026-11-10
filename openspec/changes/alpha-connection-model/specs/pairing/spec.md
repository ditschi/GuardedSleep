# Spec Delta

## MODIFIED Requirements

### Requirement: Invite shown by the baby unit

The baby unit SHALL show the invite as a QR code (inverted, at most 50 % brightness) and as copyable text. The parent unit SHALL accept the invite by scanning it, with the camera permission requested only at that moment, or by pasting the text. The invite SHALL be valid for 5 minutes with a visible countdown and a "Show new code" action, and each new code SHALL carry a fresh PSK.

#### Scenario: Code expires

- **WHEN** the parent scans a code whose expiry has passed
- **THEN** pairing is rejected as expired and the parent is told to show a new code on the baby unit and to check the date and time of both devices

#### Scenario: Parent clock ahead at the first pairing

- **WHEN** the parent's wall clock is 10 minutes ahead of the baby's and it scans a fresh invite with a 5-minute expiry
- **THEN** the parent refuses the code as expired, stores nothing, and suggests checking the date and time

### Requirement: Invite payload v4

The invite SHALL be UTF-8 JSON `{"v":4,"k":<PSK>,"m":[<mailbox>…],"x":<expiry>}` of at most 300 bytes: the PSK as unpadded base64url of 32 random bytes, one to six mailbox strings (`r:<host[:port][/path]>` for a relay reached over `wss://`, `l:<address>:<port>` for a LAN mailbox with an RFC 1918 or RFC 4193 address), and the expiry in epoch seconds. It SHALL NOT carry a room id, tag, fingerprint, parent id, device name or any public IP address.

#### Scenario: Invite built by the baby device

- **WHEN** the baby device builds an invite for one relay mailbox
- **THEN** the text has exactly the fields `v`, `k`, `m` and `x`, is at most 300 bytes, and parses back to the same PSK, mailbox and expiry

#### Scenario: Default invite with internet

- **WHEN** a baby device with default settings and internet shows its invite
- **THEN** the invite carries the project server as its relay mailbox and its QR code decodes when shown inverted at 50 % brightness

### Requirement: Invite size limit

When the mailboxes would exceed the size limit, the baby device SHALL drop own relays from the last one backwards, never a LAN mailbox, the project server or the first own relay, and SHALL fail the invite when the protected entries alone are too big.

#### Scenario: Too many relays

- **WHEN** the baby device builds an invite with more relay mailboxes than fit into six entries or 300 bytes
- **THEN** own relays are dropped from the last one backwards, and every LAN mailbox, the project server and the first own relay stay in the invite

## ADDED Requirements

### Requirement: Parent device shown on the baby unit

After pinning, the baby unit SHALL show the parent device's name with a "Not my device" action that forgets the pairing, and SHALL show the security warning when it refuses a parent.

#### Scenario: Someone else paired first

- **WHEN** the baby unit shows "Connected to" a device name the user does not recognise
- **THEN** "Not my device" forgets the pairing and a new invite can be shown

#### Scenario: Refused parent

- **WHEN** someone with the PSK but without the parent secret tries to connect to a pinned baby unit
- **THEN** the baby unit shows a security warning and never opens its microphone path for them
