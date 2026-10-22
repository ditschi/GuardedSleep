# Design

## Context

Sources: ADR-021 §1 (topologies, local PC mode, admission, listener rules, Android cleartext, user copy), ADR-012 (goals, embedded listener lifecycle, opaque NSD token, threat model; state machine and handoff policy are kept as built during the waves and re-evaluated afterwards, owner 2026-10-21), ADR-003 §5 (stored addresses and local discovery), `connectivity-pairing.md` (local/hotspot, LAN Docker fallback).

## Goals / Non-Goals

**Goals:** the two offline topologies of ADR-021 §1, secure by the pairing proof and DTLS binding, with the LAN relay treated as untrusted.

**Non-Goals:** live Wi-Fi ↔ cellular handoff and path ranking (ADR-012 §2, §4: re-evaluated after wave 3); iOS.

## Decisions

- **Embedded listener on the Android baby unit** (ADR-012 Option A): binds only while monitoring runs, holds Wi-Fi and multicast locks, advertises an opaque per-session NSD token never derived from the PSK.
- **Local PC mode**: the browser talks only to its own local binary over loopback (`http://localhost` is a secure context, so microphone, service worker, notifications and wake lock work in every browser); only the LAN signaling port binds to the LAN; `Origin` and `Host` are checked on every request; a second PC's binary forwards to the hosting relay.
- **Relay admission** on both relay kinds (from `add-rust-signaling-server`), so a replayed first message cannot take the single parent slot.
- **Android cleartext**: the network security config allows cleartext (it cannot express CIDR ranges); one platform check before every connection admits only RFC 1918, RFC 4193 (ULA) and link-local literal addresses and refuses 100.64/10 and host names. This supersedes ADR-018's "release builds disable cleartext" from beta.
- **Discarded alternatives:** HTTPS web ↔ phone listener with a self-signed certificate (`REJECTED`: teaches users to accept certificate warnings); SDP over NSD TXT records, a raw TCP/UDP socket, BLE or Wi-Fi Aware (ADR-012 Options B, C, E); a mandatory user-run LAN Docker server (Option D: stays a documented power-user path, not the default); Chrome Local Network Access (`IDEA`, Chromium only, no WebSockets yet).

## Risks / Trade-offs

- Captive portals and client isolation on venue Wi-Fi block LAN paths; the copy says so and points to mobile data (backlog: out-of-home detection).
- A user-visible "local mode" status is part of the planned connection-status UI review, not this change.
