# Spec Delta

## Purpose

Monitor between devices on the same Wi-Fi or hotspot without internet, in two topologies: Android ↔ Android with a listener on the baby unit, and Android ↔ PC in local PC mode. Decided in ADR-021 §1 and ADR-012; alpha target (owner, 2026-10-23). The LAN relay is one mailbox of the pairing (`signaling-mailboxes`).

## ADDED Requirements

### Requirement: Embedded listener on the Android baby unit

The Android baby unit SHALL run a LAN signaling listener on the shared relay logic only during an active session (while its invite is shown or monitoring runs), on a fixed default port with a fallback, SHALL keep Wi-Fi and multicast locks while it runs, and SHALL stop it when the session ends or the service is killed.

#### Scenario: Monitoring stops

- **WHEN** the baby unit stops monitoring
- **THEN** the listener socket is closed and nothing is advertised any more

#### Scenario: Invite shown

- **WHEN** the baby unit shows its invite on a Wi-Fi network
- **THEN** the listener runs and the invite carries its LAN address

### Requirement: Opaque discovery

The listener SHALL be advertised by NSD under a random per-session token and SHALL NOT advertise anything derived from the PSK or the room id.

#### Scenario: Nosy guest on the Wi-Fi

- **WHEN** a guest browses NSD services on the same Wi-Fi
- **THEN** they see only a random token that changes every session

### Requirement: Local PC mode

The GuardedSleep server binary SHALL offer a local mode that serves the web UI on `http://localhost`, opens the default browser on it, binds the UI and its API only to loopback, checks `Origin` and `Host` on every request, and binds only the signaling port to the LAN.

#### Scenario: Malicious website in another tab

- **WHEN** a website in another browser tab sends requests to the local binary
- **THEN** they are refused by the `Origin` and `Host` checks

#### Scenario: Starting local mode

- **WHEN** a user starts the local GuardedSleep app on a PC
- **THEN** the browser opens on the app without the user typing an address

### Requirement: Admission on LAN relays

Every LAN relay, the phone listener and the PC relay alike, SHALL use the relay admission challenge without any room id in LAN frames, SHALL close unauthenticated sockets within 5 seconds, and SHALL carry only sealed envelopes after admission; end-to-end security SHALL come from the pairing proof, the DTLS fingerprint binding and the envelope.

#### Scenario: Unpaired device connects to the listener

- **WHEN** a device without the pairing key opens a socket to the baby unit's listener
- **THEN** it is closed within 5 seconds without getting a role slot

#### Scenario: Passive observer on the Wi-Fi

- **WHEN** someone captures all LAN traffic of a pairing and monitoring session
- **THEN** they find no room id, PSK, SDP or ICE candidate

### Requirement: LAN address in the invite

The invite SHALL carry the LAN signaling address reported by the listener or the server as a LAN mailbox with an RFC 1918 or RFC 4193 address, next to the pairing's internet mailboxes, so the parent needs no manual input; the page SHALL never report the address itself.

#### Scenario: Pairing with a PC on the LAN

- **WHEN** a phone scans the invite shown by a PC in local mode
- **THEN** it connects to the PC's LAN signaling address without manual input

### Requirement: Recovery after a LAN address change

The baby unit SHALL announce its current LAN address inside its sealed presence over every working mailbox. Without internet, the parent unit SHALL browse local discovery and try admission against each GuardedSleep listener it finds, at most four per minute, with the timing owned by the core.

#### Scenario: Router restart offline

- **WHEN** the router restarts without internet and the baby unit gets a new address
- **THEN** the parent unit finds the listener through discovery and monitoring recovers without re-pairing

### Requirement: Honest offline copy

The app SHALL say plainly that a laptop without internet needs the local GuardedSleep app running, and SHALL never imply that an ordinary browser tab works offline.

#### Scenario: Laptop without internet

- **WHEN** a user opens the hosted web app on a laptop without internet
- **THEN** the app explains that offline use needs the local GuardedSleep app

### Requirement: LAN and internet together

When the devices share a network and the internet is also reachable, the LAN mailbox and the internet mailboxes SHALL be used at the same time. Losing the internet SHALL NOT interrupt a session that runs on the LAN.

#### Scenario: Router loses its internet uplink

- **WHEN** the home internet uplink fails while baby and parent unit monitor on the same Wi-Fi
- **THEN** monitoring continues over the LAN without an alarm, and the diagnostics view shows a direct LAN connection

#### Scenario: Browser parent offline in local PC mode

- **WHEN** a browser parent in local PC mode pairs with an Android baby unit without internet
- **THEN** the attempt verifies over the LAN even though the browser hides its host addresses behind mDNS names
