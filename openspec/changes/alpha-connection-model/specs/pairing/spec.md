# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Invite payload v3`
- TO: `### Requirement: Invite payload v4`

## MODIFIED Requirements

### Requirement: Invite payload v4

The invite SHALL be UTF-8 JSON `{"v":4,"k":<PSK>,"m":[<mailbox>…],"x":<expiry>}`: the PSK as unpadded base64url of 32 random bytes, a list of one to six mailboxes, each `{"t":"relay"|"lan"|"nostr","u":<ws:// or wss:// URL with a host>}`, and the expiry in epoch seconds. A `lan` mailbox SHALL use a private LAN literal address; `relay` and `nostr` mailboxes SHALL use `wss://`. The encoded invite SHALL be at most 480 bytes. It SHALL NOT carry a room id, mailbox tag, fingerprint, device name or any public IP address. A payload with another version (including v3), a PSK of the wrong length, an invalid or misplaced URL, too many mailboxes or a missing expiry SHALL be rejected as not a GuardedSleep code.

#### Scenario: Foreign QR code

- **WHEN** the parent scans a QR code that is not a v4 GuardedSleep invite
- **THEN** pairing is rejected as "not our code" and nothing is stored

#### Scenario: Invite at home with internet

- **WHEN** the baby device runs its LAN listener and has internet
- **THEN** its invite carries one `lan` mailbox and three `nostr` mailboxes from different operators

#### Scenario: Public address in a LAN mailbox

- **WHEN** an invite names a `lan` mailbox with a public IP address
- **THEN** it is rejected as not a GuardedSleep code

### Requirement: Signaling transport security

Signaling to the internet SHALL use `wss://`. Release builds SHALL allow cleartext `ws://` only toward private LAN literal addresses (RFC 1918, RFC 4193 and link-local), checked in one place in the platform layer before every connection; 100.64/10 and host names SHALL be refused. Debug builds MAY use cleartext toward any address. On every mailbox the signaling content SHALL be a sealed envelope (`signaling-mailboxes`), so cleartext transport on the LAN exposes no pairing material or SDP.

#### Scenario: Release build with a ws:// URL

- **WHEN** a release build is given a `ws://` signaling URL on the internet
- **THEN** the connection is refused by the platform

#### Scenario: Offline baby device on the home Wi-Fi

- **WHEN** a release build connects to `ws://192.168.1.20:<port>`
- **THEN** the connection is allowed, and a passive observer on the Wi-Fi sees only sealed envelopes
