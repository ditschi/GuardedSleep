# Technical Analysis

This document records the technical analysis that informed the architecture. **Authoritative MVP decisions:** [Architecture Decision Records](../decisions/README.md). If this document and an ADR disagree, **the ADR wins**.

> **Partially superseded (2026-10-20):** the platform analysis recommended the Hybrid Web App + Capacitor; that was replaced by native clients on a shared Rust core ([ADR-015](../decisions/ADR-015-native-clients-with-shared-rust-core.md), [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md)). Distribution wording on F-Droid is superseded by [ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §3 (Play and GitHub APK at alpha, IzzyOnDroid at beta, F-Droid main repository `DEFERRED`). The Capacitor sections stay as decision history.

## Analysis Topics

| #   | Topic                            | Status                   | ADR                                                                                                          |
| --- | -------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------ |
| 1   | App Platform                     | DECIDED                  | [ADR-002](../decisions/ADR-002-client-platform-and-repository-structure.md)                                  |
| 2   | Communication Architecture       | DECIDED                  | [ADR-003](../decisions/ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md)                              |
| 3   | Server Requirements              | DECIDED                  | [ADR-003](../decisions/ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md)                              |
| 4   | Encryption Approach              | DECIDED                  | [ADR-004](../decisions/ADR-004-encryption-transport-and-key-storage.md)                                      |
| 5   | Audio Processing                 | DECIDED (features + PoC) | [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md) |
| 6   | Deployment Model                 | DECIDED                  | [ADR-005](../decisions/ADR-005-parent-alerts-product-naming-and-distribution.md)                             |
| --  | MVP scope, PoC gate, in-app lock | DECIDED                  | [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md) |

---

## 1. App Platform

> STATUS: DECIDED -- See [ADR-002](../decisions/ADR-002-client-platform-and-repository-structure.md).

**Question**: What technology do we use to build the app?

### Research Findings

**iOS PWA Background Audio is a BLOCKER**: When a PWA is minimized or the screen is locked on iOS, audio is paused. iOS has no Background Sync, no Periodic Background Sync, no Background Fetch for PWAs. Service Workers are aggressively killed. Workarounds (NoSleep.js, minimal-ui) are unreliable. A baby unit MUST continuously monitor audio in the background -- this makes pure PWA unviable for the baby unit on iOS.

**Capacitor solves the PWA limitations**: Capacitor wraps a web app in a native container (WebView) with access to native APIs including background audio, push notifications, and camera. Same web codebase works across all platforms. iOS builds still require Xcode on macOS, but GitHub Actions provides free macOS runners for open source projects.

**No Apple hardware constraint**: Every approach that produces native iOS code (Capacitor, React Native, Flutter, native Swift) requires Xcode on macOS. For open source projects, GitHub Actions macOS runners are free. Community contributors with Macs can assist with testing.

**Local network / hotspot scenario**: A pure PWA served locally faces HTTPS requirements for Service Workers (no valid TLS cert for local IPs like 192.168.x.x). With Capacitor, the app is already installed -- no web server needed, no HTTPS certificate issues. This is a strong argument FOR Capacitor.

### Options

- [ ] **Progressive Web App (PWA)** -- Pure browser-based
  - Single codebase, runs in browser on all platforms
  - Supports microphone, camera, push notifications
  - **BLOCKER**: iOS kills background audio when PWA is minimized/screen locked
  - No app store presence
  - HTTPS requirement problematic for local network scenarios
  - **Technical Architect verdict: NOT VIABLE** for baby unit on iOS

- [ ] **Native (Kotlin/Swift)** -- Separate codebases
  - Best platform integration, full API access
  - Separate codebases for Android and iOS = double maintenance
  - No iOS testing possible without Apple hardware
  - Highest development effort for a single developer
  - **Technical Architect verdict: TOO EXPENSIVE** for project constraints

- [ ] **Cross-Platform Framework (Flutter/React Native)**
  - Single codebase compiling to native
  - Good platform API access including background execution
  - Still requires Apple hardware for iOS builds/testing
  - Web app fallback possible but not primary
  - Loses the "also works directly in browser" advantage
  - **Technical Architect verdict: VIABLE** but loses web fallback

- [x] **Hybrid: Web App + [Capacitor](https://capacitorjs.com/) (RECOMMENDED)**
  - Web framework (Svelte/React/Vue) for core UI and logic
  - Capacitor wraps into native Android + iOS apps (both platforms from one codebase)
  - PWA export as desktop/browser fallback
  - Native plugins for background audio, push notifications, camera
  - Same codebase serves all platforms
  - GitHub Actions free macOS runners for iOS builds
  - Clean architecture: Core logic (pure JS/TS) -> Platform layer (Web APIs / Capacitor plugins) -> UI layer
  - **Technical Architect verdict: RECOMMENDED** -- only approach meeting all constraints

**Recommendation (superseded, see ADR-015/ADR-020)**: Hybrid Web App + Capacitor. It is the only approach that simultaneously provides cross-platform support, solves the iOS background audio blocker, allows a browser fallback for desktop, works in local/hotspot scenarios without an HTTPS certificate, and is maintainable by a single developer.

### Discarded Alternatives

| Option              | Reason                                                             |
| ------------------- | ------------------------------------------------------------------ |
| Pure PWA            | iOS background audio is a hard blocker for baby unit functionality |
| Native Kotlin/Swift | Double codebase maintenance not feasible for single developer      |
| Tauri Mobile        | Pre-1.0 for mobile, insufficient maturity                          |

---

## 2. Communication Architecture

> STATUS: DECIDED -- See [ADR-003](../decisions/ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md).

**Question**: How do devices communicate with each other?

### Research Findings

**WebRTC without signaling server is possible**: The QWBP protocol compresses WebRTC SDP from ~2500 bytes to ~55-100 bytes, fitting in a single QR code. On local networks, no STUN/TURN is needed -- browsers generate local ICE candidates directly. Over the internet, STUN is still needed for NAT traversal.

**DataChannel-first architecture**: The feature design (heartbeat + alerts as default, audio only on-demand) maps perfectly to WebRTC DataChannel as baseline with optional MediaStream upgrade. Permanent DataChannel for heartbeat/alerts, MediaStream added only when parent activates listen mode or talk-back.

**Two-stage QR bootstrap**: QR code establishes a minimal DataChannel, then full audio/video capabilities are negotiated over that secure channel. This avoids needing to fit full SDP in the QR code.

### Options

- [x] **WebRTC P2P (RECOMMENDED)**
  - Direct device-to-device communication
  - Built-in encryption (DTLS-SRTP) for all media
  - Built-in audio/video codec support (Opus, VP8/VP9)
  - DataChannel for lightweight messages (heartbeat, alerts, control)
  - MediaStream for audio/video (added on-demand)
  - Signaling can be done via QR code (local) or minimal server (internet)
  - NAT traversal via STUN/TURN (only for internet scenarios)
  - **Technical Architect verdict: RECOMMENDED** -- brings codecs, crypto, and NAT traversal for free

- [ ] **WebSocket (Client-Server)**
  - All communication flows through a central server
  - Server sees all data unless additional E2E encryption added
  - Server is single point of failure and bandwidth bottleneck
  - Contradicts "no mandatory server" and privacy goals
  - **Technical Architect verdict: NOT RECOMMENDED** as primary

- [ ] **Custom P2P Protocol**
  - Full control but must solve NAT traversal, encryption, codecs from scratch
  - Massive effort for a single developer
  - **Technical Architect verdict: NOT VIABLE** given constraints

- [ ] **Hybrid (WebRTC + WebSocket fallback)**
  - WebRTC for media, WebSocket for signaling
  - This is actually what the recommended WebRTC approach includes: WebSocket is used as the signaling channel for internet scenarios
  - **Technical Architect verdict: THIS IS the WebRTC approach** -- not a separate option

**Recommendation**: WebRTC with DataChannel-first architecture. DataChannel is the permanent baseline connection for heartbeat, alerts, and control messages. MediaStream is added on-demand only when parent activates listen mode or talk-back. This is data-efficient and battery-friendly.

### Discarded Alternatives

| Option                       | Reason                                                         |
| ---------------------------- | -------------------------------------------------------------- |
| Pure WebSocket client-server | Contradicts privacy-first and no-mandatory-server goals        |
| Custom P2P protocol          | Unrealistic effort; WebRTC solves codecs, crypto, NAT for free |

---

## 3. Server & Connectivity Architecture

> STATUS: **DECIDED** -- [ADR-003](../decisions/ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md). Three connectivity paths; reviewed by Technical Architect, Security Architect, and Critical Reviewer.

**Decision**: MVP supports 3 connectivity paths. All paths use the same app code -- the app just connects to IPs.

### Decided Architecture: Three Connectivity Paths

**Path 1: Local (WiFi / Hotspot)** -- No server, no internet needed

- Direct WebRTC between devices on same network
- QR pairing establishes connection
- Maximum privacy, lowest latency

**Path 2: Internet (Signaling Server)** -- For mainstream users

- Minimal self-hosted WebSocket signaling server (~300-400 LOC, Docker)
- EU-based STUN servers for NAT traversal
- Optional self-hosted TURN relay (coturn) for ~10-15% of cases where P2P fails
- Community-hosted signaling instance available for non-technical users
- User-friendly: works out-of-the-box, no extra apps needed

**Path 3: Internet (VPN -- Tailscale/Headscale)** -- Optional power-user mode

- External VPN app simulates LAN over the internet
- Our app doesn't know about Tailscale -- it just sees another IP
- Double encryption: WireGuard + DTLS-SRTP
- For technically advanced users or maximum privacy (with Headscale)

**Key principle**: Our app logic is identical for all three paths. It connects to IPs. Whether that IP is local WiFi (192.168.x.x), reached via signaling+STUN, or from a VPN tunnel (100.x.x.x) is transparent. The QR code and IP sync mechanism handle discovery.

### Why Three Paths? (Expert Review Summary)

The Critical Reviewer identified that Tailscale-only internet mode was a UX dealbreaker:

- ~5-10% of target users (parents) can set up Tailscale
- Android VPN slot conflict (only 1 VPN at a time -- blocks ad-blockers, corporate VPN)
- Tailscale disconnect causes silent monitoring failure
- 300-400 LOC signaling server is less total complexity than every user configuring a VPN

All three experts agreed: Signaling server for mainstream users + Tailscale as optional bonus.

### QR Code IP Discovery

The QR code includes all active network interfaces + optional signaling server:

```json
{
  "psk": "base64-psk-44-chars",
  "dtls_fingerprint": "hex-dtls-fingerprint-64-chars",
  "ips": ["192.168.1.42", "100.64.0.5", "10.0.0.15"],
  "signal": "wss://signal.example.com",
  "nonce": "base64-nonce-24-chars",
  "ts": 1744700000
}
```

**QR Code Payload Size Analysis (VERIFIED, human-readable keys):**

Measured with representative values (256-bit PSK as Base64, SHA-256 fingerprint as 64 hex chars, 3 typical IPs, example signaling URL, 128-bit nonce as Base64, Unix timestamp):

| Scenario                           | Payload (bytes) | QR Version (EC-M)    | Modules | Fits?                    |
| ---------------------------------- | --------------- | -------------------- | ------- | ------------------------ |
| **Local only** (no `signal` field) | **239**         | Version 12 (287 cap) | 65x65   | **YES**                  |
| **With signaling** (example URL)   | **284**         | Version 13 (331 cap) | 69x69   | **YES**                  |
| **+ margin** (4 IPs, longer URL)   | ~310--320 est.  | Version 13 (331 cap) | 69x69   | **YES** (stay under 331) |

**Result: QR code fits comfortably with readable field names.** Error Correction M (recommended for reliable phone scanning) is sufficient.

**Optimizations that remain:**

- `dtls_fingerprint` as **64 hex characters** (no colons) -- same information as colon-separated, smaller payload
- `signal` omitted when using local-only pairing
- If a future payload ever approaches the limit: shorten URL, fewer IPs in QR, or CBOR/MessagePack (last resort)

**Recommendation:** Human-readable JSON keys (`psk`, `dtls_fingerprint`, `ips`, `signal`, `nonce`, `ts`) for debugging and interoperability. Error Correction M.

- App enumerates IPs via `NetworkInterface.getNetworkInterfaces()` (includes WiFi, VPN, Hotspot)
- `signal` field is optional -- only present if signaling server is configured
- Filter: exclude link-local (169.254.x.x), Docker bridge (172.17.x.x), loopback

**Connection algorithm**: Try all IPs in parallel + signaling server. First successful WebRTC handshake (verified via PSK + DTLS fingerprint) wins. Local IPs naturally respond faster -> preferred.

### Reconnection & IP Sync

1. App stores PSK + last-known IPs + signaling server URL + device ID
2. On reconnect: try all stored IPs in parallel + signaling server + mDNS/UDP broadcast
3. First successful connection wins
4. **IP sync (IDEA)**: Once connected, devices periodically exchange current IP lists via DataChannel. This keeps stored IPs fresh and enables fast reconnection if one IP becomes unreachable.
5. **Connection quality monitoring (IDEA)**: Periodically check latency/reliability of all available paths. Prefer the best path (lowest latency, most stable).

### EU/GDPR-Friendly STUN Servers

Used by Path 2 (Signaling Server) for NAT traversal:

| Server                    | Location    | Provider    |
| ------------------------- | ----------- | ----------- |
| `stun.nextcloud.com:443`  | Germany     | Nextcloud   |
| `stun.1und1.de:3478`      | Germany     | 1&1         |
| `stun.gmx.de:3478`        | Germany     | GMX         |
| `stun.hosteurope.de:3478` | Germany     | Host Europe |
| `stun.linphone.org:3478`  | France      | Linphone    |
| `stun.xs4all.nl:3478`     | Netherlands | XS4ALL      |

Self-hosted coturn recommended for production. EU STUN servers as fallback. Users informed in-app which servers are used and what metadata is visible (public IPs + connection times).

### Security Architect Notes

- Path 1 (Local): Privacy gold standard. No metadata leaks.
- Path 2 (Signaling): Signaling server sees which devices connect + when. STUN sees public IPs. All self-hostable.
- Path 3 (VPN): Tailscale sees online status + connection times. Content double-encrypted. Headscale = full control.
- DTLS-SRTP + Fingerprint Binding must NEVER be weakened, even with VPN double encryption.
- IP sync via DataChannel: use IP types (local/vpn/hotspot) instead of full IPs where possible (data minimization).
- QR code warning in UI: "Show this code only to the parent device. Never photograph or share."

### Discarded Alternatives

| Option                            | Reason                                                                                      |
| --------------------------------- | ------------------------------------------------------------------------------------------- |
| Full relay server                 | Privacy concern from metadata exposure; bandwidth bottleneck                                |
| Cloud-hosted signaling as default | Contradicts self-hosting and privacy-first principles                                       |
| Google STUN as default            | Replaced by EU-based alternatives for GDPR compliance                                       |
| Tailscale-only internet mode      | UX dealbreaker: VPN slot conflict, complex setup, privacy contradiction (Critical Reviewer) |
| Embedded WireGuard in app         | VPN slot conflict remains; ~1000+ LOC native code; disproportionate effort (all 3 experts)  |

---

## 4. Encryption Approach

> STATUS: **DECIDED** -- [ADR-004](../decisions/ADR-004-encryption-transport-and-key-storage.md). Three-layer model; Layer 3 application E2E in Phase 2.

**Decision**: DTLS-SRTP + Fingerprint Binding + Application E2E Layer. This covers ALL transmission types: audio, video, data, and control messages.

### Decided Architecture

- **Layer 1: WebRTC DTLS-SRTP** (transport encryption, automatic)
  - Covers: Audio/video streams (via SRTP) + DataChannel (via SCTP over DTLS)
  - Standard, battle-tested, comes free with WebRTC
- **Layer 2: DTLS fingerprint binding** to QR pairing key material (MITM protection)
  - Ensures the DTLS connection is to the trusted paired device, not an attacker
- **Layer 3: Application-level E2E** with keys derived from pairing secret (defense-in-depth)
  - Protects against compromised WebRTC stack
  - Phase 2 implementation (DTLS + fingerprint binding is sufficient for Phase 1)
- **Signaling channel**: TLS/WSS for the signaling WebSocket server (internet mode)
- ICE credentials derived via HKDF for per-session uniqueness
- Forward secrecy via ephemeral Diffie-Hellman per connection
- Key material stored in platform keystore (Android Keystore, iOS Keychain)

### Security Architect Clarification: Noise Protocol and TLS/mTLS

**Noise Protocol Framework**: Not needed as a replacement -- DTLS-SRTP already covers media encryption. Could theoretically be used as the Layer 3 application E2E, but adds implementation complexity without clear benefit over simpler AES-GCM with HKDF-derived keys. **Status: REJECTED** for this project.

**TLS/mTLS**: Still relevant but NOT for media encryption. TLS is used for:

- Signaling server connections (WSS = WebSocket over TLS)
- Any future REST API endpoints
- TURN over TLS
  TLS does NOT replace DTLS-SRTP for audio/video. **Status: APPLICABLE for signaling, not for media.**

### How Encryption Covers Each Data Type

| Data Type                  | Transport                 | Encryption                      | Layer     |
| -------------------------- | ------------------------- | ------------------------------- | --------- |
| Audio stream (listen mode) | WebRTC MediaStream (RTP)  | SRTP (from DTLS session)        | Layer 1   |
| Video stream (future)      | WebRTC MediaStream (RTP)  | SRTP (from DTLS session)        | Layer 1   |
| Heartbeat, alerts, control | WebRTC DataChannel (SCTP) | DTLS                            | Layer 1   |
| All of the above           | --                        | DTLS fingerprint pinned to PSK  | Layer 2   |
| All of the above (Phase 2) | --                        | Application E2E (HKDF from PSK) | Layer 3   |
| Signaling (internet mode)  | WebSocket                 | TLS (WSS)                       | Transport |

### Discarded Alternatives

| Option                                       | Reason                                                          |
| -------------------------------------------- | --------------------------------------------------------------- |
| DTLS-SRTP only (without fingerprint binding) | MITM attack possible during signaling                           |
| Noise Protocol Framework                     | Redundant with DTLS-SRTP; adds complexity without clear benefit |
| TLS/mTLS as media encryption                 | Does not apply to WebRTC media streams; used only for signaling |
| Custom cryptographic protocol                | Never roll your own crypto; use established primitives          |

### Security Requirements (from Security Architect review)

| Requirement                                                | Priority   | Status                                                                                                                                                                      |
| ---------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DTLS fingerprint bound to QR pairing key (MITM protection) | CRITICAL   | Must be designed                                                                                                                                                            |
| Threat model formally documented                           | CRITICAL   | Must be created                                                                                                                                                             |
| Cluster join with confirmation mechanism                   | CRITICAL   | Must be designed                                                                                                                                                            |
| Additional E2E encryption layer (defense-in-depth)         | IMPORTANT  | Strongly recommended                                                                                                                                                        |
| Key rotation and forward secrecy                           | IMPORTANT  | Must be designed                                                                                                                                                            |
| Remote device revocation with re-keying                    | IMPORTANT  | Must be designed                                                                                                                                                            |
| STUN/TURN privacy implications documented                  | IMPORTANT  | Must be documented                                                                                                                                                          |
| In-app device lock (PIN/biometrics) before app access      | IMPORTANT  | **DEFERRED (Phase 2)** -- MVP relies on **OS screen lock** per [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md) |
| Key material in platform keystore, never in plaintext      | IMPORTANT  | Must be implemented                                                                                                                                                         |
| QR code one-time-use with timeout (~30s)                   | IMPORTANT  | Must be designed                                                                                                                                                            |
| Replay protection (nonces, timestamps in QR protocol)      | SUGGESTION | Should be implemented                                                                                                                                                       |

### Threat Model (Security Architect assessment)

| Adversary                         | Capability                                          | Motivation                   | Priority            |
| --------------------------------- | --------------------------------------------------- | ---------------------------- | ------------------- |
| Random internet attacker / botnet | Port scans, known exploits                          | Voyeurism, camera/mic access | HIGH                |
| Malicious WiFi neighbor           | Local network sniffing, ARP spoofing                | Curiosity, harassment        | MEDIUM              |
| Abusive ex-partner / stalker      | Knowledge of setup, possibly former physical access | Surveillance, control        | HIGH                |
| Compromised infrastructure        | STUN/TURN operator, ISP                             | Mass surveillance, metadata  | MEDIUM              |
| Malware on device                 | Full device access                                  | Data theft                   | HIGH (out of scope) |

### Discarded Alternatives

| Option                                       | Reason                                                 |
| -------------------------------------------- | ------------------------------------------------------ |
| DTLS-SRTP only (without fingerprint binding) | MITM attack possible during signaling                  |
| Custom cryptographic protocol                | Never roll your own crypto; use established primitives |

---

## 5. Audio Processing

> STATUS: DECIDED (baseline) -- Requirements in [audio-monitoring.md](../features/audio-monitoring.md); Opus DTX / capture behavior validated in PoC per [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md).

**Question**: How do we capture and process audio on the baby unit?

### Topics to Analyze

- Audio capture APIs per platform:
  - Web: Web Audio API + getUserMedia
  - Capacitor: can use Web Audio API or native plugins
  - Background audio: Capacitor native plugin (foreground service on Android, audio background mode on iOS)
- Noise level detection: analyze audio buffer RMS/peak amplitude
- Sampling rate trade-offs: 8kHz (voice sufficient) vs. 16kHz (better quality) vs. 44.1kHz (overkill)
- Battery impact: lower sampling rate + larger buffer size = less CPU wake-ups
- WebRTC audio codec: Opus (required by WebRTC spec, excellent for voice, adaptive bitrate)

### Architecture Sketch

```
Microphone → Audio Capture (Web Audio API / native plugin)
           → Audio Analyzer (RMS/peak detection, runs continuously)
           → If threshold exceeded:
              → Send alert via DataChannel (lightweight message)
              → If parent activates listen mode:
                 → Add MediaStream to WebRTC connection (Opus codec)
           → If below threshold:
              → Continue monitoring (low CPU, no transmission)
```

---

## 6. Deployment Model

> STATUS: DECIDED -- See [ADR-005](../decisions/ADR-005-parent-alerts-product-naming-and-distribution.md) (naming, stores); options below remain as reference.

**Question**: How do users set up and access the system?

### Options

- [ ] **App store distribution (Android)**
  - Google Play for mainstream users
  - F-Droid for privacy-conscious users (superseded, see ADR-021 §3: IzzyOnDroid at beta, F-Droid main deferred)
  - APK direct download as alternative
  - No Apple hardware = no iOS App Store initially (community can assist)

- [ ] **Self-hosted server + Docker**
  - For the signaling server (internet mode only)
  - Docker Compose with signaling server + optional coturn (STUN/TURN)
  - Technical users can deploy easily
  - Not needed for local-only use

- [ ] **Hosted community signaling server**
  - Community maintains a signaling server for users who can't self-host
  - Lowest barrier for internet mode
  - Trust/sustainability question
  - Server sees no content, only connection metadata

- [x] **Hybrid: multiple deployment options (RECOMMENDED)**
  - App: Google Play + F-Droid (superseded, see ADR-021 §3) + APK download (Android); iOS via community/TestFlight initially
  - Local mode: no server needed, install app + QR pair
  - Internet mode: self-host signaling (Docker) or use community server
  - Desktop: PWA export accessible via browser
  - Progressive complexity: simplest setup (local) requires zero infrastructure

**Recommendation**: Progressive deployment model. Local mode is zero-infrastructure (install app, create hotspot, QR pair). Internet mode adds a self-hosted or community signaling server. Desktop access via PWA in browser.

---

## 7. Critical Review Findings

> Added after Critical Reviewer challenged the architecture, followed by Technical Architect and UX Expert refinements.

### Validated Risks

| #   | Risk                                                | Severity      | Mitigation                                                                |
| --- | --------------------------------------------------- | ------------- | ------------------------------------------------------------------------- |
| 1   | WebRTC in Capacitor WebView is unvalidated          | **BLOCKER**   | PoC required before any ADR. Go/No-Go gate.                               |
| 2   | iOS background mic capture is unproven in Capacitor | **BLOCKER**   | Needs custom native plugin. iOS deferred to Phase 2.                      |
| 3   | MVP scope is unrealistic for single developer       | **CRITICAL**  | Radical scope reduction (see MVP definition below)                        |
| 4   | QR pairing excludes remote setup scenario           | **IMPORTANT** | Physical pairing for v1; remote pairing via invite code in v2             |
| 5   | WebRTC reconnection on mobile is fragile            | **IMPORTANT** | Local Discovery mechanism (mDNS/UDP broadcast) for reconnection           |
| 6   | Battery impact is unquantified                      | **IMPORTANT** | Target: 8h screen-off. Measured in PoC. Charging recommended for night.   |
| 7   | Signaling server underestimated (~100 LOC)          | **IMPORTANT** | Corrected to ~300-400 LOC production-ready. Evaluate peerjs-server first. |
| 8   | Setup flow has 8-10 steps, not "straightforward"    | **IMPORTANT** | Reduced to 3 steps (see UX section below)                                 |
| 9   | No competitive analysis exists                      | **IMPORTANT** | Must analyze: AI Baby Monitor (OSS), Baby Monitor 3G, Dormi               |
| 10  | iOS is effectively Tier 2, not equal to Android     | **IMPORTANT** | Define platform tiers honestly in docs                                    |

### Architecture Corrections

**Signaling Server**: Corrected from "~100 LOC" to ~300-400 LOC for production-ready (core relay + auth via cluster token + rate limiting + session management). TLS handled by reverse proxy (nginx/Caddy). Evaluate existing solutions (peerjs-server) before building custom.

**Local Reconnection**: Added Local Discovery mechanism. Baby unit sends periodic encrypted beacon (mDNS/DNS-SD or UDP broadcast). Parent unit detects beacon and initiates ICE Restart using persisted keys. No re-pairing needed.

**Audio Latency**: Pre-negotiation strategy. Add a muted audio track during initial connection setup. When parent taps "listen", only `track.enabled = true` -- no renegotiation needed. Latency drops from ~200-500ms to ~50-100ms.

**Battery Target**: Baby unit must last at least 8 hours in monitoring mode (screen off, no streaming) on a 4000mAh battery. Charging recommended for overnight use. Screen MUST be off in baby mode.

### UX Corrections

**Setup reduced to 3 steps** (from 8-10):

1. Install app on both devices
2. Choose role (two large buttons: "Baby Unit" / "Parent Unit")
3. Scan QR code (baby device shows, parent device scans)

Everything else happens automatically: network detection, DTLS binding, permission prompts (contextual).

**"Same WiFi" as default mode** -- not hotspot. Most parents set up at home where WiFi exists. Hotspot is a guided fallback only when no shared WiFi is detected.

**No mode selection** -- the app detects the connection situation automatically. User sees only: connected / connecting / disconnected. A privacy shield indicator (high/medium/low) replaces technical mode names.

**Fingerprint verification is automatic** -- DTLS fingerprint binding to QR material makes manual verification unnecessary. User sees "Connection secured" checkmark. Manual fingerprint display available as expert option in settings only.

**Error messages are actionable** -- every error answers: "What happened? Is my child safe? What should I do?" Escalating alarm system for connection loss (0-30s silent, 30s-2min gentle, 2-5min urgent, >5min full alarm). Jargon banned from UI (no "WebRTC", "DTLS", "ICE", "DataChannel").

### Platform Tiers

| Tier | Platform                                 | Support Level                                      |
| ---- | ---------------------------------------- | -------------------------------------------------- |
| 1    | Android (Capacitor; superseded, ADR-015) | Fully supported by core developer                  |
| 2    | iOS (Capacitor; superseded, ADR-015)     | Community-maintained, CI builds via GitHub Actions |
| 3    | Desktop/Browser (PWA)                    | Best-effort, browser fallback                      |

---

## 8. MVP Definition (Phase 1)

> Informed by Critical Reviewer + Technical Architect + Product Owner perspectives. **Scope:** full documented MVP (no deliberate reduction) per [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md). Effort risk remains; PoC reduces platform risk first.

### PREREQUISITE: Proof of Concept

**Full definition, Go/No-Go criteria, and rationale (PoC vs straight-to-MVP):** [ADR-001: MVP Scope, PoC Gate, and Authentication Strategy](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md).

Before any implementation, validate (minimum checklist):

- [ ] WebRTC DataChannel + MediaStream works in Capacitor Android WebView
- [ ] Audio capture continues in background (Android Foreground Service)
- [ ] Battery consumption measured over 8 hours (screen off, monitoring mode)
- [ ] Reconnection after WiFi interruption works with persisted keys

**This is a Go/No-Go gate. If the PoC fails, the platform choice must be reconsidered.**

### MVP Scope ("It Works")

```
GuardedSleep Phase 1:
- Android only (Capacitor)
- 1 Baby device + 1 Parent device
- Local WiFi/Hotspot + Internet via signaling server + EU STUN
- Optional: Tailscale/Headscale as power-user alternative (external VPN)
- QR code pairing with PSK + DTLS fingerprint binding + ALL device IPs
  - While QR is displayed: auto-accept on scan
  - After QR dismissed: join request with device name (accept/reject)
  - Auto-detection: tries all IPs in parallel, fastest wins
- WebRTC DataChannel for heartbeat + alerts
- Noise detection (RMS threshold, configurable)
- Alert-triggered audio: threshold exceeded -> auto-stream to parent
- "Listen" button -> manual audio stream from baby to parent
  - Pre-negotiated muted track with Opus DTX (~0.2-0.5 MB/h)
  - On activation: track.enabled = true (~50-100ms latency)
- Android Foreground Service for background execution
- Local Discovery for automatic reconnection (mDNS/UDP broadcast)
- Setup in 3 steps: install, choose role, scan QR

UX Quick Wins (included in MVP):
- Baby battery level shown on parent device (via DataChannel heartbeat)
- "All quiet since..." indicator with timestamp of last noise event
- Connection quality indicator (good/medium/poor from WebRTC stats)
- Deep-link to Android battery optimization settings (OEM-specific)
- Alarm snooze/mute button (monitoring continues, notifications paused)
- Minimal kiosk/screen-lock on baby unit (prevent accidental taps)
- Vibration alert option (for silent/night mode on parent device)
- Screen wake lock in listen mode (optional, for party/event use)

Parent UI:
- Night mode (dark, minimal brightness)
- Noise level bars (volume visualization, visible from afar)
- Visual alert indicator (pulsing/blinking when noise detected)
- Connection status + quality indicator
- Baby battery level
- "All quiet since..." display
- Large "Listen" toggle button
- Snooze/mute button
```

### Repository implementation status (snapshot)

> This section tracks **code vs documented MVP** so scope drift is visible in-repo (not only in chat). Update when major pieces land.

As of the current tree:

- **WebRTC:** [`apps/mobile/src/lib/webrtc/peer-session.ts`](../../apps/mobile/src/lib/webrtc/peer-session.ts) — baby runs `getUserMedia` (audio), adds a **muted-by-default** outbound track (`track.enabled` gates alert + listen), parent plays via `ontrack`; **DataChannel** carries heartbeat (+ optional baby battery / RMS), `noise_alert`, and parent `listen` commands. Protocol helpers: [`packages/protocol/src/datachannel.ts`](../../packages/protocol/src/datachannel.ts), extended heartbeat in [`heartbeat.ts`](../../packages/protocol/src/heartbeat.ts).
- **Noise / RMS:** [`apps/mobile/src/lib/audio/noise-monitor.ts`](../../apps/mobile/src/lib/audio/noise-monitor.ts) gates sustained loud noise on the baby unit and triggers alert streaming + DC notify.
- **Pairing UX:** compact **JSON QR** (`v1`: room, signaling URL, PSK) in [`apps/mobile/src/routes/pair/+page.svelte`](../../apps/mobile/src/routes/pair/+page.svelte); legacy `https` invite URLs still parse via [`apps/mobile/src/lib/invite-url.ts`](../../apps/mobile/src/lib/invite-url.ts). **PSK + DTLS:** after both SDPs are set, peers exchange [`pair_proof`](../../packages/protocol/src/datachannel.ts) on the DataChannel using HMAC-SHA256 over `roomId|sorted(fp1,fp2)|protocolVersion` — see [`apps/mobile/src/lib/pairing/pair-mac.ts`](../../apps/mobile/src/lib/pairing/pair-mac.ts). PSK is kept in **`sessionStorage`** only ([`session-material.ts`](../../apps/mobile/src/lib/pairing/session-material.ts)).
- **Android native:** [`GuardedSleepNativePlugin`](../../apps/mobile/android/app/src/main/java/org/guardedsleep/app/GuardedSleepNativePlugin.java) — app **battery details** intent, **keep screen on**, **immersive / kiosk-style** chrome, **mDNS** advertise/discover (`_guardedsleep._tcp`, service name `gssleep-<room>`). **Foreground notification:** [`@capawesome-team/capacitor-android-foreground-service`](../../apps/mobile/package.json) + [`foreground-monitoring.ts`](../../apps/mobile/src/lib/android/foreground-monitoring.ts) (baby monitoring start).
- **Parent UX:** listen toggle, **vibration** on noise alert, **screen wake lock** while listening, **snooze** (5 min), coarse **connection quality** from WebRTC stats, baby **battery** + **quiet-since** hints from heartbeats, optional **LAN peer list** from mDNS — [`apps/mobile/src/routes/parent/+page.svelte`](../../apps/mobile/src/routes/parent/+page.svelte).
- **Saved connections:** [`apps/mobile/src/lib/connection-store.ts`](../../apps/mobile/src/lib/connection-store.ts) (upsert on successful connect).
- **Gaps vs full product doc:** QR **join-request / auto-accept** timing UX, **multi-IP parallel ICE** probing beyond what WebRTC gathers, **saved connections + PSK** in secure storage (Keystore), **OEM-specific** battery exemption intents, **Play/F-Droid** release plumbing, **LAN↔Internet handoff**, and other **Phase 2+** items in feature docs.

### Phase 2 extension (multi-parent, planned)

- **After MVP 1:1 is validated:** up to **4** parent devices per **one** baby device (hard product cap).
- **Add device:** same QR workflow as initial pairing (`Add device` → show QR → scan). **Either** the baby or an already-paired **parent** may display the QR; on the baby, **kiosk unlock + explicit action** is required first (see [device-management.md](../features/device-management.md)).

### Pre-Negotiated Audio: How It Works

The "pre-negotiated muted track" does NOT transmit data when muted:

- At connection setup, a MediaStream track is added but `track.enabled = false`
- With Opus DTX: silence frames cost ~0.2-0.5 MB/h (negligible even on mobile data)
- Pre-negotiated on ALL connection types (LAN + mobile) for instant audio activation
- Alert-triggered: when noise threshold exceeded, audio auto-enables and plays on parent
- Listen mode: parent manually enables audio streaming from baby
- Direction: always baby -> parent only
- When parent taps "Listen" or alert triggers: `track.enabled = true`, audio flows immediately (~50-100ms)

### Pairing Behavior

- **QR code actively displayed on screen**: scanning device is auto-accepted (physical proximity = trust)
- **QR code no longer displayed** (screen changed, timeout): any connection attempt triggers a join request notification on the existing device: "Device [name] wants to join -- Accept / Reject"
- This prevents unauthorized pairing while keeping the setup flow simple during initial configuration

### Design Consideration: LAN <-> Internet Handoff

> STATUS: IDEA -- Architecture must support this from the start. Phase assignment TBD.

When the local connection drops (device leaves WiFi range), the app should automatically attempt to reconnect via signaling server or stored VPN IPs. When both devices are on the same network again, prefer the direct local connection for lowest latency.

This requires:

- Store all known IPs per paired device (local + VPN + signaling server)
- On connection loss: try all stored IPs in parallel + signaling server
- Priority: local IPs > VPN IPs > signaling server (latency preference)
- Seamless switch without user interaction
- IP sync mechanism keeps stored IPs current

**Phase 1 implements all three paths (local + signaling + VPN). User manually reconnects if mode changes. Automatic handoff is a future feature.**

### Explicitly NOT in Phase 1

| Feature                             | Phase | Reason                                                                          |
| ----------------------------------- | ----- | ------------------------------------------------------------------------------- |
| iOS support                         | 2     | Unvalidated background audio, needs custom native plugin, no Apple hardware     |
| LAN <-> Internet automatic handoff  | TBD   | Both modes work in Phase 1, but switching is manual                             |
| Talk-back                           | 2     | Adds complexity, not essential for monitoring                                   |
| Application E2E layer (Layer 3)     | 2     | DTLS + fingerprint binding is sufficient for Phase 1                            |
| Key rotation                        | 2     | Pairing key valid until manual re-pair                                          |
| Multi-parent (max 4 parents / baby) | 2     | MVP is 1:1 only; Phase 2 adds parents with same QR add-device flow              |
| Remote pairing (invite code)        | 2     | Physical QR pairing only in Phase 1                                             |
| Video monitoring                    | 3+    | Future feature                                                                  |
| Sound categorization (ML)           | 3+    | Future feature                                                                  |
| Multi-language (i18n)               | 1     | en/de/fr/es from Phase 1 (owner decision 2026-10-19; supersedes "English only") |

---

## 9. PWA Distribution (Desktop/Browser Fallback)

> For Phase 2/3 -- documenting here for reference.

A PWA (Progressive Web App) is distributed as a website. Users access it via URL and can "install" it:

**How users install a PWA:**

1. Open the URL in a browser (Chrome, Edge, Firefox, Safari)
2. Browser detects PWA capability (manifest.json + Service Worker + HTTPS)
3. Browser shows "Install" banner or user clicks browser menu -> "Install app" / "Add to Home Screen"
4. PWA appears as an icon on home screen / desktop, opens in its own window (no browser chrome)

**How we distribute it:**

- Host the PWA on a web server (e.g., GitHub Pages, Netlify, or self-hosted)
- Users visit the URL and install
- Updates are automatic (server-side deployment, Service Worker fetches new version)
- No app store needed, no review process, no fees
- Can ALSO be wrapped via Google Play (Trusted Web Activity) or Microsoft Store

**Limitations on iOS:**

- Safari supports PWA install but with restrictions (no push notifications in EU until recently, limited storage)
- For full iOS support, the Capacitor-wrapped native app is needed

---

## 10. App Store Distribution & Monetization

> STATUS: DECIDED -- See [ADR-005](../decisions/ADR-005-parent-alerts-product-naming-and-distribution.md); details below.

### Capacitor = Real Native Apps

Capacitor wraps our web app into **real native apps** (.apk/.aab for Android, .ipa for iOS). These are indistinguishable from "native" apps to the stores and users. They can be published to:

| Store               | Requirements                                               | Fees                       |
| ------------------- | ---------------------------------------------------------- | -------------------------- |
| **Google Play**     | Google Play Developer account ($25 one-time)               | 15-30% on in-app purchases |
| **Apple App Store** | Apple Developer Program ($99/year), macOS + Xcode required | 15-30% on in-app purchases |
| **F-Droid**         | Open source, submit metadata to fdroiddata repo            | Free, no fees              |
| **Direct APK**      | Host on website / GitHub Releases                          | Free                       |

### Distribution Strategy

| Channel                                      | Target Audience                         | Version                                                     |
| -------------------------------------------- | --------------------------------------- | ----------------------------------------------------------- |
| **F-Droid** (main repo deferred, ADR-021 §3) | Privacy-conscious users, FOSS community | Full version, free, with direct donation links              |
| **Google Play**                              | Mainstream Android users                | Full version, free, with optional "Support" in-app purchase |
| **Direct APK** (GitHub)                      | Developers, early adopters              | Full version, free                                          |
| **Apple App Store**                          | iOS users (Phase 2)                     | Full version, free, with optional "Support" in-app purchase |

### Monetization: "Support Me" Model

The app is **free and fully functional** in all versions. No features behind paywall.

**Google Play version:**

- "Support the Developer" in-app purchase (tip jar): e.g., €1.99, €4.99, €9.99
- Uses Google Play Billing (mandatory -- Google prohibits external payment links)
- Google takes 15% commission (reduced rate for small developers <$1M revenue)
- No donation links to external sites allowed in the Play Store version

**F-Droid / APK version:**

- Direct donation links allowed: GitHub Sponsors, Open Collective, Liberapay, PayPal
- No Google commission
- Full flexibility in how to receive support

**Build flavors**: Use Android product flavors in `build.gradle` to manage two variants:

- `playStore` flavor: includes Google Play Billing, no external donation links
- `foss` flavor: includes external donation links, no Google Play dependencies

### Important: Google Play Policy

Google Play prohibits apps from linking to external payment/donation platforms. Apps have been removed for this (including the WireGuard app). The `playStore` build flavor must use Google's billing system exclusively. The `foss` flavor (F-Droid, direct APK) has no such restriction.

---

## Recommended Architecture Summary

```
┌──────────────────────────────────────────────────────────────┐
│                    ARCHITECTURE (Phase 1 MVP)                 │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Platform:       Web App + Capacitor (Android only)          │
│                  iOS and Desktop deferred to Phase 2/3       │
│                                                              │
│  Communication:  WebRTC                                      │
│                  - DataChannel: heartbeat, alerts, control    │
│                  - MediaStream: pre-negotiated (muted),       │
│                    enabled on "Listen" tap (~50-100ms)        │
│                  - Mobile data: no auto-stream, explicit only │
│                                                              │
│  Pairing:        QR code (PSK + DTLS fingerprint binding)    │
│                  QR visible: auto-accept on scan              │
│                  QR dismissed: join request (accept/reject)   │
│                  3-step setup: install → role → scan QR       │
│                                                              │
│  Encryption:     Layer 1: DTLS-SRTP (WebRTC, automatic)      │
│                  Layer 2: DTLS fingerprint pinned to PSK      │
│                  Internet: + WireGuard (Tailscale, double enc)│
│                  (Layer 3 E2E deferred to Phase 2)            │
│                                                              │
│  Connectivity:   3 paths, same app logic:                    │
│                  1. Local WiFi/Hotspot (no server)            │
│                  2. Internet: signaling + EU STUN (mainstream)│
│                  3. VPN (Tailscale/Headscale, power-user)     │
│                  QR contains ALL IPs + signaling URL          │
│                  Auto-detection: fastest path wins            │
│                                                              │
│  Server:         Signaling server (Docker, ~300-400 LOC)     │
│                  Community instance for non-technical users    │
│                  EU STUN + optional self-hosted TURN (coturn)  │
│                                                              │
│  Battery:        Target: 8h+ screen-off monitoring           │
│                  Screen off in baby mode                      │
│                  Charging recommended for overnight           │
│                                                              │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                    ARCHITECTURE (Full Vision)                 │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Phase 2 adds:   iOS support (custom native audio plugin)    │
│                  LAN <-> Internet automatic handoff           │
│                  Talk-back, Multi-device, E2E Layer 3         │
│                  Remote pairing via invite code               │
│                  i18n, PWA desktop fallback                   │
│                                                              │
│  Phase 3 adds:   Video monitoring, Sound categorization (ML) │
│                  Advanced cluster management                  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Next Steps

| Priority    | Action                                                                                   | Blocks               |
| ----------- | ---------------------------------------------------------------------------------------- | -------------------- |
| **BLOCKER** | Build PoC: Capacitor + WebRTC + Background Audio on Android                              | Everything           |
| **BLOCKER** | PoC: Multi-IP QR code + parallel connection + signaling server                           | Internet mode        |
| **HIGH**    | Formalize MVP scope as ADR-001                                                           | Implementation start |
| **HIGH**    | Build minimal signaling server (~300-400 LOC, Docker)                                    | Internet mode        |
| **HIGH**    | Design Local Discovery mechanism for reconnection                                        | Implementation       |
| **HIGH**    | Verify Opus DTX silence frame data usage in PoC                                          | Audio optimization   |
| **MEDIUM**  | Choose web framework (Svelte/React/Vue)                                                  | Implementation       |
| **MEDIUM**  | Design UX wireframes for 3-step setup flow                                               | Implementation       |
| **MEDIUM**  | Set up app store distribution (Google Play + F-Droid; F-Droid main deferred, ADR-021 §3) | Distribution         |
| **LOW**     | Write Tailscale/Headscale setup guide (optional power-user mode)                         | Documentation        |
| **LOW**     | Plan iOS PoC (after Android validated)                                                   | Phase 2              |
