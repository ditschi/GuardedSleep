# Tasks

Coarse tasks; refine them from the wave-2 plan before `/opsx:apply`.

## 1. Relay crate

- [ ] 1.1 `guardedsleep-relay`: rooms, roles, pinning, forwarding, liveness and limits per the `signaling-relay` spec (sans-IO, cargo-deny bans)
- [ ] 1.2 Admission challenge with conformance vectors, including a replayed first message

## 2. Server

- [ ] 2.1 `guardedsleep-server` (axum) on the relay crate, with built-in STUN
- [ ] 2.2 Distroless, non-root, read-only image; update the image workflow
- [ ] 2.3 Run the relay scenarios against Node and Rust; delete `apps/signaling-server`

## 3. Clients

- [ ] 3.1 Android: built-in STUN by default; public STUN opt-in in Advanced with copy naming who sees the IP address

## 4. Close

- [ ] 4.1 `docs/ci-gates.md` rows; `just ci` green
