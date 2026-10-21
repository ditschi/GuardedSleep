# Feature: Connectivity and Pairing

## Overview

Manages how baby and parent units discover, authenticate, and communicate with each other. The system must work both over the internet and on local networks (including ad-hoc hotspot scenarios). Security is paramount -- no unauthorized device should be able to connect.

## Requirements

- `[STATUS: DECIDED]` **MVP (Phase 1):** exactly **one** baby device and **one** parent device (1:1). **Phase 2:** up to **4** parent devices per baby (product limit); signaling/server may allow more — app enforces 4.
- `[STATUS: DECIDED]` Works over the internet (signaling server + EU STUN, or optional VPN) -- in MVP
- `[STATUS: DECIDED]` Works on local networks without internet (WiFi, hotspot). Superseded wording "in MVP / Phase 2" (the feature list and the listener section disagreed): the offline LAN topologies below are DECIDED with target **beta** ([ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §1); the alpha uses internet signaling for all pairs
- `[STATUS: DECIDED]` Offline topology **Android <-> Android**: the Android baby unit runs an embedded signaling listener while monitoring is active; the parent finds it by NSD or from the QR invite -- target **beta**
- `[STATUS: DECIDED]` Offline topology **Android <-> PC/laptop and PC <-> PC (local PC mode)**: the PC runs the GuardedSleep server binary (or compose bundle) in local mode, which serves the web UI on `http://localhost` and signaling on the LAN; Android devices connect to the PC's LAN address, shown in the pairing QR -- target **beta**
- `[STATUS: REJECTED]` Web client over HTTPS <-> phone listener (self-signed certificate click-through): it would teach users to accept certificate warnings and undermines pairing as the trust anchor ([ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md))
- `[STATUS: IDEA]` Chrome Local Network Access (long-polling `fetch`) for an HTTPS web client to reach a phone; Chromium only, WebSockets not yet covered -- re-check when Chrome covers WebSockets
- `[STATUS: DEFERRED]` Offline iOS (Phase 2, revisit at the iOS gate)
- `[STATUS: DECIDED]` Secure device pairing via QR code -- in MVP
- `[STATUS: DECIDED]` End-to-end encryption (DTLS-SRTP + fingerprint binding) -- in MVP
- `[STATUS: DECIDED]` Automatic reconnection after network interruptions -- **MVP**
- `[STATUS: DECIDED]` Heartbeat interval: 10 seconds default (configurable 5-30s) -- **MVP**
- `[STATUS: DECIDED]` Saved connections for reconnection without QR code -- **MVP**
- `[STATUS: IDEA]` Network resilience: buffer events during brief disconnections
- `[STATUS: IDEA]` LAN <-> Internet automatic handoff (same session: Wi-Fi path fails → **prefer continuing on cellular default route** when OS allows; Wi-Fi viable again → **prefer cheaper/local path** without a user "mode" switch) -- see subsection _Path selection vs explicit LAN mode_ below
- `[STATUS: IDEA]` Signaling server rate limiting (e.g. connections or messages per IP / per room) to mitigate abuse and DoS — needs design (reverse proxy vs. in-process; mobile IP churn)

### Invite link / QR (mobile app — UX phase)

The app implements a **single** invitation QR / deep link on `/pair` with query parameters **`room` + `signal`** (WebSocket URL of the inviting device). This is **meeting-link style** convenience: anyone with the link can join that signaling room; it is **not** the full secure JSON payload (PSK, DTLS fingerprint, nonce, …) described under _QR Code Pairing_ below. That cryptographic payload is planned as an evolution of the **same** one-scan UX.

**Saved connections** persist **`roomId` + `signalingUrl` per entry** so reconnect uses the server from the invite; the **Settings** default signaling URL is **not** overwritten when using an invitation.

## Sub-Features

### QR Code Pairing

> Informed by Technical Architect and Security Architect reviews. See [technical analysis](../technical/analysis.md).

Secure pairing workflow using QR codes for physical key exchange:

1. Device A (already in cluster) generates a time-limited QR code containing:
   - Device ID
   - Pre-Shared Key (PSK) -- cryptographic key material (min. 256-bit entropy)
   - Local IP address (if on same network)
   - Optional: signaling server endpoint (for internet mode)
   - Nonce + timestamp for replay protection
2. Device B scans the QR code
3. **While QR is displayed on Device A**: Device B is auto-accepted (physical proximity = trust)
4. **After QR is dismissed** (screen changed, timeout): any connection attempt shows a join request on Device A: "Device [name] wants to join -- Accept / Reject"
5. WebRTC DataChannel established, authenticated via PSK
6. DTLS certificate fingerprint verified against PSK-derived value (MITM protection) -- **invisible to user, automatic verification**
7. Over the authenticated DataChannel, long-term keys are exchanged
8. Trust established -- no manual fingerprint comparison needed (QR scan = physical verification)

### Adding further parent devices (Phase 2)

> `[STATUS: DECIDED]` -- same UX as the first pairing; implementation in Phase 2 after MVP 1:1 is stable.

- User chooses **Add device** (or equivalent) on a device that is **already in the cluster**.
- That device shows a **time-limited QR** (same payload family as initial pairing: PSK material, IPs, signaling, nonce, etc.).
- The **new** device **scans** the QR — flow matches steps 3–8 above (auto-accept while QR visible; join request after dismiss).
- **Either role may show the QR:** an already-paired **baby** unit or an already-paired **parent** unit can host “Add device”. On the baby unit, **kiosk unlock + explicit confirmation** is required before showing pairing QR (see [device-management.md](device-management.md)).
- **Cap:** at most **4** parent devices per baby (enforced in app; cluster UX must surface the limit clearly).
- **Failover (Phase 2):** when one paired parent device becomes unresponsive (see [failure-model.md](../concept/failure-model.md) F-1a), the baby unit **escalates alerts** to another paired parent so monitoring is not a single-point-of-failure on the parent phone. Engineering: parallel WebRTC parent sessions on the baby unit.

**Payload format:** UTF-8 JSON with **human-readable field names** (`psk`, `dtls_fingerprint`, `ips`, `signal`, `nonce`, `ts`). Size verified to fit standard QR codes; see [technical analysis](../technical/analysis.md) (QR Code IP Discovery).

### Time-limited pairing ("Babysitter mode")

> `[STATUS: IDEA]` -- Phase 2 UX + security design.

For temporary access (babysitter, grandparent overnight), a cluster member may issue a **QR or invite with expiry** (e.g. 6--24 hours). After expiry, the guest device's saved connection is invalidated without manual revoke. Distinct from full remote pairing (still DEFERRED for Phase 1 per table below).

Security requirements:

- `[STATUS: DECIDED]` QR code must be one-time-use with configurable timeout (~30 seconds)
- `[STATUS: DECIDED]` DTLS fingerprint bound to QR pairing key material (MITM protection) -- invisible/automatic
- `[STATUS: DECIDED]` While QR is displayed: auto-accept on scan. After dismissed: join request with device name.
- `[STATUS: IDEA]` Cluster join notification to all existing devices
- `[STATUS: IDEA]` If cluster has >2 devices: second device must confirm the join
- `[STATUS: IDEA]` Audit log of all join/leave events visible in app
- `[STATUS: DECIDED]` Product cap: **max 4 parent devices** per baby (Phase 2); optional stricter server limits are a deployment detail

### Cluster Management

- `[STATUS: DECIDED]` **Phase 2:** Remote device revocation: any cluster member can remove a device; re-keying after removal so lost/stolen devices cannot reconnect with old material ([failure-model.md](../concept/failure-model.md) F-7).
- `[STATUS: DEFERRED]` App lock (PIN/biometrics) before accessing monitoring functions -- Phase 2; MVP uses OS screen lock ([ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md))
- `[STATUS: IDEA]` Key rotation: periodic renewal of cluster root key (e.g., every 30 days) -- distinct from re-key-on-revocation
- `[STATUS: IDEA]` Forward secrecy: ephemeral Diffie-Hellman per connection
- `[STATUS: IDEA]` Key material stored in platform keystore (Android Keystore / iOS Keychain), never in plaintext

### Internet Connectivity (Signaling Server -- Primary)

> STATUS: **DECIDED** -- included in MVP. Mainstream internet mode.

For devices on different networks, a minimal signaling server enables WebRTC connection:

- Minimal WebSocket signaling server (~300-400 LOC production-ready, Docker)
- EU-based STUN servers for NAT traversal (defaults in code: **sipgate.de**, **Linphone** public STUN; no Google STUN)
- Self-hosted coturn recommended for production (STUN + TURN)
- TURN relay as fallback when P2P fails (~10-15% of cases)
- Audio/video flows P2P directly between devices after signaling
- Community-hosted signaling instance available for non-technical users
- QR code contains signaling server URL + all device IPs

Privacy notes:

- Signaling server sees which devices connect + when (metadata only, no content)
- STUN servers see public IPs of both devices + connection times
- Self-hosted signaling + coturn = full privacy control
- Users informed in-app which servers are used and what metadata they see

### Internet Connectivity (VPN -- Optional Power-User Mode)

> STATUS: **DECIDED** -- optional in MVP. For technically advanced users.

[Tailscale](https://tailscale.com/) or [Headscale](https://github.com/juanfont/headscale) (self-hosted) creates a WireGuard VPN mesh that makes devices appear on the same LAN:

- Both devices install Tailscale and join the same tailnet
- Our app treats Tailscale IP like any other local IP -- no code difference
- Double encryption: WireGuard (Tailscale) + DTLS-SRTP (our WebRTC)
- For full privacy: use Headscale (self-hosted, open source)
- No signaling server needed when using VPN (devices are "local")

Note: Android allows only 1 VPN at a time. Users with corporate VPN, ad-blockers, or other VPNs must choose. This is why VPN mode is optional, not primary.

### Local / Hotspot Connectivity

For devices on the same network or ad-hoc connections:

- **No separate signaling service** is required (no Docker, no public server); SDP/ICE exchange uses an **embedded WebSocket listener on the Baby-Android unit** (target **beta**, Kotlin on the Rust `relay` crate per [ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §1; the Capacitor plugin implementation is legacy; see [ADR-012](../decisions/ADR-012-connection-model-offline-lan-and-handoff.md)). The path-selector layer ships now so saved-connection fallbacks already work.
- No STUN/TURN needed on local networks
- User must manually create hotspot via OS settings (no programmatic API available)
- App provides step-by-step hotspot setup guide
- No data leaves the local network = maximum privacy
- UI prominently indicates when connection is local ("Secure local mode") -- copy and timing per [ADR-013](../decisions/ADR-013-connection-status-observability-and-copy.md)

### LAN Docker fallback (power-user)

> STATUS: DOCUMENTED -- complementary to the embedded-listener path, **not** a substitute. See [ADR-012](../decisions/ADR-012-connection-model-offline-lan-and-handoff.md) Option D.

Users with a NAS / router that can run Docker may run the `ghcr.io/<owner>/guardedsleep-signaling` image ([ADR-008](../decisions/ADR-008-container-image-build-and-distribution.md)) on the LAN and add its URL (e.g. `wss://nas.local:8443/`) to **Saved Connections → Fallback signaling URLs** (Advanced). The path-selector will try it after the primary URL fails. This was the only offline-LAN option for PWA/iOS targets (superseded for PCs by the local PC mode, see [ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §1; it remains a power-user option and the same Rust server binary serves it). Browsers over HTTPS cannot reach a phone listener.

### Out-of-Home / Event connectivity (hotels, venues)

> STATUS: DECIDED product intent, **PROPOSED** UX detail -- see [use-cases.md](../concept/use-cases.md) UC-3 and [ADR-009](../decisions/ADR-009-personas-and-failure-model.md).

Event use (Persona A) often hits **captive portals**, **client isolation** on venue WiFi, or **blocked WebSocket / STUN / TURN** ports. The app must not assume "same SSID = same LAN" in those environments.

- **Detection**: after pairing or on reconnect, verify that baby and parent can exchange ICE / DataChannel on the path the user expects; if hotel WiFi blocks P2P, fall back to **cellular + signaling** when both devices have mobile data.
- **User-visible framing**: prefer plain language ("Checking connection…", "This network may block baby monitors -- try mobile data on both phones") over technical mode names.
- **Pre-flight / venue test**: optional guided check before the caregiver leaves the room (see [notifications-alerts.md](notifications-alerts.md) probe / pre-flight).
- **Data honesty**: surface approximate data usage when Internet path is active ([communication.md](communication.md)).

### Path selection vs explicit "LAN mode" (engineering note)

> STATUS: **DECIDED** intent for documentation; implementation details follow PoC.

- **There is no separate user-facing "LAN mode" switch** that must be flipped for efficiency. What users call "LAN" is an **outcome** of WebRTC **ICE**: when both sides expose reachable **host** (or mDNS) candidates on the same layer-2/L3 path, media tends to flow **directly** with lower latency -- the stack "prefers" those candidates when they work.
- The **OS default network** (Wi-Fi vs cellular) chooses which interfaces participate in candidate gathering and which sockets can reach the internet (e.g. **TURN**, **signaling WSS**). That is necessary but **not sufficient** for "always optimal": a phone can be "on Wi-Fi" yet the venue Wi-Fi blocks peer-to-peer; then **cellular** (or TURN) is the only working path even though Wi-Fi is the default route.
- **Automatic Wi-Fi ↔ cellular** for an active session therefore needs **explicit app logic** on mobile: listen for `Network` / capability changes, optionally **bind** or restart ICE when the default network changes, and re-run connectivity checks -- not assuming the browser/OS alone will silently migrate a live WebRTC session in all cases.
- UI copy like **"Secure local mode"** ([Local / Hotspot Connectivity](#local--hotspot-connectivity)) is a **derived status** ("media is not leaving your subnet / no relay in use") for transparency, not a second product mode the user must maintain.

### LAN <-> Internet Handoff

> STATUS: **DECIDED** (this sprint = path-selector + saved-connection fallbacks; Phase 2 = live `@capacitor/network` handoff with hysteresis; superseded: link supervision lives in the Rust core, see [ADR-017](../decisions/ADR-017-link-recovery-and-connection-supervision.md)). Binding spec: [ADR-012](../decisions/ADR-012-connection-model-offline-lan-and-handoff.md).

When the local connection drops (e.g., device leaves WiFi range), the app should automatically attempt reconnection via Tailscale IP (or future signaling server). When both devices are on the same network again, prefer the direct local connection for maximum privacy and minimum latency.

Design requirements (to be considered in Phase 1 architecture):

- Store all known IPs per paired device (local + Tailscale)
- ICE candidate gathering for both local and remote
- Priority: local candidates always preferred over remote
- Seamless switch without user interaction
- Must also work in reverse: Internet -> LAN when local becomes available
- Must handle unstable local networks (frequent WiFi drops)
- **Cellular as backup to failing Wi-Fi:** when the active Wi-Fi path cannot complete ICE / heartbeat but **cellular is available and user policy allows metered use**, attempt continuation on the **default unmetered-or-user-consented** route (implementation: Android network APIs + ICE restart / controlled reconnect; see [communication.md](communication.md) for honesty on data volume)

Phase 1 provides all three paths (local + signaling + VPN). Switching is manual. Automatic handoff is a future feature.

### IP Sync Between Connected Devices

> STATUS: IDEA

Once devices are connected, they periodically exchange their current IP lists via DataChannel:

- Triggered on network change (WiFi connected/disconnected, VPN up/down) and on a timer
- Keeps stored IPs current -- enables fast reconnection if the active path fails
- Security: use IP type labels (local/vpn/hotspot) instead of full IPs where possible (data minimization per Security Architect recommendation)
- Stored IPs have short TTL and are not logged persistently

### Connection Quality Monitoring

> STATUS: IDEA

Regularly check connection quality across all available paths:

- Measure latency (RTT via DataChannel ping), stability (packet loss), throughput
- Prefer the path with lowest latency and highest reliability
- Could enable automatic path switching (future: handoff feature)
- Display connection quality indicator in parent UI (good/medium/poor)

### Saved Connections / Connection History -- MVP

> STATUS: DECIDED -- Moved to MVP. Low effort, high UX impact.

Store recent successful connections on both devices for reconnection without QR code:

- After successful QR pairing, connection data (PSK, device name, known IPs, signaling URL) is persisted
- PSK stored in Android Keystore (TEE) -- not extractable even with root access
- Device names and IPs in Encrypted SharedPreferences
- On app restart, show list of saved connections: "Reconnect to [device name]"
- No QR code needed for reconnection -- just tap the saved connection
- **One-tap cold reconnect (parent device):** after OS reboot, app kill, or battery drain on the **parent** phone, unlocking the device and tapping the saved connection must re-establish monitoring **without** walking to the baby unit and **without** re-scanning QR, using persisted keystore-backed credentials ([failure-model.md](../concept/failure-model.md)).
- **Cold reconnect timeout:** same as below (15 s) unless user cancels; on failure show guided troubleshooting (not raw errors).
- Max stored connections: 5 (realistic: 2-4 devices per family)
- Auto-delete after 90 days of inactivity (no successful connect), with 7-day warning
- Editable names (default: device model + role, e.g., "Samsung A53 (Baby)")
- Swipe to delete individual connections
- "Delete all connections & keys" panic button in settings
- ~~Reconnection timeout: 15s, then "Device not reachable. Scan QR code again?"~~ superseded: the link never gives up while monitoring and shows a troubleshooter from escalation level 2 ([`link-supervision`](../../openspec/specs/link-supervision/spec.md))

**Mode persistence**: App remembers last role + one-tap confirmation on start:

- First start: full role selection (two large buttons)
- Subsequent starts: "Start as [last role]?" with large confirm button + small "Change role" link
- Prevents accidental wrong-mode start at 3 AM

**Mode switching**: A device can change its role:

- Mode switch requires explicit local confirmation dialog
- Switch to baby mode: warning "Microphone will be activated and audio transmitted to connected parent devices. Continue?"
- Switch from baby to parent: immediate (safer state, no warning needed)
- Mode switch can NEVER be triggered remotely (security requirement)
- Permanent microphone indicator in notification bar when in baby mode

### Network Resilience

> STATUS: DECIDED -- moved to [`openspec/specs/link-supervision`](../../openspec/specs/link-supervision/spec.md) (recovery by a new attempt, missed-event buffer, link states). ICE restart stays `IDEA`.

### Heartbeat -- MVP

> STATUS: DECIDED -- moved to [`openspec/specs/link-supervision`](../../openspec/specs/link-supervision/spec.md): both sides send it, 10 s default, 5-15 s, loss after `2 × interval + 5 s`. The earlier "5-30 s, 2 missed beats" wording was superseded by ADR-017 §5.

## Dependencies

- [Communication](communication.md) -- uses the established connection for data transmission
- [Notifications](notifications-alerts.md) -- connection-lost alerts
- [Technical Analysis](../technical/analysis.md) -- platform and encryption decisions

## Open Questions

- ~~Which web framework (Svelte, React, Vue) for the Capacitor app?~~ **RESOLVED, superseded**: Svelte 5; the Capacitor app is legacy and the web client is rebuilt on Vite 8 + Svelte 5 ([ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §5)
- Exact QR code data format and encoding (binary vs. base64)?
- How to handle re-pairing if a device is replaced?
- ~~Optimal heartbeat interval (balance responsiveness vs. battery)?~~ **RESOLVED**: 10 s default, 5-15 s ([`link-supervision`](../../openspec/specs/link-supervision/spec.md))
- Should the app guide users through hotspot setup with OS-specific screenshots?

## Discarded Alternatives

| Alternative                         | Reason for Rejection                                                                                                 |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Manual IP address entry for pairing | Poor UX, error-prone, exposes technical details                                                                      |
| Cloud-based device discovery        | Contradicts privacy-first principle; adds cloud dependency                                                           |
| Programmatic hotspot creation       | Not possible on mobile OS due to security restrictions                                                               |
| Full SDP in QR code (uncompressed)  | SDP is ~2500 bytes, exceeds QR code capacity; QWBP compression deferred to future optimization                       |
| Tailscale-only internet mode        | UX dealbreaker: VPN slot conflict, complex setup (~6 extra steps), privacy contradiction. Critical Reviewer verdict. |
| Embedded WireGuard in app           | VPN slot conflict remains; ~1000+ LOC native code; disproportionate complexity. All 3 experts rejected.              |
