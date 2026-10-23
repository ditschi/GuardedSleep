# Design

## Context

See `proposal.md` (Why). Current state:

- **Signaling:** one transport, our Rust relay (`signaling-relay`), with rooms, `members`/`peer` presence and an opaque `signal` payload. The relay is untrusted: confidentiality and integrity come from the pairing proof and the DTLS fingerprint binding (`pairing`). Today it still sees the plaintext SDP and ICE candidates.
- **Configuration:** release builds ship without a signaling URL. The v3 invite carries a URL list.
- **ICE:** only the relay's built-in STUN by default; third-party STUN is opt-in (`add-stun-server-choice`, not built). TURN is T1.
- **Offline LAN:** prepared in `add-offline-lan` (beta). Its decisions (embedded listener, opaque NSD, local PC mode, admission, private-address cleartext) are kept and move here.
- **Spike:** see `spike.md` (2026-10-23). Nostr ephemeral events work as a mailbox on about 11 EU relays (p95 < 600 ms). Rate limits allow only a few events per attempt. strfry keeps ephemeral events for about 5 minutes. No free EU TURN exists.

Owner decisions of 2026-10-23:

| Topic               | Decision                                                                                                                         |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Internet rendezvous | Nostr EU set as default, no project-run server                                                                                   |
| STUN fallbacks      | Automatic, from a vetted EU list, disclosed                                                                                      |
| Signaling payloads  | End-to-end sealed on every mailbox                                                                                               |
| TURN                | None by default in the alpha; user-entered TURN only; decide for beta with data (recommended, awaiting the owner's confirmation) |

## Goals / Non-Goals

**Goals:**

- Pairing and monitoring without any manual address, at home, offline on one Wi-Fi, phone ↔ PC, and across networks.
- No single operator whose outage stops every family.
- Mailbox operators learn no more than IP addresses, timing and an opaque address that rotates daily.

**Non-Goals:**

- A project-run public server or TURN (the alpha has none).
- Background wake-up push.
- iOS.
- Remote pairing without a QR code.
- Refreshing the relay list without an app release (`IDEA`).

## Decisions

### 1. Parallel mailboxes, owned by the core (layer: core; platforms own sockets)

The `Link` holds a set of mailbox transports instead of one signaling transport, each with its own connection state. The kinds:

- `relay`: our server, internet or self-hosted;
- `lan`: the baby device's embedded listener or a local PC relay;
- `nostr`: a public relay.

The core emits `open_mailbox(id)`, `send(mailbox_ids, envelope)` and `close_mailbox(id)`, and accepts `mailbox_opened/closed/received(id, bytes)`. Every outgoing envelope goes to all open mailboxes. Inbound envelopes are de-duplicated by envelope id, so whichever mailbox delivers first wins.

Attempts, liveness and escalation are unchanged. A network change remains `network_available` → fresh attempt, now over whichever mailboxes are still open. The platform still runs no reconnect loops (`link-supervision`).

- **Pros:** LAN and internet work at the same time with no mode switch. Wi-Fi ↔ mobile data recovers automatically. One relay outage is invisible.
- **Cons:** more connections per device (3–5 sockets). The core state grows.
- **Recommendation:** adopt.

### 2. Sealed signaling envelope (layer: core)

Every signaling message on every mailbox kind (offer, answer, candidates, presence, stop) is sealed:

- **Cipher:** XChaCha20-Poly1305 (RustCrypto `chacha20poly1305`).
- **Key:** `HKDF-SHA256(PSK, info "guardedsleep signal seal v1")`.
- **Nonce:** a random 24-byte nonce supplied by the platform. The core stays sans-IO: randomness is an input, as for the PSK today.
- **Associated data:** version, sender role, epoch and attempt id.
- **Padding:** every plaintext is padded to a 4 KiB bucket, so message types cannot be told apart by length.

The receiver rejects envelopes from its own role, from outside the current epoch ± grace, and envelope ids it has already seen (replay window). The pairing proof and the DTLS binding stay as they are. The envelope adds confidentiality and replay protection on top; it is not a substitute.

This amends AGENTS.md §7.3 ("HMAC/HKDF only for the pairing proof and room derivation"): a standard AEAD from RustCrypto for the signaling envelope is allowed. It is not transport crypto, because media stays on DTLS-SRTP.

- **Pros:** operators no longer see SDP, LAN addresses or fingerprints. Replayed or reflected messages are rejected before the attempt logic. A single path for all mailbox kinds.
- **Cons:** a new crypto use that needs a Security Architect review and test vectors. Plaintext debugging of signaling is gone (debug builds may log decrypted envelopes behind the existing `GS_POC` gate, never the key).
- **Recommendation:** adopt.

### 3. Nostr mailbox (layer: core builds and verifies events; platform holds the WebSockets)

| Item         | Rule                                                                                      |
| ------------ | ----------------------------------------------------------------------------------------- |
| Epoch        | The UTC day; both sides also listen to the neighbouring epoch for ±15 min around midnight |
| Address      | Tag `t = base64url(HMAC-SHA256(PSK, "gs-mailbox/1                                         | " ‖ epoch))[0..22]`                                           |
| Signing keys | Per role and epoch: `sk = HKDF(PSK, "gs-nostr-key/1                                       | " ‖ role ‖ epoch)`, BIP-340 via RustCrypto `k256` (`schnorr`) |
| Subscription | Filter on the peer's public key, our kind and `#t`, with `since` = now − 60 s             |
| Event kind   | One fixed ephemeral kind (25xxx, not shared with other apps' known kinds)                 |
| Content      | The sealed envelope, base64                                                               |
| Fan-out      | Each device publishes to the 3 relays the invite names (§5) and subscribes on all of them |

**Budget:** relays carry presence and attempt setup only.

- An attempt bundles candidates: offer and answer with gathered candidates, at most 2 trickle batches, at most 6 events per side.
- Presence ("here") is sent at start, after a network change, and while unpaired-in-room with backoff of 5 s, 15 s, 60 s and then every 5 min.
- No heartbeats go through Nostr: liveness runs on the DataChannel, as today.
- The core enforces an events-per-hour cap per mailbox.

**Relays that misbehave:**

- A relay that answers `blocked`, `restricted`, `auth-required`, `pow` or `rate-limited` is parked for the session.
- A relay's `OK` is not taken as delivery. The peer's answer is the acknowledgement, and an unanswered offer is resent on the other mailboxes.

**Presence replaces `members`/`peer`.** A sealed `here` envelope (role, a fresh session nonce, capabilities) tells the peer to start or answer an attempt. The one-parent cap now rests on the PSK: only PSK holders can produce valid envelopes, and the parent persists the pairing after its first verified attempt (`pairing`).

- **Pros:** no project server. About 10 independent EU operators. Prior art exists (Trystero, voistr). The scheme is protocol-compatible with any NIP-01 relay.
- **Cons:**
  - Volunteer infrastructure with churn and rate limits.
  - Every relay used sees both IPs and timing.
  - Two new core dependencies (`k256` with `schnorr`, `chacha20poly1305`); the dependency-count guard rises and is justified in the PR.
- **Recommendation:** adopt. Our relay remains the mailbox for LAN, PC mode and self-hosting.

### 4. Invite v4 (layer: core codec from `rebuild-web-client`)

The v4 invite is `{"v":4,"k":<PSK>,"m":[<mailbox>…],"x":<expiry>}`. Each mailbox is `{"t":"relay"|"lan"|"nostr","u":<ws:// or wss:// URL>}`.

The baby device fills the list:

- its LAN listener address, or the PC relay's address in local mode, when there is one;
- any configured own relay;
- 3 Nostr relays picked from the shipped list.

Room ids and Nostr tags are derived, never carried.

**Size:** at most 6 mailboxes and 480 bytes, so the QR code stays scannable at level M when inverted at ≤ 50 % brightness.

**v3 handling:** v3 is dropped (BREAKING), which is fine because nothing has been released. The core codec now builds and parses v4 only, with new conformance vectors. A v3 code is `not_our_code`.

- **Pros:** the parent learns every path from one scan, so no settings are needed.
- **Cons:** a larger QR code. The relay choice is frozen at pairing time; a re-pair refreshes it.
- **Recommendation:** adopt.

### 5. Curated lists as data (repository: `data/`; generated into both clients)

`data/nostr-relays.json` and `data/stun-servers.json` hold, for each entry: URL, operator, country, privacy-policy link, software, last-vetted date and measured notes.

**Vetting criteria:**

- Operator in the EU, EEA or CH.
- No payment, membership or whitelist needed for ephemeral events.
- Accepts 4 KiB events.
- No known logging of event contents.
- Distinct operators: one entry per operator; nos.lol and nostr.mom count as one.

**Selection:**

- The baby device picks 3 relays per pairing, spread over different operators and hosters (deterministic from the PSK, so retries pick the same ones), and skips relays the current session has parked.
- A non-blocking scheduled CI job re-measures the lists (the spike scripts, kept in `scripts/spike/`). It needs a `docs/ci-gates.md` row.
- Re-vetting happens quarterly; an entry older than 12 months is dropped by the generator.

- **Pros:** an update is a small reviewed change. Users and auditors see exactly who is used.
- **Cons:** a stale list until the next release. Monthly churn needs about 1 h per quarter.
- **Recommendation:** adopt.

### 6. ICE: automatic EU STUN, no default TURN (layer: platform builds the ICE config from core-supplied lists)

**STUN:** the ICE servers are the own relay's STUN when a relay mailbox is configured, plus 2 entries from the STUN list (Nextcloud on 443/3478 and Telekom first, chosen for distinct operators). ICE queries all of them in parallel, so the list is kept short to limit who sees the IP.

**Disclosure:** the setup screen and Settings → Connection name the STUN and Nostr operators: "These services see this device's internet address and when it connects. They never hear the baby." The switch "Use only my own server" turns both off.

This amends AGENTS.md §7.1 ("public STUN is opt-in only") into: vetted EU STUN and mailboxes are allowed by default with disclosure and an off switch; Google and big-tech STUN stay excluded.

**TURN:**

- No default TURN in the alpha. The existing Advanced "TURN server" setting takes a user-entered `turn:`/`turns:` URL with credentials.
- When ICE fails with no relayed candidate, the parent shows "No direct connection between these networks" with two actions: "use the same Wi-Fi" or "add a TURN server".
- A local diagnostics view (Settings → Connection → Details) shows the selected candidate-pair type of the last attempt (host, srflx or relay). It never leaves the device.
- Decide project TURN (Cloudflare with a project account and a credential Worker, or an EU VPS) for beta with that data.

**Alternatives considered (TURN):**

| Option                                | Pros           | Cons                                                                             |
| ------------------------------------- | -------------- | -------------------------------------------------------------------------------- |
| Automatic Cloudflare TURN like Signal | best reach     | US operator; a project account and a credential service; abuse and cost exposure |
| Own EU TURN                           | best reach, EU | a server the owner must run                                                      |
| No TURN (recommended for the alpha)   | no operation   | about 10–25 % of cross-network pairs fail, with clear copy                       |

### 7. Offline LAN in the alpha (layers: platform listener and NSD, server local mode)

The decisions of `add-offline-lan` are taken over unchanged:

- embedded listener on the Android baby device, only while monitoring, with Wi-Fi and multicast locks;
- opaque per-session NSD token;
- local PC mode with loopback-only UI and API and `Origin`/`Host` checks;
- relay admission on every LAN relay;
- the private-address cleartext rule.

The listener is one more mailbox kind (`lan`), so it runs in parallel with the internet mailboxes. A LAN-only session works without internet.

`add-offline-lan` is withdrawn once its specs live here. `add-stun-server-choice` is withdrawn too: its list moves here, its settings become the disclosure and the off switch.

### 8. Guardrail and spec updates (AGENTS.md §7, §9)

- **§7.1:** vetted EU mailboxes and STUN by default with disclosure and an off switch; no third party in the media path except user-chosen TURN.
- **§7.3:** sealed signaling envelope allowed (standard AEAD, HKDF from the PSK).
- **§7.8:** applies to the LAN listener in the alpha.
- **§9:** "Offline LAN" becomes alpha, and "LAN ↔ Internet automatic handoff" is covered by parallel mailboxes. Remove the TURN row's alpha restriction only for user-entered TURN.

### Discarded alternatives

- **Project-run public signaling instance as default:** a single operator and single point of failure, and owner workload (owner, 2026-10-23).
- **Nostr plus a project fallback instance:** better availability, but the owner would run a server anyway.
- **Public MQTT test brokers, WebTorrent trackers, IPFS/libp2p, Matrix, ntfy.sh, Supabase/Firebase:** see `spike.md` (operators discourage reliance, too few operators, heavy, account-bound, or a cloud vendor).
- **Signaling over the QR code only (manual two-way exchange):** reconnects at night need a channel without user action.
- **Remembering the peer's public address for direct reconnects:** NAT mappings expire, CGNAT and DS-Lite prevent inbound connections, and UPnP is rare and risky.
- **A short authentication string (emojis) instead of a mailbox:** it authenticates but does not deliver messages. It is kept for remote pairing (`DEFERRED`).
- **Sealing only on Nostr:** two code paths, and our own relay would keep seeing candidates.
- **Keeping invite v3 with a flat URL list:** cannot say which URL is LAN, relay or Nostr, and so cannot apply the cleartext and admission rules per kind.

## Risks / Trade-offs

| Risk                                                                          | Mitigation                                                                                                                        |
| ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| strfry relays keep events for about 5 min and serve them to anyone            | sealed envelopes, daily-rotating tags and keys, padding                                                                           |
| Rate limits, and shared CGNAT IPs exhausting per-IP limits                    | a per-mailbox event budget, bundled candidates, parking relays, 3 relays per pair, and our relay as the fallback for self-hosters |
| Relay churn: a relay the invite named disappears                              | the other two relays carry on; a re-pair picks fresh ones; the copy on persistent failure suggests re-pairing                     |
| Silent drops (`OK` without delivery)                                          | application-level acks (answer to offer, answer to presence) and resend on the other mailboxes                                    |
| Every relay used and the STUN operators learn IPs and timing (sleep patterns) | EU operators only, 3 relays, daily rotation, disclosure, and "Use only my own server"                                             |
| Clock skew around the epoch change                                            | ±15 min dual-epoch listening                                                                                                      |
| Mailbox outage at night                                                       | link supervision escalates exactly as for a relay outage today; no new alarm logic                                                |
| Cross-network pairs without a direct path fail in the alpha                   | honest copy, the local diagnostics, and a TURN decision for beta                                                                  |
| Larger core attack surface (event parsing, AEAD)                              | parser lint denies, proptests, conformance vectors through every binding, a fuzz target for envelope and event parsing            |

## Migration Plan

Nothing is released, so there is no data migration.

- **Order of work:** core (envelope, Nostr, mailbox set, invite v4) → relay and server → Android → web. LAN pieces in parallel after the mailbox set.
- **Rollback:** the own-relay mailbox alone reproduces today's behaviour, with "Use only my own server" on.

## Open Questions

- **Exact ephemeral kind number:** pick one not in a registry or in known apps, and record it in the spec at implementation time. It does not change the approach.
