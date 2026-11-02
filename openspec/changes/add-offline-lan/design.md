# Design

## Context

Sources:

- ADR-021 §1: topologies, local PC mode, admission, listener rules, Android cleartext, user copy.
- ADR-012: goals, embedded listener lifecycle, opaque NSD token, threat model.
- ADR-003 §5: stored addresses and local discovery.
- `connectivity-pairing.md`: local and hotspot use, the LAN Docker fallback.
- `alpha-connection-model`: mailbox set, sealed session-bound envelope, invite v4 with `l:` entries.

Retargeted to the alpha on 2026-10-23. The findings of the critical review of `alpha-connection-model` that concern the LAN are answered here.

## Goals / Non-Goals

**Goals:**

- Android ↔ Android and Android ↔ PC on one network without internet.
- Security from the pairing proof, the DTLS binding and the sealed envelope, with the LAN relay treated as untrusted.
- Automatic recovery after a LAN address change.

**Non-Goals:**

- Path ranking beyond what ICE does.
- PC ↔ PC offline, which needs second-PC forwarding (`IDEA`).
- iOS.

## Decisions

### Embedded listener on the Android baby unit (ADR-012 Option A; layer: platform)

- **Lifecycle:** it binds only during an active session, that is while an invite is shown or monitoring runs. It holds Wi-Fi and multicast locks while it runs.
- **Port and discovery:** it uses a fixed default port (with a fallback port when that one is taken) and advertises an opaque per-session NSD token that is never derived from the PSK.
- **Mailbox:** it is the pairing's `lan` mailbox. The relay logic is the shared `relay` crate in Required admission mode.

### No stable room id on the LAN (layer: relay crate)

- **The problem:** today `admit` carries the room id in clear over `ws://`, so a passive observer, or a foreign host reached at the same private address on another network, learns it.
- **The fix:** on LAN relays the joiner sends no room id. It proves membership with `mac = HMAC(admission_key, "gs-relay-admit/2|" ‖ nonce ‖ "|" ‖ role)`. The listener serves one pairing, so its admission key identifies the room.
- **Local PC relay:** it may host more than one pairing. It tries each registered key, a constant-time loop over at most 4 keys.
- **Wire content:** after admission, only sealed envelopes cross the LAN. No room id appears in `welcome` or `members` on LAN relays.
- **Slot squatting:** a rogue NSD advertiser can relay a real listener's nonce to the parent and get admitted (a chosen-nonce relay). It sees only sealed envelopes, but could hold the parent slot. LAN relays therefore let the newest admitted parent replace the old socket, while the parent proof (`alpha-connection-model` §4) still decides who gets audio.
- **PC relay keys:** admission keys are memory-only (AGENTS.md §7.2). A fifth pairing is refused with a plain message.

### Recovery after a LAN address change (layer: core decides, platform discovers)

- **Baby side:** the baby announces its current LAN address and port inside its sealed `here` over every working mailbox, and in the mailbox update over the DataChannel.
- **Parent side:** without internet, the parent browses NSD for GuardedSleep listeners and tries admission against each one, at most 4 per minute. Only the listener holding the pairing's key admits it.
- **Ownership:** the core owns the retry timing. The platform only reports what NSD finds.

### Local PC mode (layer: server binary, web client)

- **Network exposure:** the browser talks only to its own local binary over loopback. `http://localhost` is a secure context, so microphone, service worker, notifications and wake lock work. Only the LAN signaling port binds to the LAN, and `Origin` and `Host` are checked on every request.
- **Start-up:** the binary opens the default browser on its URL by itself, so nobody types `localhost:<port>`.
- **Web client:** it detects the local mode and adds the `lan` mailbox to its invite.

### Android cleartext (layer: platform)

- **Configuration:** the base network security config is **permissive for cleartext**, because it cannot express CIDR ranges. Release safety rests on the one guarded check below, not on the config.
- **One factory:** every socket and HTTP client is created through one guarded connection factory, enforced by a lint or test that fails on direct construction.
- **Parser tests:** IPv4-mapped IPv6 (`::ffff:8.8.8.8`), NAT64 `64:ff9b::/96`, decimal and octal IPv4 forms, zone ids, `0.0.0.0`, `127/8`, and ULA `fc00::/7` versus `fd00::/8`.
- **The check:** one platform check before every connection admits only RFC 1918 and RFC 4193 (ULA) literal addresses. It refuses link-local addresses, 100.64/10 and host names.
- **What it supersedes:** ADR-018's "release builds disable cleartext".
- **Other own relays:** a `ws://` own relay toward a private address is treated as a `lan` mailbox and needs Required admission. An Open-mode LAN Docker relay is refused in release builds; the documented power-user path uses `wss://` or Required mode.

### Android permissions

- Check `ACCESS_WIFI_STATE`, `CHANGE_WIFI_MULTICAST_STATE` and any local-network runtime permission of the pinned target SDK.
- Update the expected permission set (`add-android-release` 1.4).
- If a runtime prompt is needed, add a setup checklist row.

### Browser mDNS candidates (risk, tested)

A browser parent without microphone permission hides its host candidates behind `.local` mDNS names, and offline there is no STUN. The Android baby must therefore resolve mDNS candidates through libwebrtc. A gate scenario covers it. If it fails, the web parent asks for microphone permission in local mode, which reveals the host candidates, and the copy explains why.

### Discarded alternatives

- **HTTPS web ↔ phone listener with a self-signed certificate** (`REJECTED`): teaches users to accept certificate warnings.
- **SDP over NSD TXT records, a raw TCP/UDP socket, BLE or Wi-Fi Aware** (ADR-012 Options B, C, E).
- **A mandatory user-run LAN Docker server** (Option D): stays a documented power-user path, not the default.
- **Chrome Local Network Access** (`IDEA`): Chromium only, no WebSockets yet.
- **Room id in clear in `admit` on the LAN**: leaks a stable identifier to passive observers and foreign hosts.
- **Listener only while monitoring**: the invite needs the LAN address while the QR code is shown.
- **Second-PC forwarding in the alpha**: deferred to keep the change small (owner, 2026-10-23).
- **A random NSD port only**: the parent cannot recover after an address change without internet.

## Risks / Trade-offs

| Risk                                                                | Mitigation                                                                                     |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Captive portals and client isolation on venue Wi-Fi block LAN paths | the copy says so and points to mobile data (backlog: out-of-home detection)                    |
| NSD is unreliable on some routers                                   | the address in sealed `here` whenever any mailbox works; re-pair as the last resort, with copy |
| mDNS candidate resolution                                           | the gate scenario and the microphone-permission fallback                                       |
| A local "mode" status in the UI                                     | part of the planned connection-status UI review, not this change                               |
