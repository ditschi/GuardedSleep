# Design

## Context

Sources: ADR-003 §2, ADR-004 §4, ADR-017 §10, `connectivity-pairing.md` (internet, VPN sections). As built: the Node relay in `apps/signaling-server` (legacy, replaced in wave 2) and the Android ICE configuration (`config/Policy.kt`, `media/MediaEngine.kt`).

## Goals / Non-Goals

**Goals:** a relay spec that the Rust server can be checked against for parity.

**Non-Goals:** the Rust server, built-in STUN, relay admission, the static web UI and TURN; they are decided (ADR-020 §6, ADR-021 §1) but not built, and go into the prepared change `add-rust-signaling-server` and the backlog.

## Decisions

- **Spec the behaviour, not the Node implementation**: message types, error codes, limits and defaults are part of the contract; environment variable names are kept only where operators rely on them (`SIGNALING_MAX_PARENTS`).
- **VPN** (owner, 2026-10-21): no special support; the "no signaling server needed over VPN" claim of the feature doc is `REJECTED`, because cleartext signaling toward 100.64/10 is refused (ADR-021 §1).
- **Public signaling instance** (owner, 2026-10-21): open. The spec records the fact that release builds ship without a default URL; operating a project instance is `UNDER DISCUSSION` in the backlog.
- **Guardrail gap found:** the client uses public STUN by default without telling the user who sees the IP, which AGENTS.md §7.1 forbids. Built-in STUN with public STUN as an opt-in is part of `add-rust-signaling-server`; until then the gap is listed in the backlog under the security audit.
- **Superseded:** ADR-003 §6 heartbeat (by ADR-017 §5); the feature doc's "QR contains all device IPs" (by the v3 invite).

## Risks / Trade-offs

- Specifying a legacy implementation could carry its quirks into the Rust server. Mitigation: only behaviour the clients depend on is specified (codes they handle, limits ADR-017 §10 asked for).
