# ADR-012: Connection model -- offline-LAN signaling, path selection, and network handoff

- Status: PROPOSED
- Date: 2026-05-13
- Deciders: Product owner (user), coordinated design (Tech Architect, Security/Privacy, UX, Critical Reviewer 2026-05)
- Supersedes parts of: [ADR-003](ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md) "Local: no server" -- this ADR **precises** that statement; see §"Relationship to ADR-003".

## Context

Today `apps/mobile/src/lib/webrtc/peer-session.ts` always opens a WebSocket to a single signaling URL via `connectSignaling(...)`. If that URL is unreachable (no internet, captive portal, hotel WiFi blocking WSS), the session goes to `failed` -- even when the two devices are sitting on the **same LAN** and could trivially exchange SDP/ICE locally. The current Android plugin already advertises and discovers peers via NSD (`_guardedsleep._tcp.`, `lanPeerResolved` event), but the discovered host is **only shown in a diagnostics box** -- it never feeds into the actual signaling.

Three other gaps surfaced in the [2026-05 Critical Review](../reviews/concept-review-2026-05.md) and the connection-model design pass:

1. **Live network handoff** (Wi-Fi ↔ cellular, walk out of range and back) -- no listener on OS network events; the only mitigation today is exponential backoff with full `start()` on next attempt ([`session-auto-reconnect.ts`](../../apps/mobile/src/lib/webrtc/session-auto-reconnect.ts)).
2. **Concurrent path racing** -- there is no abstraction that says "try LAN first, then internet"; whichever signaling URL is in the query string wins.
3. **Privacy:** the mDNS service name is `gssleep-<roomId>`, broadcasting the room id to anyone with NSD on the LAN (security sub-agent flagged as IMPORTANT).

## Goals

GuardedSleep must work, with the **same install and the same monitoring session**, in:

1. Internet available, devices on **different networks** (parent cellular, baby home Wi-Fi).
2. Internet available, devices on the **same Wi-Fi** (signaling reaches both; ICE picks host candidates → media path is effectively local).
3. **No internet** at all, devices on the **same LAN / hotspot**.
4. Live handoff Wi-Fi ↔ cellular mid-session (e.g. router reboot, walking out of range).
5. Transient outage with automatic reconnect when the network returns.

## Decision

**Adopt a unified connection-orchestrator with explicit path ranking, opaque LAN discovery, and hysteresis-based handoff.** Implemented in stages: path-selector + `PeerSession` shipped first; Phase 2 adds the embedded baby-side LAN relay, opaque NSD, `@capacitor/network` handoff, and baby-side prolonged-disconnect pulse (see Phase split table).

### 1. Connection state machine

The parent app drives the following states (binding for `PeerSession` callers and UI):

```
idle
  └─ user starts monitoring ──>
discovering_paths   (collect: cached LAN host from NSD, saved signaling URL, query-param URL)
  └─ all paths empty ──> failed_needs_user
  └─ next path ──>
signaling_connecting   (open WebSocket / local listener)
  └─ connect timeout / error / close ──> next path or backoff
  └─ open ──>
webrtc_negotiating     (SDP/ICE exchange via that signaling)
  └─ ICE failed / pair_proof failed ──> next path or backoff
  └─ DataChannel open + pair_proof ok ──>
connected
  ├─ network event change (debounced 2-5 s) ──> reconnecting_handoff
  ├─ heartbeat loss / WebRTC failed ──> reconnecting_backoff
  └─ user stop ──> idle

reconnecting_backoff  (use existing sessionReconnectDelayMs())
  └─ retries exhausted ──> failed_needs_user
  └─ next attempt ──> discovering_paths

reconnecting_handoff  (network changed; do NOT count as failure)
  └─ network stable for hysteresis window ──> discovering_paths
```

**Cancellation:** state transitions explicitly cancel in-flight `PeerSession` instances; **only one** `PeerSession` is alive at a time on the parent (Single-Writer invariant). The baby side starts a `PeerSession` on demand from the embedded listener (Phase 2) and from a query-param URL today.

### 2. Path ranking and path selection

A small framework-free TS module **`lib/signaling/path-selector.ts`** (see [§Implementation](#implementation)) takes an **ordered list** of candidate signaling URLs and returns the **first** that completes a WS open within a per-URL timeout. Returns `null` if none are reachable.

**Ordering rules (parent, monitoring start):**

1. **LAN host candidate** from NSD `lanPeerResolved`, if any matches the active room (Phase 2 wires this; Phase 1 only ranks if the host is explicitly known and reachable, e.g. via saved-connection `fallbackSignalingUrls`).
2. **Saved-connection `signalingUrl`** (today's behavior).
3. **Saved-connection `fallbackSignalingUrls`** (new; manually configured, e.g. local NAS Docker, see ADR-008 for the image).
4. **Query-param `signal`** from invite link (used for first pairing only).

The baby side, in the absence of an embedded local listener (Phase 2), simply uses the URL from its own invite QR / query string.

**Per-URL timeout:** 4 s default, configurable for tests. The aggregate "all paths failed" wait is bounded; we do **not** wait for all candidates if a higher-priority one succeeds.

### 3. Offline-LAN signaling (Phase 2)

> STATUS: **DECIDED**; Android baby embedded relay + opaque NSD + handoff listener **implemented** (Phase 2). PWA / iOS remain internet-signaling-only for hosting reasons (see platform matrix).

**Approach (Tech Architect Option A, Security: APPROVED WITH CONCERNS):**

- **Android Baby unit** opens an **embedded WebSocket listener** on the LAN IP, on a **stable per-install random port** (persisted; falls back to ephemeral on conflict). Speaks the **same** `@guardedsleep/protocol` messages as the public signaling-server (`hello`, `welcome`, `peer`, `signal`, ...). Implemented in a Capacitor plugin extension to `GuardedSleepNativePlugin`, started/stopped **only with the monitoring foreground service** and holding a **`WifiManager.WifiLock`** (and **`MulticastLock`** for NSD) for the duration.
- **Advertised via existing NSD** (`_guardedsleep._tcp.`) -- the **port from `WifiLock`-protected listener replaces the placeholder `9`** currently in `GuardedSleepNativePlugin.java`.
- **Discovery name is OPAQUE:** the service name is changed from `gssleep-<roomId>` to a per-session **random discovery token** (no semantic link to room id; ~64 bits entropy, regenerated per session). The room id is **only** revealed after the WebSocket open and `pair_proof` (PSK + DTLS fingerprint) succeeds. Migration: the old `gssleep-<roomId>` name is dropped; backwards compatibility is **not** required because no users yet rely on offline-LAN (the feature is new). This addresses the Security IMPORTANT finding.
- **Parent (Android)** resolves the NSD service, builds `ws://<lan-ip>:<port>` (no TLS on LAN listener -- relying on **DTLS-SRTP + `pair_proof`** for E2E security; the LAN signaling is treated as **untrusted relay** per `AGENTS.md` §7).
- **PWA / iOS scope (non-goal in MVP):** PWA browsers cannot host an incoming socket and Capacitor iOS is not in MVP scope; for those targets, **internet signaling is required**. Users who want fully-offline LAN on those platforms can run the public-image `guardedsleep-signaling` Docker on a NAS/router (see option D in §Options Considered and [ADR-008](ADR-008-container-image-build-and-distribution.md)). Stated explicitly in the platform matrix below.

**Android lifecycle (binding):**

| Event                             | Embedded WS listener                                                                       | NSD advertise                  |
| --------------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------ |
| Monitoring starts (Baby)          | start, bind to Wi-Fi IPv4 + optional IPv6                                                  | start (opaque discovery token) |
| Monitoring stops                  | stop                                                                                       | stop                           |
| FGS killed by OS                  | stop (no orphan socket)                                                                    | stop                           |
| Wi-Fi disconnects                 | stop and restart on next interface                                                         | stop (re-advertise on new IP)  |
| Wi-Fi sleep / Doze                | hold `WifiLock(WIFI_MODE_FULL_HIGH_PERF)` + `MulticastLock` for as long as monitoring runs |                                |
| Port conflict                     | catch BindException, fall back to ephemeral port, persist new port                         | re-register service info       |
| Boot completed (auto-resume baby) | listener starts only when FGS starts; no boot-receiver socket                              |                                |

**Threat model (Security sub-agent):** treat the embedded listener like the public signaling-server -- only metadata, no app traffic before `pair_proof`. Realistic adversaries: (1) nosy guest on the same Wi-Fi, (2) rogue AP / captive portal, (3) malware/second app on the same device. All three are deflected by `pair_proof` MAC binding + DTLS fingerprint check; the listener never reveals room/PSK material.

### 4. Live network handoff (Wi-Fi ↔ cellular)

**Listen API:** [`@capacitor/network`](https://capacitorjs.com/docs/apis/network) `Network.addListener('networkStatusChange', ...)` is the primary signal. `navigator.onLine` is **insufficient** alone (false-positive on captive portals). Phase 2 wires the listener; this ADR binds the **policy**.

**Policy (decision):**

- **Hysteresis:** ignore network-change events for the first **2-5 s** (configurable, default **3 s**) -- transient flapping does not trigger reconnect.
- After the window elapses with a **different** active network, drop the current `PeerSession`, run `discovering_paths` fresh (same path-ranking), and start a new `PeerSession`. **No half-reuse** of the prior peer connection.
- During reconnect: UI stays in **`reconnecting_handoff`** state -- silent for 0--20 s (matching the existing failure-model escalation), then the standard escalation kicks in (see [ADR-013](ADR-013-connection-status-observability-and-copy.md) for copy + timing).
- `restartIce()` (in-place) is **explicitly deferred** as a later optimization once the orchestrator is stable. Reasoning: fewer Android-specific quirks, fewer Audio-Focus interruption issues when we tear down cleanly.
- **Listen mode auto-resume:** if the user had Listen ON when the handoff started, the parent **re-enables** Listen automatically once the new session reaches `connected`, **without** an extra tap. A small toast ("Connection restored -- Listen on") acknowledges the auto-resume.

### 5. Connection status surface (cross-reference)

UI/UX (ambient bar, escalation timing, plain-language copy) is delegated to [ADR-013](ADR-013-connection-status-observability-and-copy.md). This ADR fixes only the **states** the UI must render.

### 6. Relationship to ADR-003

[ADR-003](ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md) §"Three connectivity paths" line 2 says **"Local: WiFi / hotspot, no server"**. That phrasing is misleading and is **clarified** by this ADR: "no separate signaling **product/service** is required" (true), but a **process listening on a socket on one of the peers** is still needed for SDP/ICE exchange. ADR-003 is **not superseded**; a precision note is added to it pointing here.

## Options Considered

### Option A: Embedded WS listener on Baby-Android (CHOSEN)

- **Pros:** reuses the existing `@guardedsleep/protocol` messages; Parent code path identical; Android-native lifecycle (FGS, Wi-Fi lock) is well-understood.
- **Cons:** Android-only for MVP (PWA/iOS need internet signaling); native Java/Kotlin code; port management.

### Option B: SDP-over-NSD-TXT

- **Pros:** no second socket.
- **Cons:** SDP and ICE candidates exceed mDNS TXT-record sizes; fragmentation is unreliable. **REJECTED** -- the Tech Architect sub-agent verdict was `NEEDS REWORK`.

### Option C: Tiny TCP/UDP socket for SDP/ICE only (no WebSocket)

- **Pros:** even smaller than a WS server.
- **Cons:** introduces a second protocol that must be security-reviewed in parallel; WebSocket protocol is already battle-tested in this codebase. **REJECTED** for MVP; revisit only if Option A's WS lifecycle proves too painful on OEMs.

### Option D: Mandatory user-run LAN Docker signaling

- **Pros:** zero extra code in the apps; reuses `ghcr.io/<owner>/guardedsleep-signaling` ([ADR-008](ADR-008-container-image-build-and-distribution.md)).
- **Cons:** excludes ordinary families without a NAS; setup friction kills the "install, scan, done" UX promise.
- **Kept** as **complementary** power-user path (documented in [connectivity-pairing.md](../features/connectivity-pairing.md) §"LAN Docker fallback"), **not as a substitute for Option A.**

### Option E: BLE / Wi-Fi Aware for signaling

- **Pros:** no IP at all.
- **Cons:** complex permissions on Android (Bluetooth Scan), worse on iOS, fragile on old devices ([Persona C](../concept/personas.md)). **REJECTED** for MVP; documented as IDEA only.

### Option F: Defer offline-LAN to Phase 2 entirely

- **Pros:** smallest code change today; only path-selector + tests.
- **Cons:** keeps a known [Persona B / Persona A](../concept/personas.md) failure mode unaddressed.
- **REJECTED** as full deferral. **Compromise actually chosen:** documentation + path-selector + saved-connection `fallbackSignalingUrls` shipped in the first sprint; embedded baby-side LAN relay + opaque NSD + network handoff shipped in Phase 2 (this document’s Phase split table).

## Phase split

| Item                                                                                    | Phase                                                                                                    |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| ADR-012 documented, ADR-013 documented, ADR-003 precision note                          | **MVP / this sprint**                                                                                    |
| `path-selector.ts` + unit tests                                                         | **MVP / this sprint**                                                                                    |
| `connection-store.ts` extension: `fallbackSignalingUrls?: string[]`                     | **MVP / this sprint**                                                                                    |
| `PeerSession` consumes path-selector for ordered URL attempts                           | **MVP / this sprint**                                                                                    |
| Playwright E2E test: two WS instances, primary unreachable → fallback wins              | **MVP / this sprint**                                                                                    |
| Ambient status bar component + symmetric, non-technical copy + reconnect countdown copy | **MVP / this sprint** (per [ADR-013](ADR-013-connection-status-observability-and-copy.md))               |
| Embedded WebSocket server on Baby-Android (`GuardedSleepNativePlugin` extension)        | **Phase 2 — implemented** (Java relay + `prependSignalingUrls` / localhost WS)                           |
| Opaque NSD discovery token (replacing `gssleep-<roomId>`)                               | **Phase 2 — implemented** (`gst-<token>`, `lanPeerResolved` includes `port` + `token`)                   |
| `@capacitor/network` listener + handoff hysteresis state machine                        | **Phase 2 — implemented** (`subscribeNetworkHandoff`, 3 s debounce → `start({ resetBackoff: true })`)    |
| `pc.restartIce()` optimization                                                          | **Phase 3** if metrics show audio-cut > 2 s is a real complaint                                          |
| Slow red baby pulse animation (failure-model F-1 secondary observability)               | **Phase 2 — implemented** (`guardedsleep-slow-pulse` after ~60 s idle/failed without parent)             |
| Captive-portal detection / out-of-home explicit prompts                                 | **Phase 2** (referenced in [`connectivity-pairing.md`](../features/connectivity-pairing.md) Out-of-Home) |

## Platform matrix

| Scenario                        | Android↔Android                                               | Android↔PWA                                                          | PWA↔PWA / iOS  |
| ------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------- | -------------- |
| Internet, different networks    | yes                                                           | yes                                                                  | yes            |
| Internet, same LAN              | yes                                                           | yes                                                                  | yes            |
| **No internet, same LAN**       | **yes** (baby embedded WS + NSD; `pair_proof` on DataChannel) | **no** (PWA cannot host listener -- needs user-run Docker, Option D) | **no** for MVP |
| Live Wi-Fi ↔ cellular handoff   | **yes** (`@capacitor/network` debounced reconnect)            | partial (`online`/`offline` events only)                             | partial        |
| Transient outage auto-reconnect | yes (already today, backoff)                                  | yes                                                                  | yes            |

## Non-Goals

- Automatic Wi-Fi + cellular **bonding** (both interfaces simultaneously).
- Captive-portal-bypass tricks (privacy-hostile, may be flagged by stores).
- Cross-platform native libraries for offline LAN beyond the existing Capacitor plugin scope.
- Persisting full SDP into the QR code (size unreliable -- already rejected in [analysis.md](../technical/analysis.md)).

## Implementation

> Concrete files in this sprint:
>
> - `apps/mobile/src/lib/signaling/path-selector.ts` -- pure TS, framework-free; tries an ordered list of WS URLs with per-URL timeout, returns first reachable or `null`.
> - `apps/mobile/src/lib/signaling/path-selector.test.ts` -- Vitest, mock WebSocket constructor.
> - `apps/mobile/src/lib/connection-store.ts` -- add `fallbackSignalingUrls?: string[]`.
> - `apps/mobile/src/lib/webrtc/peer-session.ts` -- consume path-selector before first WS open; accept ordered URL list in addition to single URL.
> - `apps/e2e/tests/13-signaling-fallback-lan-simulated.spec.ts` -- spin up two WS-server processes (existing `signaling-server` code), close the first to simulate "no internet", verify the parent + baby still meet via the second.
> - Native Android embedded server -- separate PR per Phase 2.

## Consequences

### Positive

- Honest behavior for the documented "no internet, same network" scenario (Persona B home Wi-Fi router reboot; Persona A hotel where the only working path is two phones + a hotspot).
- Single state machine for both first-pairing and reconnect; less code drift.
- Discovery privacy posture (opaque token) aligns with `AGENTS.md` §7.

### Negative

- Two ADRs (012/013) to maintain; more cross-references.
- Two-stage delivery (orchestrator now, embedded server later) -- documentation must clearly mark "Phase 2" so users do not promise the offline-LAN feature before native code lands.

### Risks

- **Android OEM lifecycle quirks** (Doze, Wi-Fi sleep on aggressive OEMs) may kill the embedded listener silently. Mitigation: Phase 2 PR will include the "Lakmus test" idea from [failure-model.md](../concept/failure-model.md) F-5 -- counted heartbeats over the first 30 min to detect silent OS-kill of the listener.
- **NSD on hotspot / captive networks**: NSD may not traverse client-isolation. Mitigation: documented in [connectivity-pairing.md](../features/connectivity-pairing.md) §"Out-of-Home"; user-facing copy must say "If both phones cannot find each other, switch one phone's mobile data on".
- **CI test feasibility**: the user-requested test "no internet, but same LAN" cannot be reproduced in Playwright with real NSD. We instead test the **path-selection layer** (Tech Architect Option A's orchestration responsibility) with two WS-server processes; real NSD + real offline LAN is covered by Phase 2 Android-instrumented tests and manual release validation.

## Discarded alternatives

| Alternative                                               | Reason                                                                                                     |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Option B -- SDP over NSD TXT                              | Size limits; fragmentation unreliable.                                                                     |
| Option C -- raw TCP/UDP                                   | Adds a second protocol surface for no measurable gain over WS.                                             |
| Option D as **sole** offline-LAN story                    | Requires every user to run Docker; kills the "install, scan, done" UX. Kept as complement, not substitute. |
| Option E -- BLE / Wi-Fi Aware                             | Permission heavy; fragile on old devices.                                                                  |
| Option F -- defer entirely                                | Leaves Persona A/B failure modes unaddressed; orchestration layer is small enough to ship now.             |
| Aggressive immediate full-reconnect on every network blip | UX disaster on commute / weak Wi-Fi; rejected in favor of hysteresis.                                      |
| `restartIce()` first                                      | Increased OEM-quirk surface; deferred until orchestrator is stable.                                        |
| Boot-receiver-started LAN listener (no FGS)               | OS may kill silently; would violate failure-model F-5 mitigations.                                         |
