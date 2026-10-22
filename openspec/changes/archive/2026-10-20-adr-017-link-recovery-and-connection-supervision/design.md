# ADR-017: Link recovery and connection supervision

- Status: DECIDED (owner, 2026-10-20; revision 2 after role reviews)
- Date: 2026-10-19
- Deciders: Project owner (pending); reviewed by Security/Privacy Architect (NEEDS REWORK → addressed), Critical Reviewer (NEEDS REWORK → addressed), Technical Architect (APPROVED WITH CONCERNS), Product Owner (APPROVED WITH CONCERNS), UX
- Related: [ADR-004](../2026-04-15-adr-004-encryption-transport-and-key-storage/design.md) (amended: direction-bound `pair_proof`), [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md) (open item C1, gate AC-4/AC-10), [ADR-012](../2026-10-21-adr-012-connection-model-offline-lan-and-handoff/design.md), [ADR-013](../2026-10-21-adr-013-connection-status-observability-and-copy/design.md), [failure-model.md](../../../../docs/concept/failure-model.md), [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md)
- Supersedes (once DECIDED): ADR-012 §1 "retries exhausted → failed_needs_user"; failure-model "Reconnect timeout 15 s → troubleshooter"; `heartbeat_stale_ms` default 50 s; the DECIDED heartbeat range "5–30 s" (now 5–15 s, §5)

> **Migrated to OpenSpec (2026-10-21):** §2–§9 and §11 are now normative in [`openspec/specs/link-supervision/spec.md`](../../../specs/link-supervision/spec.md); change them there. This file keeps the rationale and options, §1 is normative in [`openspec/specs/pairing`](../../../specs/pairing/spec.md). §10 is normative in [`openspec/specs/signaling-relay`](../../../specs/signaling-relay/spec.md) (server side as built; the Rust server is the prepared change `add-rust-signaling-server`), and the §12 cases are scenarios in `link-supervision`. Archived 2026-10-22.

> **Context note (2026-10-20):** references to the Capacitor app and the Node server are historical; the Rust `relay` crate and server ([ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md) §3, §6) implement the server-side liveness and room pinning described here. No decision in this ADR is changed.

## Context

A baby monitor that silently stops monitoring is worse than none. Findings before this ADR:

- The core `Session` ends permanently on `PeerConnectionState` `failed`/`closed`. The web client has a route-level reconnect loop for the parent (`session-auto-reconnect.ts`, 36 attempts, 45 s cap) and a network-handoff listener, but no recovery for the baby and nothing in the native app.
- Only the baby sends heartbeats, so the baby cannot tell whether a parent is listening (failure-model F-1).
- The signaling server has no WebSocket liveness check: a baby whose Wi-Fi flapped is refused with `BABY_TAKEN` until TCP gives up, possibly for hours. Today's core treats every server error as fatal.
- **Security defect (review C1):** `pair_proof` is symmetric — both sides send exactly the MAC they expect. Anyone who knows the room id can join as parent, reflect the baby's proof, pass verification and open the microphone with `listen`. This affects the web client and the PoC core alike and must be fixed before any recovery logic multiplies attempts.

## Decision

### 1. Wire protocol v3 (clean cutover)

- `PROTOCOL_VERSION` becomes **3**. v2 and v3 peers do not mix: the server accepts both versions but pins each room to the version of its first member (`UNSUPPORTED_PROTOCOL` otherwise), so the Capacitor app keeps working on a shared server. There is no partial compatibility.
- **Direction-bound pairing proof (ADR-004 amendment):** `mac_role = HMAC-SHA256(PSK, "gs-pair-proof/3|" + role + "|" + room + "|" + fp_sender + "|" + fp_receiver)`. Each side sends `mac_self` and verifies the peer's `mac_peer` with roles and fingerprints swapped. A reflected proof never matches. Conformance vectors cover the reflection attack.
- **Room id derived from the PSK:** `room = base64url(HKDF-SHA256(PSK, info "gs-room/3"))[0..22]` (128 bit). The QR carries only the PSK, the signaling URL list and an expiry. A room id is no longer guessable and needs no separate protection.
- New DataChannel messages: parent `heartbeat`, `device_status`, `missed_events`, `quiet_request`/`quiet_state` (ADR-018). New signal payloads: `aid` on every `sdp`/`ice`, and `request_offer`.

### 2. Two layers in the core: `Attempt` and `Link`

- **Attempt**: one RTCPeerConnection lifetime with its own pairing proof; no signaling state; never retries. Extracted from today's `Session`, which stays a thin wrapper so existing vectors remain valid.
- **Link**: one monitoring relationship from "start" to "stop". Owns the signaling-transport state, attempts, liveness, escalation, listen intent, snooze, peer status and the missed-event buffer. Platforms talk **only to the Link**.
- A Link always has a PSK (none = construction error; the dev bypass of `Session` is not available in `Link`).

### 3. Recovery is a new attempt — make-before-break

- Recovery creates a fresh RTCPeerConnection with new DTLS certificates and a new pairing proof. ICE restart stays `STATUS: IDEA` (measure the cost of new-attempt recovery over TURN on hardware first).
- **At most one live attempt.** The core always emits `close_attempt` before `create_attempt`. Inputs that carry a closed handle are dropped silently (late WebRTC callbacks).
- **A verified attempt is closed only by its own failure** — its peer connection `failed`/`closed`, its DataChannel closing, or its liveness timeout (§5). Nothing that arrives through signaling can close a verified attempt: not a new offer, not a new `aid`, not a membership change.
- **Who offers:** the baby, and only when it has no live verified attempt and a parent is present. The parent sends `request_offer` (to the baby's client id) when it joins or recovers without a live attempt; the baby answers with an offer for a fresh attempt. This also makes recovery after a parent process restart immediate.
- **Who is listened to:** offers and candidates are accepted only from the client id that `members` currently lists as the room's baby (on the parent) or from the parent that sent the most recent `request_offer` (on the baby), always addressed with `to`. `aid` (128 bit) only filters stale candidates; it is never a trust decision.

### 3a. Implementation refinements (core implementation 2026-10-19)

- An offer that answers a `request_offer` carries `re` = the request's id; the parent adopts it only for that offer, and the baby ignores answers from superseded attempts. Without it, two sides that start at the same time strand each other's answers (reproduced in the two-link harness).
- `LinkConfig.paired_before` persists "this PSK has verified at least once", so the "never terminal after a successful pairing" rule survives a process restart.
- `alarm_after_ms` must be below the level-3 threshold (180 s).
- An authenticated `stop_notice` closes the verified attempt immediately; after it the link waits until the peer leaves and rejoins the room.
- A parent restart while the baby still holds a verified attempt recovers after the baby's liveness timeout (≤ 25 s default), not immediately: the baby ignores `request_offer` while verified (make-before-break). This stays below level 2.
- Security review 2026-10-20: `UNSUPPORTED_PROTOCOL` is honoured only as the reply to `hello` (before `welcome`); with a verified attempt it only closes signaling and sets a sticky `update_required` hint. A signaling `stop_notice` must carry `mac = HMAC(PSK, "gs-stop/3|role|room|receiver_client_id|nonce")` with a fresh nonce; unauthenticated stops are ignored, so a relay cannot mute escalation. Remote ICE is ignored once an attempt is verified. `LinkStatus.last_alive_ms` exposes the last contact for "not reachable for m:ss".
- Open: a ghost room member can keep replacing the baby's _unverified_ attempt with `request_offer` (verified attempts are unaffected); a per-peer rate limit is a follow-up.

### 4. Attempt identity and determinism

- **Local handle** (u32 counter) between core and platform; **wire `aid`** (128 bit) drawn from a PRNG seeded by `start{entropy}`. Traces record the seed, so replay is deterministic.

### 5. Liveness and loss detection

- **Both** sides send `heartbeat` every `heartbeat_interval_ms` (default 10 s; Advanced **5–15 s**). The heartbeat `t` field is opaque to the receiver; receivers stamp wall-clock time on arrival (the core clock is monotonic).
- A verified attempt is lost on: peer connection `failed`/`closed`; DataChannel closed; or **no DataChannel message for `liveness_ms` = 2 × interval + 5 s** (25 s default, 35 s maximum). `disconnected` alone triggers nothing. Absence from `members` is **not** a loss signal (it happens on every server restart); it only stops further offers.
- Invariant enforced at configuration time: `liveness_ms ≤ alarm_after_ms − 20 s`.
- `last_alive` = time of the last DataChannel message from the peer. All escalation timing is measured from `last_alive`.

### 6. Attempt scheduling and errors

- Delays before a new attempt: 0 s, then 1, 2, 4, 8, 15 s (cap); after 10 minutes without success the cap rises to 30 s (battery). Reset after a verified attempt.
- **Immediate retry and backoff reset** on: `network_available` (new platform input), signaling reopened, or the peer reappearing in `members`.
- An attempt not verified within 20 s is abandoned.
- Server errors `BABY_TAKEN`, `PARENT_CAP`, `NOT_JOINED`, `SIGNAL_TARGET_REQUIRED` are **retryable** at the signaling layer. `UNSUPPORTED_PROTOCOL` is terminal (`failed_needs_user`, key `update_required`).
- **Pairing mismatch:**
  - First pairing (never verified with this PSK): two consecutive mismatches → `failed_needs_user` (key `pairing_key_mismatch`, "scan the code again").
  - After a successful pairing it is **never terminal**: each mismatch closes that attempt, emits `security_warning` (parent copy: "Someone tried to connect with a wrong key. Monitoring continues."), and slows retries (next delay ≥ 15 s).
- The link never gives up while monitoring is on. From escalation level 2 it surfaces a **troubleshooter while retrying**, with a cause hint: `server_unreachable` (signaling never opens), `peer_absent` (signaling fine, other device not in the room), `media_blocked` (offer/answer done but no verification after 3 attempts → likely needs TURN). "Try now" resets the backoff.

### 7. Link status and escalation (rendered per ADR-013)

States: `connecting` (never verified in this run), `connected`, `reconnecting`, `peer_stopped`, `failed_needs_user{key}`, `stopped`. Levels while not connected, from `now − last_alive`:

| Level | From                    | Parent                                                                                                       | Baby                                          |
| ----- | ----------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| 0     | 0 s                     | yellow, silent                                                                                               | edge indicator yellow                         |
| 1     | 20 s                    | orange, visual only (no vibration), low-priority notification                                                | edge indicator orange                         |
| 2     | `alarm_after_ms` = 60 s | red, repeating vibration, full-screen alert, soft tone every 10 s, **"connection lost" alarm** (`link_lost`) | slow dim red pulse — never sound or vibration |
| 3     | 180 s                   | audible alarm (alarm stream, DND bypass per ADR-018)                                                         | unchanged                                     |

**Deliberate stop is not an outage:** `stop` sends a `stop_notice` over the DataChannel (and signaling) before closing. The peer link goes to `peer_stopped` instead of `reconnecting`: the parent raises one non-repeating alert ("Baby unit stopped monitoring"), the baby shows a calm grey state. No escalation runs for a stopped peer.

**Acknowledge:** the parent input `acknowledge_alarm` ("I'm checking") silences sound and vibration of levels 2–3 for 5 minutes while the state stays red; if the link is still down afterwards the level jumps straight to 3. Owner decision 2026-10-20: level 1 is visual only; level 2 repeats a soft tone every 10 s (like classic hardware baby monitors). The core emits the level; platforms map it to feedback.

**Test contract (AC-4/AC-10 wording):** an outage of ≤ 30 s must recover without reaching level 2; an outage of ≥ 60 s must raise level 2 within 60 s + 2 s of `last_alive`; between 30 and 60 s either outcome is valid (grey zone). Snooze affects noise alerts only, never levels 1–3.

### 8. After recovery

- `link_restored{down_ms}` after verification following level ≥ 1; alarm sound and vibration stop; the parent shows "Connection restored — not monitored for m:ss".
- The baby buffers noise events while not connected (count, max level, relative offsets; no audio) and sends `missed_events{complete}` after verification. `complete = false` when the buffer did not cover the whole outage (e.g. the baby process restarted). The parent always shows the unmonitored duration and shows a count only when `complete`.
- Listen intent is re-sent after recovery if the outage was < 60 s, otherwise reset to off (ADR-013 §6).

### 9. Process death and restart — known limitations stated plainly

- **Parent:** the foreground service is `START_STICKY` and resumes monitoring from the persisted (encrypted) pairing.
- **Baby:** Android 14+ does not let a microphone foreground service start from the background. After the OS kills the baby process it resumes only when the app is opened; a "Tap to resume monitoring" notification is posted when possible. The parent escalates as usual meanwhile (F-2/F-5). Auto-start after reboot stays `DEFERRED`.
- The parent phone is a single point of failure for alerting; onboarding says so (F-1, F-5).

### 10. Signaling transport and server

- Clients send the application `ping` every 15 s; no message for 35 s marks the socket dead. Signaling loss while an attempt is verified is not a monitoring loss; the link reconnects signaling in the background.
- **Server changes:** WebSocket `ping()` every 10 s with an alive flag, `terminate()` sockets that missed one (≤ 20 s), so the normal cleanup and `members` broadcast run; for **both** roles. `BABY_TAKEN` semantics stay (no takeover). `SIGNALING_MAX_PARENTS` default **1** (MVP 1:1). Per-IP limits on hello rate and room creation. No IPs or room ids in logs. Reverse proxies must allow > 35 s idle (documented).

### 11. Sans-IO surface (normative; serde JSON form is the contract for every binding)

Inputs: `start{entropy}`, `stop`, `acknowledge_alarm`, `network_available`, `signaling_opened`, `signaling_message{text}`, `signaling_closed`, `attempt_local_description{attempt, sdp_type, sdp}`, `attempt_local_ice{attempt, candidate}`, `attempt_data_open{attempt}`, `attempt_data_message{attempt, text}`, `attempt_data_closed{attempt}`, `attempt_pc_state{attempt, state}`, `audio_level{rms}`, `device_status{…}` (baby), `set_listen{enabled}`, `request_probe`, `set_snooze{until_ms?}`, `request_quiet` (parent), `quiet_applied{…}` (baby), `tick`.

Actions: `open_signaling`, `close_signaling`, `send_signaling{text}`, `create_attempt{attempt, offerer}`, `close_attempt{attempt}`, `create_offer{attempt}`, `set_remote_description{attempt, sdp_type, sdp}`, `create_answer{attempt}`, `add_ice_candidate{attempt, candidate}`, `send_data{attempt, text}`, `set_mic_enabled{enabled}`, `apply_quiet` (baby), `emit{event}`.

Clock: monotonic milliseconds that include deep sleep (Android `SystemClock.elapsedRealtime()`; web `Date.now()` clamped to never go backwards), ticked at least every 500 ms while monitoring; Doze gaps are time jumps. The core exports `message_keys()`; the i18n check verifies each key exists in `locales/en.json`. The baby's local SDP must contain a `sendonly` audio m-line, else the core refuses it (child-side silence, ADR-018).

### 12. Tests (Stage 1 first, ADR-016)

Host simulation (`gs-scenario/1`, seeded) must pass before device runs:

- Wi-Fi off/on on the baby: 20 s outage (no level 2) and 90 s outage (level 2, then `link_restored`); the same on the parent.
- Signaling-server restart while connected (no state change) and while disconnected.
- Stale baby socket and rejoin through server liveness.
- Baby process restart; parent process restart (`request_offer`).
- Time jump of 10 min (Doze) with and without peer loss.
- Wrong PSK at first pairing (terminal after two consecutive mismatches).
- Attacks: proof reflection, forged offer with a new `aid` against a verified attempt (must not close it), spoofed mismatch after pairing (not terminal), stranger in the room, ghost parent socket.

## Options Considered

### Option A: Recovery by ICE restart on the existing peer connection

- **Pros**: no new DTLS handshake.
- **Cons**: does not survive process death or server restarts; second code path; renegotiation glare. **Deferred** (`STATUS: IDEA`).

### Option B: New attempt per recovery, supervised by a core `Link`, make-before-break (CHOSEN)

- **Pros**: one code path for all failures; pairing re-verified every time; deterministic host simulation; thin platforms.
- **Cons**: full negotiation per recovery (≈ 1–3 s); larger core API.

### Option C: Platform-side reconnect loops

- **Cons**: three implementations of the most safety-relevant logic. **Rejected.**

### Option D: Server-side baby takeover

- **Cons**: lets any room member disconnect the baby. **Rejected**; liveness (≤ 20 s) suffices.

### Option E: Keep v2 and add fields compatibly

- **Cons**: an old web parent cannot handle a re-offer on the same peer connection; a new baby loops on missing parent heartbeats; the symmetric proof stays exploitable. **Rejected** in favour of a clean v3 cutover.

## Consequences

### Positive

- AC-10 becomes implementable and verifiable on the host first; both sides know whether the other is alive.
- The pairing reflection attack is closed; room ids stop being guessable.

### Negative

- Breaking protocol change: the Capacitor app and v3 clients cannot pair with each other.
- Larger core API; the web client moves onto the WASM Link.

### Risks

- Recovery near the 60 s threshold after long Wi-Fi rejoins — handled by the grey zone and immediate retry on `network_available`.
- All-night retry loops cost battery — longer cap after 10 minutes.
- OEM process killing of the baby — parent escalation plus "tap to resume"; measured in soak tests.
