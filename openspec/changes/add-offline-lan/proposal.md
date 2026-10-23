# Proposal

## Why

GuardedSleep must also work without internet, with devices on the same Wi-Fi or hotspot (ADR-012 goal 3). ADR-021 §1 (owner D1) decided two topologies: Android ↔ Android with an embedded listener on the baby unit, and Android ↔ PC in a local PC mode where the server binary serves the web UI on `http://localhost`. An HTTPS page cannot reach a phone without a certificate warning, which would undermine pairing as the trust anchor.

The target moved from beta to **alpha** (owner, 2026-10-23). This change follows `alpha-connection-model`: the LAN relay becomes one more mailbox (kind `lan`) next to the internet mailboxes, so a LAN session needs no mode switch and survives a lost uplink. It changes the alpha scope and the LAN security model (room id never in clear on the LAN, address recovery), so the Critical Reviewer role applies.

## What Changes

- New capability spec `offline-lan`:
  - the embedded Android listener as a `lan` mailbox, active while an invite is shown or monitoring runs;
  - opaque NSD discovery on a fixed default port;
  - local PC mode with loopback-only UI and API, the binary opening the browser itself;
  - relay admission on every LAN relay, with no stable room id in LAN frames;
  - recovery after a LAN address change;
  - the private-address cleartext rule;
  - plain user copy.
- `pairing`: release builds allow cleartext signaling toward private LAN literal addresses only; LAN invites use RFC 1918 or ULA addresses.
- The invite carries the LAN mailbox (`alpha-connection-model` invite v4), so the phone needs no manual input.
- Out of scope:
  - HTTPS web ↔ phone listener (`REJECTED`);
  - Chrome Local Network Access (`IDEA`);
  - offline iOS (`DEFERRED`);
  - **second-PC forwarding**, so PC ↔ PC offline is deferred (backlog `IDEA`; owner, 2026-10-23 split);
  - an Open-mode LAN Docker relay without admission.

## Capabilities

### New Capabilities

- `offline-lan`: monitoring between devices on the same local network without internet.

### Modified Capabilities

- `pairing`: cleartext signaling toward private LAN addresses in release builds.

## Impact

- **Tier:** T0 (alpha), after `alpha-connection-model`.
- **Personas:** serves Persona C (old phones on the home Wi-Fi) and Persona B (home without internet or during an outage), and Persona A at hotspots. Trade-off: captive portals and client isolation on venue Wi-Fi block LAN paths.
- **Code:**
  - Android platform: listener on the `relay` crate, NSD, Wi-Fi and multicast locks, the network security config, the address check, and permissions;
  - `guardedsleep-server` local mode;
  - the web client's local PC mode detection;
  - onboarding copy and `docs/ci-gates.md`.
- **Depends on:** `alpha-connection-model` (mailbox set, envelope, invite v4) and `rebuild-web-client`.
