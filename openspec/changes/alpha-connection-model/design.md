# Design

## Context

See `proposal.md` (Why).

**Current state:**

- **Signaling:** one transport, our Rust relay (`signaling-relay`), with rooms, `members`/`peer` presence and an opaque `signal` payload. The relay is untrusted: the pairing proof and the DTLS fingerprint binding (`pairing`) carry confidentiality and integrity, but the relay still sees plaintext SDP and ICE candidates. Release builds ship without a signaling URL. The invite (v3) carries a URL list.
- **The core `Link`** (`crates/guardedsleep-core/src/link.rs`, about 2250 lines) is sans-IO. It has a monotonic clock that includes sleep, a `SplitMix64` PRNG seeded by `start{entropy}` for wire ids (not cryptographic), and a single `Sig` state machine bound to relay membership.
- **ICE:** only the relay's built-in STUN; third-party STUN is opt-in. TURN is T1.

**Spike:** see `spike.md` (2026-10-23). Nostr ephemeral events work as a mailbox on about 11 EU relays with p95 < 600 ms. Rate limits allow only a few events per attempt. strfry keeps ephemeral events for about 5 minutes and serves them to any subscriber.

**Owner decisions of 2026-10-23:**

- Nostr EU set as the default internet mailbox, with no project-run server.
- Automatic, disclosed EU STUN.
- A sealed envelope on every mailbox.
- No default TURN in the alpha.
- Offline LAN in the alpha, as a separate change (`add-offline-lan`, retargeted to the alpha) applied after this one.
- The baby device pins the first verified parent device.
- Internet mailboxes are closed while an attempt is verified.

**Critical review of 2026-10-23:** all CRITICAL and IMPORTANT findings are answered in the decisions below and in the review response at the end.

## Goals / Non-Goals

**Goals:**

- Pairing and monitoring without any manual address, with internet on any networks.
- No single operator whose outage stops every family.
- Mailbox operators learn only IP addresses, timing and opaque daily values.
- A relay can never mute escalation, fake "monitored", or swap the parent device.

**Non-Goals:**

- The LAN listener and the local PC mode (`add-offline-lan`). This change only defines the `lan` mailbox kind and leaves room for it.
- Project-run server or TURN.
- Web Push.
- iOS.
- Remote pairing without a QR code.
- A post-pairing key change (stays `IDEA`).
- Refreshing the lists without an app release.

## Decisions

### 1. Mailbox set in the core (layer: core decides; platforms own sockets)

The `Link` holds a set of mailboxes instead of the single `Sig` machine. Kinds:

- `relay`: our server;
- `nostr`: a public relay;
- `lan`: defined here, produced by `add-offline-lan`.

**Interface:**

- Actions: `open_mailbox(id, kind, url)`, `mailbox_send(id, frame)`, `close_mailbox(id)`.
- Inputs: `mailbox_opened(id)`, `mailbox_closed(id, reason)`, `mailbox_frame(id, bytes)`.
- The core speaks each kind's wire protocol: relay `hello`/`signal`, and NIP-01 `REQ`/`EVENT`/`OK`/`NOTICE`/`CLOSED`.

**Behaviour:**

- **Fan-out and de-duplication:** every envelope is sent to every open mailbox. Inbound envelopes are de-duplicated by envelope id (§2).
- **While an attempt is verified:**
  - The core closes all internet mailboxes. A `lan` listener stays (owner decision).
  - Every 30 minutes the core opens them briefly for a reachability check: connect plus `REQ` and `EOSE` for Nostr, `hello` and `welcome` for a relay, without publishing anything.
  - If none is reachable, the core emits a hint, `recovery_path_unavailable`. The UI shows it calmly; it is not an alarm.
- **On loss:** the core reopens every mailbox at once.
- **While not connected:** the core re-issues the subscription on each open Nostr mailbox every 60 s, because browsers cannot detect half-open sockets. This costs no events.

Attempts, liveness, escalation and listen are unchanged. Loss is still detected on the DataChannel.

- **Pros:** one relay outage is invisible, and no single operator is needed. Closing the mailboxes saves battery on old phones and denies relays an all-night connection pattern.
- **Cons:** a large refactor of `link.rs`. Recovery is about 0.3–1 s slower, because mailboxes must reopen after a loss.
- **Recommendation:** adopt, in the staged order of `tasks.md` (relay mailbox first, so the traces stay unchanged).

### 2. Sealed envelope (layer: core)

**Keys:** one HKDF-SHA256 extract over the PSK with an empty salt, then a separate expand per purpose:

| Purpose                | Info string                 | Output   |
| ---------------------- | --------------------------- | -------- |
| Envelope seal key      | `guardedsleep seal v1`      | 32 bytes |
| Nostr tag key          | `guardedsleep nostr tag v1` | 32 bytes |
| Nostr signing key seed | `guardedsleep nostr key v1` | 32 bytes |

The pairing-proof MAC, the room id and the relay-admission key keep their existing derivations. No key is used for two purposes.

**Cipher:** XChaCha20-Poly1305 (RustCrypto `chacha20poly1305`, `default-features = false`, features `alloc`). The 24-byte nonce comes from an in-core CSPRNG, `ChaCha20Rng` from `rand_chacha` with `default-features = false`.

- The CSPRNG is seeded with 32 bytes that the platform supplies in `start` (Android `SecureRandom`, web `crypto.getRandomValues`). It is deterministic in recorded traces, which keep the seed.
- `SplitMix64` stays for non-secret ids only.

**Envelope id:** the AEAD nonce, so it is authenticated implicitly.

**Associated data:** `"gs-env/1" ‖ sender_role ‖ kind_of_message`, fixed-width encoded.

**Plaintext:**

- `recipient_session` (16 bytes): the recipient's current session nonce, taken from its last `here`; all zeros for `here` itself.
- `sender_session` (16 bytes).
- The message, as the existing JSON shapes.
- Padding to a bucket ladder of 1, 4 and 8 KiB, chosen by message class.

**Receiver rules:**

- Reject envelopes that fail authentication, carry the receiver's own role, or repeat a nonce in the replay window (the last 1024 ids, or 10 minutes, whichever is larger).
- Reject any message other than `here` whose `recipient_session` is not the receiver's current session.
- The receiver draws a fresh session nonce at every `start` and process restart.

This closes the stop replay through strfry retention: a stored stop carries the old session and is ignored after a restart.

**Epoch:** checked only for Nostr addressing (§3), never on relay or LAN mailboxes. A wrong clock therefore cannot break those.

**Allowed relay-visible items, exhaustively:**

- on Nostr: the daily tag, the daily public keys, BIP-340 signatures, ciphertext under the seal key, and the event `created_at`;
- on a relay: the derived room id, as today, and the ciphertext.

The log canary covers the PSK, every derived key, the room id and plaintext SDP. The tag is excluded only for relay-visible bytes.

**AGENTS.md §7.3 amendment (exact wording for task 1.1):** "No custom transport crypto: media uses DTLS-SRTP. HKDF-SHA256 (distinct info strings per purpose) and HMAC-SHA256 only for the pairing proof, room and admission derivation, and the Nostr tag. XChaCha20-Poly1305 only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded by the platform. BIP-340 only for signing Nostr events, never for a trust decision."

- **Pros:** operators see no SDP, candidates or fingerprints. The binding to the recipient's session defeats replay of stored events. One path for all kinds.
- **Cons:** a new primitive, so it needs vectors, fuzzing and a security review. No forward secrecy: a later PSK compromise decrypts retained envelopes (SDP and IPs, never audio). That is recorded in the threat model.
- **Recommendation:** adopt.

### 3. Nostr mailbox (layer: core)

**Epoch:** days since the Unix epoch, counted from **12:00 UTC**, as a u32 big-endian. The rollover therefore falls in the afternoon in Europe, not during the night.

- The core needs a wall clock. A new input, `wall_clock(unix_s)`, is sent at `start`, with every tick, and after the platform observes a time change.
- An unknown wall clock means no Nostr mailbox is opened, and the core emits the `clock_unknown` hint. Relay mailboxes still work.

**Tag:** `base64url(HMAC-SHA256(tag_key, epoch_be32))[0..22]`.

**Signing key:** the secp256k1 scalar is `HKDF-Expand(sign_seed, "role|" ‖ role ‖ epoch_be32)`, reduced modulo n, and rejected without a panic if it is 0. BIP-340 uses `k256` with `default-features = false` and features `schnorr` and `arithmetic`.

**Subscription:**

- Filter `{kinds:[K], authors:[peer_pubkey(epoch)], "#t":[tag], since: now − 15 min}`.
- During the hour around the rollover, subscribe to both epochs.
- At rollover the core re-subscribes with the new tag and authors.
- The receiver drops stale events through the session binding of §2, not through `since`.

**Kind:** one fixed ephemeral kind K, recorded in the spec at implementation time.

- Discarded: a kind shared with other apps, which would hide the app better but mix traffic and filtering.
- Trade-off: anyone can count GuardedSleep setups on strfry relays. This is recorded in the threat model.

**Content:** base64url of the sealed envelope only. Content and tags are ASCII base64url; any other characters are rejected. This avoids divergence in NIP-01 canonical JSON. Frames are capped at 64 KiB on the platform and in the core.

**Presence:**

- A device publishes a sealed `here` (role, session nonce, capability flags):
  - at start;
  - after a network change;
  - after any mailbox reopens while the link is not connected;
  - after a received `here` that carries a session the device has not seen before;
  - while waiting, with backoff of 5, 15 and 60 s, then every 5 min.
- Over Nostr the baby starts an attempt only after a fresh parent `here`, and the parent sends `request_offer` only after a fresh baby `here`.
- The attempt schedule of `link-supervision` still applies. Nostr adds this gating so the budget holds during long outages.

**Budget and pacing:**

- At most 2 events/s per relay.
- At most 6 events per attempt and side (candidates bundled; at most 2 trickle batches).
- At most 120 events per hour per mailbox. When the cap is reached, that mailbox carries only `here` until the hour rolls over. The link keeps escalating as normal, and the cause hint names the servers.
- Inbound: at most 60 events per minute per mailbox are verified. Above that, the relay is parked.

**Parking:** a refusal (`blocked`, `restricted`, `auth-required`, `pow`, `rate-limited`), invalid or forged events, or inbound floods park a relay with backoff of 1, 5 and 30 min.

**Delivery:** an `OK` is not delivery. If an offer, answer or `request_offer` is not answered within 6 s, the same envelope (same id) is resent once to every open mailbox. The attempt is still abandoned at 20 s, as today.

### 4. Parent pin (layer: core; pairing spec)

The parent draws a random 16-byte parent id at pairing and persists it with the pairing. It is sent inside sealed envelopes and confirmed over the verified DataChannel.

- After its first verified attempt, the baby persists the parent id. On Android it is encrypted next to the PSK; on the web it is in memory only.
- From then on the baby refuses `request_offer`, offers and answers from any other parent id until "Forget".
- The invite closes after the first verification, as today, but is now enforced by the pin instead of `PARENT_CAP`.

- **Pros:** a photographed QR code or an old install cannot take over the baby during an outage.
- **Cons:** the PSK stays long-lived. Losing the real parent device means re-pairing (that is "Forget" on the baby).
- **Recommendation:** adopt. The post-pairing key change stays `IDEA`.

### 5. Invite v4 (layer: core codec)

`{"v":4,"k":<PSK>,"m":[…],"x":<expiry>}`. Each mailbox is a compact string:

- `"n:<host>"` for a Nostr relay (always `wss://<host>`);
- `"r:<host[:port][/path]>"` for an own relay (always `wss://`);
- `"l:<rfc1918-or-ula>:<port>"` for a LAN mailbox (`add-offline-lan`; IPv6 link-local is not allowed).

**Size:**

- At most 6 entries and **≤ 300 bytes** encoded, about QR version 13 at level M.
- When over budget, the baby drops entries in this order: extra own relays, then Nostr beyond 3, then Nostr down to 2. A `lan` entry and the first own relay are never dropped.
- A scan test renders the QR inverted at 50 % brightness and decodes it with zxing (Playwright and Robolectric).

**Mailbox update:** after verification the baby may send a sealed `mailboxes` message over the DataChannel. The parent replaces its stored set, so pairings survive relay churn and app updates without a re-pair.

**Version skew:** the invite and the update carry hosts. A receiver uses them even if they are not in its own shipped list, as long as they pass the kind's URL rules. Both devices also add up to 2 relays from their own list, chosen deterministically from the PSK, so devices with overlapping lists still meet.

**v3:** rejected as `not_our_code`. Nothing has been released.

### 6. Curated lists (repository `data/`; generator into both clients and the web CSP)

`data/nostr-relays.json` and `data/stun-servers.json`, seeded from `spike.md`.

**Vetting:**

- operator in the EU, EEA or CH;
- free for ephemeral events;
- accepts **8 KiB** events;
- no known logging of contents;
- one entry per operator.

**Build check:** at least 6 usable relays from at least 4 operators, and at least 2 STUN servers from distinct operators. Otherwise the build fails. Entries older than 12 months are only a warning, plus an issue opened by the scheduled job; this is not a lint failure.

**Scheduled job:** the spike scripts are committed under `scripts/spike/` and run weekly as a non-blocking job. It needs a `docs/ci-gates.md` row.

**Web CSP:** the web CSP's `connect-src` is generated from the relay list, our origin and configured own relays (amends `rebuild-web-client` "Browser security").

### 7. ICE (layer: platform builds the ICE config from core-supplied lists)

- **With an own relay mailbox configured:** only its STUN.
- **Otherwise (the default):** the first 2 STUN entries of distinct operators (Nextcloud on 443/3478 and Telekom), queried in parallel by ICE.
- Google and big-tech STUN are excluded, and a lint check fails on them in source.

**TURN:** none by default; a user-entered TURN in Advanced. An attempt that fails with no relayed candidate maps to the existing `media_blocked` cause. Its level-2 troubleshooter copy is plain: "These two devices can't reach each other from where they are." Its actions are "Try again" and "Put both on the same Wi-Fi", plus "Advanced: TURN server" for experts. It never replaces the alarm.

**Diagnostics:** a local view shows the connection type of the last attempt. It never leaves the device.

**AGENTS.md §7.1 amendment (exact wording for task 1.1):** "Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use vetted EU/EEA/CH mailboxes and STUN from `data/` (criteria in `alpha-connection-model`), with disclosure and an off switch on each device. Google and big-tech STUN are excluded."

### 8. Disclosure and off switch (layer: UI over contract models)

**Basic settings (Connection) and setup:**

- One sentence: "To find each other over the internet, your devices use a few independent services in Europe. They see this device's internet address and when it is in use, never the baby's sound."
- A "Who are they?" list that expands. Operator names come from the data files, not from locale copy.

**Advanced:** the switch "Only same Wi-Fi or my own server".

- It acts on its own device: no public relay or STUN is contacted from it.
- If the peer still announces public mailboxes, the UI shows a hint.

### 9. Cut list and go/no-go

A **go/no-go gate** sits after the core work (tasks 2–3) and before Android and web (5–6).

- **What it tests:** a CLI soak prototype (`scripts/spike/soak`) on the core runs one night on 3 EU relays, plus the spike's unknowns 1–4 (8 h subscriptions, bursts behind one CGNAT IP, tag-only filter, LTE / IPv6-only).
- **Go:** at least 2 of 3 relays without data loss and setup p95 < 3 s.
- **If no-go:** the alpha ships with relay mailboxes (self-hosted or LAN) only. Nostr moves to beta, and the owner decides again.

**Deferred regardless:**

- second-PC forwarding (`add-offline-lan`);
- refreshing the lists at run time;
- the events-per-hour cap beyond the fixed limit in §3.

**Operator contact:** an owner task before release. Contact the listed operators about acceptable use and logging, and record their answers in the data notes.

### Discarded alternatives

**From this change:**

- **Project-run public signaling instance as default:** a single operator and single point of failure, and owner workload (owner, 2026-10-23).
- **Nostr plus a project fallback instance:** the owner would still run a server.
- **Public MQTT test brokers, WebTorrent trackers, IPFS/libp2p, Matrix, ntfy.sh, Supabase/Firebase:** see `spike.md`.
- **Signaling over the QR code only:** night-time reconnects need a channel without user action.
- **Remembering the peer's public address for direct reconnects:** NAT mappings expire, CGNAT and DS-Lite block inbound connections, and UPnP is rare and risky.
- **A short authentication string instead of a mailbox:** it authenticates but does not deliver. Kept for remote pairing (`DEFERRED`).
- **Sealing only on Nostr:** two paths, and relays would still see candidates.
- **Keeping all mailboxes open while connected:** costs battery and exposes the monitoring pattern (owner, 2026-10-23).
- **Epoch checked on every mailbox:** a wrong clock on an offline phone would break LAN pairing.
- **Epoch rollover at 00:00 UTC:** falls in the middle of every European night.
- **Freshness through `since` only:** strfry retention and clock skew make it unsafe.
- **Envelope nonce supplied by the platform per message:** a sans-IO core cannot request input mid-step. The SplitMix64 PRNG is not cryptographic.
- **A cap on the parent device by the PSK alone:** a photographed QR code could take over the baby.
- **Post-pairing key change now:** stronger, but a large crypto addition for the alpha (owner, 2026-10-23; stays `IDEA`).
- **Lint failure on stale list entries:** would turn CI red by date.
- **One change for everything:** too large to review and archive (owner, 2026-10-23: split, `add-offline-lan` follows).
- **Automatic Cloudflare TURN like Signal; own EU TURN:** a US operator or a server run by the owner. Decided for beta with data.

**Carried over from `add-stun-server-choice` (withdrawn by this change):**

- Google or big-tech STUN (a third party sees IP and usage times).
- A single hard-coded fallback (no user choice, a single point of trust).
- Only manual `host:port` entry (too technical).
- "Never automatic" (superseded by the owner, 2026-10-23: automatic and disclosed).

## Risks / Trade-offs

| Risk                                                                                                   | Mitigation                                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Volunteer relays disappear or start requiring auth                                                     | 3+2 relays per pair, the mailbox update over the DataChannel, the build minimums, the weekly job, operator contact, and the go/no-go fallback to relay mailboxes |
| strfry retention                                                                                       | session-bound envelopes, daily rotation, padding                                                                                                                 |
| Per-IP rate limits behind CGNAT                                                                        | pacing, the budget, presence-gated attempts, parking with backoff                                                                                                |
| All mailboxes dead while connected                                                                     | the 30-minute reachability check and a calm hint                                                                                                                 |
| A third party counts GuardedSleep setups through the fixed kind; the 3-relay set is stable across days | recorded in the threat model; the mailbox update allows rotating the set                                                                                         |
| No forward secrecy for signaling                                                                       | recorded in the threat model; envelopes never contain audio                                                                                                      |
| Persona A (venue Wi-Fi, mobile ↔ mobile) is served only where a direct path exists                     | the proposal claims A only partly; a hardware round measures it for the beta TURN decision                                                                       |
| Larger parser surface (NIP-01, AEAD)                                                                   | parser lint denies, the ASCII-only content rule, 64 KiB caps, fuzz targets for envelopes and NIP-01 frames                                                       |

## Migration Plan

Nothing is released.

- **Order:** core (inputs, envelope, relay mailbox with sealing, invite v4, presence, Nostr) → go/no-go → server → Android → web → `add-offline-lan`.
- **Rollback:** relay mailboxes only.

## Review response (critical review 2026-10-23)

| Finding                                                                                | Where it is answered                                                                                                                                             |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Link-supervision relied on membership                                                  | `specs/link-supervision`: MODIFIED offer direction, accepted senders, scheduling, errors, causes, deliberate stop, authenticated stop, transport liveness, clock |
| No wall clock and no CSPRNG                                                            | §2, §3; `specs/link-supervision` "Clock and time jumps" and "Deterministic attempt identity"                                                                     |
| Stop replay through retention                                                          | §2 session binding; `specs/signaling-mailboxes`                                                                                                                  |
| Unauthenticated envelope id                                                            | §2                                                                                                                                                               |
| Epoch check on LAN                                                                     | §2, §3                                                                                                                                                           |
| `since` versus clock skew                                                              | §3                                                                                                                                                               |
| Parent cap without a relay room                                                        | §4; `specs/pairing`                                                                                                                                              |
| Room id in clear on the LAN                                                            | moved to `add-offline-lan` (LAN admission without the room id in clear)                                                                                          |
| §7.1 and §7.3 wording                                                                  | §2, §7 exact texts                                                                                                                                               |
| Parking per session                                                                    | §3 backoff                                                                                                                                                       |
| Budget versus "never give up"                                                          | §3 presence gating and cap behaviour                                                                                                                             |
| Key separation                                                                         | §2 table                                                                                                                                                         |
| Untestable spec wording                                                                | `specs/signaling-mailboxes` "Relay-visible data", tasks 7.1                                                                                                      |
| Overlaps with `rebuild-web-client`                                                     | its delta is edited to be version-agnostic and to allow the curated `connect-src`                                                                                |
| Missing cut list                                                                       | §9                                                                                                                                                               |
| Invite size                                                                            | §5 (≤ 300 bytes, scan test, drop order)                                                                                                                          |
| Task size and order                                                                    | `tasks.md` groups 2–4                                                                                                                                            |
| Dependency defaults                                                                    | §2, §3                                                                                                                                                           |
| Linkability, padding, link-local                                                       | §3 kind, §2 ladder, §5                                                                                                                                           |
| Scope and budget                                                                       | split; §9 deferrals; maintenance estimate of 1–2 h per month in the proposal                                                                                     |
| Freshness lint                                                                         | §6                                                                                                                                                               |
| Deleting folded changes                                                                | `add-stun-server-choice` archived as withdrawn; `add-offline-lan` retargeted                                                                                     |
| ADR-018 tester setup                                                                   | task 1.1 amends the context note and the hardware-test rounds                                                                                                    |
| Off switch placement and label                                                         | §8                                                                                                                                                               |
| "No direct connection" copy                                                            | §7                                                                                                                                                               |
| LAN address change, second PC, PC browser launch, mDNS candidates, Android permissions | `add-offline-lan`                                                                                                                                                |
| All mailboxes down while connected; causes over the set                                | §1; `specs/link-supervision` "Never give up"                                                                                                                     |
| STUN fallback versus parallel                                                          | §7, aligned with the proposal                                                                                                                                    |
| Resend versus fan-out                                                                  | §3 delivery                                                                                                                                                      |
| Untestable scenarios                                                                   | `specs/*`, with timings named                                                                                                                                    |
| Storage requirement                                                                    | `specs/pairing` "Encrypted storage"                                                                                                                              |
| Operator consent                                                                       | §9                                                                                                                                                               |
