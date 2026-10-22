# Proposal

## Why

GuardedSleep must also work without internet, with devices on the same Wi-Fi or hotspot (ADR-012 goal 3). ADR-021 §1 (owner D1) decides two topologies: Android ↔ Android with an embedded listener on the baby unit, and Android ↔ PC / PC ↔ PC in a local PC mode where the server binary serves the web UI on `http://localhost`. An HTTPS page cannot reach a phone without a certificate warning, which would undermine pairing as the trust anchor. Target: beta (an exit criterion of the beta).

## What Changes

- New capability spec `offline-lan`: embedded Android listener with opaque NSD discovery, local PC mode with loopback-only UI and API, relay admission on every LAN relay, the private-address cleartext rule, and plain user copy.
- `pairing`: release builds allow cleartext signaling toward private LAN addresses only.
- The invite carries the LAN signaling address reported by the server or listener, so the phone needs no manual input.
- Out of scope: HTTPS web ↔ phone listener (`REJECTED`), Chrome Local Network Access (`IDEA`), offline iOS (`DEFERRED`), automatic LAN ↔ internet handoff (`IDEA`; switching stays manual), the connection-model re-evaluation after wave 3 (backlog).

## Capabilities

### New Capabilities

- `offline-lan`: monitoring between devices on the same local network without internet.

### Modified Capabilities

- `pairing`: cleartext signaling toward private LAN addresses in release builds.

## Impact

Android platform layer (listener on the `relay` crate, NSD, Wi-Fi and multicast locks, network security config, address check), `guardedsleep-server` (local mode, static UI, loopback API), web client (local PC mode), onboarding copy, `docs/ci-gates.md`. Depends on `add-rust-signaling-server` (relay crate, admission) and `rebuild-web-client`.
