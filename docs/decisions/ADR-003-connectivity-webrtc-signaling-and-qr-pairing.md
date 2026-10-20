# ADR-003: Connectivity -- WebRTC, Three Paths, Signaling, QR Pairing

- Status: DECIDED
- Date: 2026-04-15
- Deciders: Architecture + security review inputs

> **Context note (2026-10-20):** the "Local: no server" path is realised through the offline LAN topologies in [ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md) §1 (see also [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md)). No decision here is changed.

## Context

Devices must connect over LAN, over the internet (without forcing users to install Tailscale), and optionally over VPN; pairing must be simple and secure.

## Decision

1. **Transport:** **WebRTC** peer-to-peer -- DataChannel for heartbeat, alerts, control; MediaStream for audio (pre-negotiated muted track, enable on alert/listen).

2. **Three connectivity paths** (same app logic -- connect to IPs):
   - **Local:** WiFi / hotspot. _No separate signaling **service/product** is required; clarification in [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md) -- a small signaling endpoint **on one of the peers** is still needed for SDP/ICE exchange and ships in Phase 2 as an embedded WebSocket listener on the Baby-Android unit._
   - **Internet (primary):** **WebSocket signaling server** + **EU STUN** (fallback TURN self-hosted optional).
   - **Internet (optional):** **Tailscale/Headscale** -- external VPN; app treats VPN IPs like any other.

3. **Pairing:** **QR code** with JSON payload, human-readable keys: `psk`, `dtls_fingerprint` (64 hex chars), `ips`, `signal` (optional), `nonce`, `ts`. Payload size **verified** to fit QR Version 12--13 with EC-M (see [analysis.md](../technical/analysis.md)).

4. **Behavior:** QR visible → auto-accept scan; QR dismissed → join requests with device name. **No new QR display in baby kiosk mode** for pairing new devices (exit baby mode to pair) -- see device management spec.

5. **Discovery / reconnect:** Stored IPs + signaling URL + local discovery (mDNS/UDP) as specified in feature docs.

6. **Heartbeat:** Default **10 s** (configurable 5--30 s in settings).

## Consequences

### Positive

- Mainstream users do not need Tailscale; power users can still use VPN.

### Negative

- Signaling server sees connection metadata (not media) -- must be documented and self-hostable.

### Risks

- TURN may be needed for some NAT pairs -- optional coturn in deployment docs.

## Discarded alternatives

- Tailscale-only internet -- rejected (UX/privacy trade-offs previously documented).
- Full relay of media through server -- rejected.
