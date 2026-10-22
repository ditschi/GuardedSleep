# Design

## Context

Sources: ADR-020 §3 (crates, bans), §6 (server), ADR-021 §1 (relay admission), ADR-017 §10 (liveness, limits). The Node relay's behaviour is the `signaling-relay` spec; the Rust server must pass it unchanged except for the deltas here.

## Goals / Non-Goals

**Goals:** parity with `signaling-relay`, built-in STUN, admission, hardened image. **Non-Goals:** TURN (T1), LAN listeners, static UI.

## Decisions

- **Rust axum on a sans-IO `relay` crate, now** (ADR-020 §6, owner D6). The `relay` crate is shared by the server and the Android LAN listener; the platform owns the sockets. cargo-deny bans tokio, mio, getrandom and socket crates from the `relay` graph.
- **Built-in STUN answers Binding requests only**: response no larger than the request, per-IP rate limit, UDP 3478 published beside the reverse proxy.
- **Relay admission** (ADR-021 §1): the paired client gives its relay `HKDF(PSK, "guardedsleep relay admission v1")`; the relay sends a fresh nonce; the joiner answers `HMAC(admission key, nonce ‖ direction ‖ room id)`; a role slot is assigned only after a valid answer; unauthenticated sockets close within 5 s. The key is not reversible to the PSK, so a PC relay never holds the PSK.
- **Admission is a relay mode** (decided 2026-10-22, consistent with ADR-021 §1): LAN relays run `Admission::Required` with the key handed over by the paired client; the public server runs `Admission::Open`, because it never knows the PSK. On the public server, end-to-end security still rests on the direction-bound pairing proof and the DTLS fingerprint binding, and room ids are HKDF-derived and unguessable.
- **Local-mode hooks already exist in the server** (`--serve-web`, `--local`, loopback key registration; W2-B implemented them ahead of time). They are not part of this change's spec deltas: `add-offline-lan` specifies, reviews and archives them (owner decision 2026-10-22). Until then they are undocumented as product behaviour.
- **`packages/protocol` is deleted here**, together with its last consumer, the Node server (moved from `add-core-facades`).
- **Discarded alternatives:** keep the Node server until after the alpha (two validators to maintain, no code shared with the Android listener); public STUN by default (leaks IP addresses to a third party without consent); a takeover of the baby slot instead of liveness (ADR-017 Option D: any room member could disconnect the baby).

## Risks / Trade-offs

- Parity gaps between Node and Rust: mitigated by running the `signaling-relay` scenarios against both before deleting the Node server.
- Built-in STUN behind NAT-unfriendly hosting: public STUN stays available as an opt-in.
