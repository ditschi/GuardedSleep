# Proposal

## Why

ADR-020 §4 decides `io.github.webrtc-sdk:android` behind the Kotlin `MediaEngine` interface, with a CVE-based patch policy, so the WebRTC vendor can be replaced without touching platform logic. Tier T0, wave 2 (group 1).

## What Changes

- Android uses `io.github.webrtc-sdk:android` behind `MediaEngine`; no behaviour change for users.
- The libwebrtc patch policy becomes a toolchain rule.
- Building libwebrtc from source stays `DEFERRED` with F-Droid main.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: libwebrtc patch policy.

## Impact

`apps/android` media layer, Gradle version catalog, Renovate (7-day release age for libwebrtc).
