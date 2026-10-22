# Proposal

## Why

The relay runs on the legacy Node server, the client uses public STUN by default without telling the user (against AGENTS.md §7.1), and the LAN relays of ADR-021 need an admission challenge. ADR-020 §6 decided a Rust server on a sans-IO `relay` crate now rather than after the alpha (owner decision D6: one fuzzed validator shared with the clients, relay logic shared with the Android LAN listener). Tier T0, wave 2 (group 1).

## What Changes

- New crates `guardedsleep-relay` (sans-IO rooms and admission) and `guardedsleep-server` (axum); `apps/signaling-server` is deleted once parity with the `signaling-relay` spec is proven.
- Built-in STUN in the server; the client uses it by default, public STUN only as an opt-in with copy that names who sees the IP address.
- Relay admission challenge for LAN relays (phone listener and local PC mode), with a negative vector for a replayed first message.
- Hardened image: distroless, non-root, read-only root filesystem.
- Out of scope: TURN profile and compose bundle (T1, backlog), the LAN listeners themselves (`add-offline-lan`), serving the static web UI (`add-offline-lan`).

## Capabilities

### New Capabilities

### Modified Capabilities

- `signaling-relay`: built-in STUN, client ICE defaults, relay admission.
- `distribution`: hardened signaling image.

## Impact

`crates/guardedsleep-relay`, `crates/guardedsleep-server`, `apps/signaling-server` (deleted), `docker/signaling-server`, Android `config/Policy.kt` and settings copy, `deny.core.toml` (relay graph), `docs/ci-gates.md`.
