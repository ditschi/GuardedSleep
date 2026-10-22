# Tasks

Refined 2026-10-22 from the wave-2 plan. Most items were implemented as W2-B on branch `worktree-agent-a7e67909c63c181c6` (12 commits from 3140d426 to 39174b08); they are ticked after the rebase onto `chore/toolchain-wave-2` and the checks below pass.

## 1. Relay crate

- [x] 1.1 `guardedsleep-relay`: rooms, roles, pinning, forwarding, liveness and limits per the `signaling-relay` spec (sans-IO, `on_frame(conn, bytes, now) -> Vec<RelayEffect>`, cargo-deny bans, proptest model tests)
- [x] 1.2 Admission challenge as a relay mode: `Admission::Required(key)` for LAN relays, `Admission::Open` for the public server; protocol messages and `HKDF(PSK, "guardedsleep relay admission v1")` in the core; conformance vectors including a replayed first message
- [x] 1.3 Add `guardedsleep-relay` to the iOS check

## 2. Server

- [x] 2.1 `guardedsleep-server` (axum) on the relay crate: same WebSocket path, ports and env names as the Node server, health endpoint, no IP logging
- [x] 2.2 Built-in STUN: Binding requests only, response no larger than the request, per-IP rate limit, UDP 3478, decode without panics
- [x] 2.3 Distroless, non-root, read-only image (cargo-chef, `--healthcheck` probe); compose files; image workflow builds the Rust image natively for amd64 and arm64
- [x] 2.4 Parity: every Node relay test ported to Rust; `just dev`, Playwright E2E and the PoC gate scripts run against the Rust server; smoke test replaces `smoke-relay.ts`
- [x] 2.5 Delete `apps/signaling-server` and `packages/protocol` (moved here from `add-core-facades`); `apps/mobile` conformance test uses the generated types

## 3. Clients

- [ ] 3.1 Android: built-in STUN of the configured server by default; public STUN opt-in in Advanced with copy naming who sees the IP address (after `swap-webrtc-sdk` is merged, because both touch the ICE configuration)

## 4. Close

- [ ] 4.1 `docs/ci-gates.md` rows; `just lint`, `just test`, `just e2e` green
