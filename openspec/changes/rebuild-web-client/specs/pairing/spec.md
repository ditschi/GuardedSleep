# Spec Delta

## MODIFIED Requirements

### Requirement: Invite payload v3

The invite SHALL be UTF-8 JSON `{"v":3,"k":<PSK>,"s":[<signaling URLs>],"x":<expiry>}`: the PSK as unpadded base64url of 32 random bytes, a non-empty list of `ws://` or `wss://` signaling URLs with a host, and the expiry in epoch seconds. It SHALL NOT carry a room id, fingerprint, IP address or device name. A payload with another version, a PSK of the wrong length, an invalid URL or a missing expiry SHALL be rejected as not a GuardedSleep code.

#### Scenario: Foreign QR code

- **WHEN** the parent scans a QR code that is not a v3 GuardedSleep invite
- **THEN** pairing is rejected as "not our code" and nothing is stored

## ADDED Requirements

### Requirement: One invite codec in the core

The core SHALL build and parse the v3 invite, with the PSK bytes and the current epoch seconds supplied by the platform, and SHALL classify a rejected text as not our code, incomplete paste or expired. Every client SHALL use this codec through its binding, conformance vectors SHALL cover it through every binding, and no platform SHALL parse invite text itself.

#### Scenario: Cut-off paste

- **WHEN** a client passes an invite text that starts with `{` but is not complete JSON
- **THEN** the core rejects it as an incomplete paste, on Android and in the browser alike
