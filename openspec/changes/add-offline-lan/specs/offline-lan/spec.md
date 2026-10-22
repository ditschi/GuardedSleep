# Spec Delta

## Purpose

Monitor between devices on the same Wi-Fi or hotspot without internet, in two topologies: Android ↔ Android with a listener on the baby unit, and Android ↔ PC or PC ↔ PC in local PC mode. Decided in ADR-021 §1 and ADR-012; target beta.

## ADDED Requirements

### Requirement: Embedded listener on the Android baby unit

The Android baby unit SHALL run a LAN signaling listener on the shared relay logic only while monitoring is active, SHALL keep Wi-Fi and multicast locks while it runs, and SHALL stop it when monitoring stops or the service is killed.

#### Scenario: Monitoring stops

- **WHEN** the baby unit stops monitoring
- **THEN** the listener socket is closed and nothing is advertised any more

### Requirement: Opaque discovery

The listener SHALL be advertised by NSD under a random per-session token and SHALL NOT advertise anything derived from the PSK or the room id.

#### Scenario: Nosy guest on the Wi-Fi

- **WHEN** a guest browses NSD services on the same Wi-Fi
- **THEN** they see only a random token that changes every session

### Requirement: Local PC mode

The GuardedSleep server binary SHALL offer a local mode that serves the web UI on `http://localhost`, binds the UI and its API only to loopback, checks `Origin` and `Host` on every request, and binds only the signaling port to the LAN. A second PC SHALL use its own local binary, which forwards to the hosting relay.

#### Scenario: Malicious website in another tab

- **WHEN** a website in another browser tab sends requests to the local binary
- **THEN** they are refused by the `Origin` and `Host` checks

### Requirement: Admission on LAN relays

Every LAN relay, the phone listener and the PC relay alike, SHALL use the relay admission challenge and close unauthenticated sockets within 5 seconds; end-to-end security SHALL come from the pairing proof and the DTLS fingerprint binding.

#### Scenario: Unpaired device connects to the listener

- **WHEN** a device without the pairing key opens a socket to the baby unit's listener
- **THEN** it is closed within 5 seconds without getting a role slot

### Requirement: LAN address in the invite

The invite SHALL carry the LAN signaling address reported by the listener or the server, so the parent needs no manual input; the page SHALL never report the address itself.

#### Scenario: Pairing with a PC on the LAN

- **WHEN** a phone scans the invite shown by a PC in local mode
- **THEN** it connects to the PC's LAN signaling address without manual input

### Requirement: Honest offline copy

The app SHALL say plainly that a laptop without internet needs the local GuardedSleep app running, and SHALL never imply that an ordinary browser tab works offline.

#### Scenario: Laptop without internet

- **WHEN** a user opens the hosted web app on a laptop without internet
- **THEN** the app explains that offline use needs the local GuardedSleep app
