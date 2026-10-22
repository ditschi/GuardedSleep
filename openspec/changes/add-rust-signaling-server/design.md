# Design

## Context

Sources: ADR-020 §3 (crates, bans), §6 (server), ADR-021 §1 (relay admission), ADR-017 §10 (liveness, limits). The Node relay's behaviour is the `signaling-relay` spec; the Rust server must pass it unchanged except for the deltas here.

## Goals / Non-Goals

**Goals:** parity with `signaling-relay`, built-in STUN, admission, hardened image. **Non-Goals:** TURN (T1), LAN listeners, static UI.

## Decisions

- **Rust axum on a sans-IO `relay` crate, now** (ADR-020 §6, owner D6). The `relay` crate is shared by the server and the Android LAN listener; the platform owns the sockets. cargo-deny bans tokio, mio, getrandom and socket crates from the `relay` graph.
- **Built-in STUN answers Binding requests only**: response no larger than the request, per-IP rate limit, UDP 3478 published beside the reverse proxy.
- **Relay admission** (ADR-021 §1): the paired client gives its relay `HKDF(PSK, "guardedsleep relay admission v1")`; the relay sends a fresh nonce; the joiner answers `HMAC(admission key, nonce ‖ direction ‖ room id)`; a role slot is assigned only after a valid answer; unauthenticated sockets close within 5 s. The key is not reversible to the PSK, so a PC relay never holds the PSK.
- **Discarded alternatives:** keep the Node server until after the alpha (two validators to maintain, no code shared with the Android listener); public STUN by default (leaks IP addresses to a third party without consent); a takeover of the baby slot instead of liveness (ADR-017 Option D: any room member could disconnect the baby).

## Risks / Trade-offs

- Parity gaps between Node and Rust: mitigated by running the `signaling-relay` scenarios against both before deleting the Node server.
- Built-in STUN behind NAT-unfriendly hosting: public STUN stays available as an opt-in.
