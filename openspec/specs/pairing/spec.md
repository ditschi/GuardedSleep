# pairing Specification

## Purpose

Establish trust between exactly one baby unit and one parent unit through a physical QR scan, bind every WebRTC session to that trust, and keep the pairing secret off disk in cleartext and out of logs. Migrated from ADR-003 §3–§4, ADR-004 (with the 2026-10-19 and 2026-10-20 amendments), ADR-017 §1 and ADR-018 §2.

## Requirements

### Requirement: One baby unit and one parent unit

A pairing SHALL connect exactly one baby unit and one parent unit. The invite SHALL close after the first parent has verified, and the relay SHALL refuse a second parent in the room with `PARENT_CAP`.

#### Scenario: Second parent in the room

- **WHEN** a second parent unit joins a room that already has a parent
- **THEN** the relay refuses it with `PARENT_CAP` and the existing pairing is unaffected

### Requirement: Invite shown by the baby unit

The baby unit SHALL show the invite as a QR code (inverted, at most 50 % brightness) and as copyable text. The parent unit SHALL accept the invite by scanning it, with the camera permission requested only at that moment, or by pasting the text. The invite SHALL be valid for 5 minutes with a visible countdown and a "Show new code" action, and each new code SHALL carry a fresh PSK.

#### Scenario: Code expires

- **WHEN** the parent scans a code whose expiry has passed
- **THEN** pairing is rejected as expired and the parent is told to show a new code on the baby unit

### Requirement: Invite payload v3

The invite SHALL be UTF-8 JSON `{"v":3,"k":<PSK>,"s":[<signaling URLs>],"x":<expiry>}`: the PSK as unpadded base64url of 32 random bytes, a non-empty list of `ws://` or `wss://` signaling URLs with a host, and the expiry in epoch seconds. It SHALL NOT carry a room id, fingerprint, IP address or device name. A payload with another version, a PSK of the wrong length, an invalid URL or a missing expiry SHALL be rejected as not a GuardedSleep code.

#### Scenario: Foreign QR code

- **WHEN** the parent scans a QR code that is not a v3 GuardedSleep invite
- **THEN** pairing is rejected as "not our code" and nothing is stored

### Requirement: Room id derived from the PSK

The signaling room id SHALL be the first 22 characters of base64url(HKDF-SHA256(PSK, info `gs-room/3`)), 128 bits. It SHALL NOT be reversible to the PSK and SHALL NOT be transmitted separately from the PSK.

#### Scenario: Room id from a known PSK

- **WHEN** both units derive the room id from the same PSK
- **THEN** they compute the same 22-character id, matching the conformance vectors

### Requirement: Direction-bound pairing proof

Each attempt SHALL be verified by `mac_role = HMAC-SHA256(PSK, "gs-pair-proof/3|" + role + "|" + room + "|" + fp_sender + "|" + fp_receiver)`. Each side SHALL send its own MAC and verify the peer's MAC with roles and fingerprints swapped, using a constant-time comparison. Peers on protocol v2 and v3 SHALL NOT pair with each other.

#### Scenario: Reflected proof

- **WHEN** an attacker who knows the room id reflects the baby unit's own proof back to it
- **THEN** verification fails and the microphone is never opened for that attempt

### Requirement: Fingerprint source

The fingerprint bound by the proof SHALL be parsed only from SDP lines that start with `a=fingerprint:`, and exactly one distinct, well-formed SHA-256 value SHALL be accepted. Otherwise the attempt SHALL fail with `fingerprint_invalid`.

#### Scenario: Fingerprint hidden in another line

- **WHEN** a relay puts a second SHA-256 fingerprint into another SDP line
- **THEN** the attempt fails with `fingerprint_invalid`

### Requirement: DTLS certificate cross-check

An attempt SHALL be verified only when the remote certificate fingerprint reported by the platform after DTLS connects equals the SDP fingerprint bound by the proof. This check SHALL be on by default and MAY be switched off only for a platform that cannot read WebRTC certificate statistics.

#### Scenario: DTLS runs against another certificate

- **WHEN** the DTLS peer presents a certificate whose fingerprint differs from the proven SDP fingerprint
- **THEN** the attempt is not verified

### Requirement: Pairing persisted only after verification

The parent unit SHALL persist a pairing only after its first verified attempt, so a wrong or foreign code never leaves a stored pairing. A stored pairing SHALL record that the PSK was verified, and SHALL allow monitoring to start again with one tap, without a new QR scan.

#### Scenario: Wrong code

- **WHEN** the parent scans a valid-looking code whose baby unit never verifies
- **THEN** no pairing is stored

### Requirement: Encrypted storage of pairing material

On Android the PSK, the derived room id and the signaling URLs SHALL be stored encrypted with an AES-GCM key held in the Android Keystore (StrongBox when available). Pairing material SHALL NOT appear in logs, Intents, Bundles, URLs or saved UI state, and app backup SHALL be disabled.

#### Scenario: Pasted code in the UI

- **WHEN** the parent pastes an invite into the text field and the activity is recreated
- **THEN** the pasted text is not restored from saved instance state

### Requirement: Forget pairing

"Forget" SHALL stop monitoring, cancel an open invite, delete the stored pairing and the peer name, cancel snooze, and restore the phone's Do Not Disturb state if the app had changed it.

#### Scenario: Forget on the baby unit

- **WHEN** the user forgets the pairing on a baby unit that silenced the phone
- **THEN** the pairing is deleted and the previous Do Not Disturb state is restored

### Requirement: Signaling transport security

Release builds SHALL refuse cleartext network traffic, so signaling uses `wss://`. Cleartext `ws://` SHALL be allowed only in debug builds.

#### Scenario: Release build with a ws:// URL

- **WHEN** a release build is given a `ws://` signaling URL on the internet
- **THEN** the connection is refused by the platform

### Requirement: One invite codec in the core

The core SHALL build and parse the invite (the current payload version of "Invite payload"), with the PSK bytes and the current epoch seconds supplied by the platform, and SHALL classify a rejected text as not our code, incomplete paste or expired. Every client SHALL use this codec through its binding, conformance vectors SHALL cover it through every binding, and no platform SHALL parse invite text itself.

#### Scenario: Cut-off paste

- **WHEN** a client passes an invite text that starts with `{` but is not complete JSON
- **THEN** the core rejects it as an incomplete paste, on Android and in the browser alike
