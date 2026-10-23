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
- The baby device pins the first verified parent device with a parent secret that travels only over the verified DTLS DataChannel (owner, 2026-10-23, second review), and shows the parent device's name with "Not my device".
- Internet mailboxes are closed while an attempt is verified.

**Critical reviews of 2026-10-23:** two passes. Every CRITICAL and IMPORTANT finding is answered in the decisions below and in the review-response tables at the end.

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

- **Fan-out and de-duplication:** every envelope is sent to every open mailbox. Inbound envelopes are de-duplicated by envelope id only **after** they authenticate and pass the session checks (§2), so a corrupted copy from one relay cannot block the genuine copies.
- **While an attempt is verified:**
  - The core closes all internet mailboxes. A `lan` listener stays (owner decision).
  - Every 30 minutes the core opens them briefly for a reachability check without any pairing identifier: for Nostr, a connect plus a `REQ` with a random throwaway filter, waiting for `EOSE`; for a relay, a connect and a WebSocket ping. Nothing is published.
  - A stop sent while verified goes over the DataChannel only, because no internet mailbox is open.
  - If none is reachable, the core emits a hint, `recovery_path_unavailable`. The UI shows it calmly; it is not an alarm.
- **On loss, and on entering `peer_stopped`:** the core reopens every mailbox at once, because it then has to hear the peer's new session.
- **While not connected:** the core re-issues the subscription on each open Nostr mailbox every 60 s, because browsers cannot detect half-open sockets. This costs no events.

Attempts, liveness, escalation and listen are unchanged. Loss is still detected on the DataChannel.

- **Pros:** one relay outage is invisible, and no single operator is needed. Closing the mailboxes saves battery on old phones and denies relays an all-night connection pattern.
- **Cons:** a large refactor of `link.rs`. Recovery is about 0.3–1 s slower, because mailboxes must reopen after a loss.
- **Recommendation:** adopt, in the staged order of `tasks.md` (relay mailbox first, so the traces stay unchanged).

### 2. Sealed envelope (layer: core)

**Keys:** one HKDF-SHA256 extract over the PSK with an empty salt, then a separate expand per purpose:

| Purpose                | Info string                  | Output   |
| ---------------------- | ---------------------------- | -------- |
| Envelope seal key      | `guardedsleep seal v1`       | 32 bytes |
| Nostr tag key          | `guardedsleep nostr tag v1`  | 32 bytes |
| Nostr signing key seed | `guardedsleep nostr key v1`  | 32 bytes |
| Relay pick seed        | `guardedsleep relay pick v1` | 32 bytes |

- The room id and the relay-admission key keep their existing derivations on the same PRK.
- The raw PSK is also the HMAC key of the existing pairing proof. That predates this change; it is noted, and no new use of the raw PSK is added.

**CSPRNG and seed:**

- The CSPRNG is `chacha20` 0.10 with its `rng` feature (`ChaCha20Rng`), already in the lockfile through `rand 0.10`, so no duplicate `rand_core` appears.
- It is seeded with 32 bytes in `LinkConfig.seed`, so construction fails without one. The platform draws a **fresh seed for every `Link` run** (Android `SecureRandom`, web `crypto.getRandomValues`). Reusing a seed would repeat nonces under a long-lived key.
- The seed is secret-class: it predicts every nonce, session nonce and parent secret. Real seeds never appear in release or gate logs or traces. Only simulation traces with test PSKs record them, and the log canary covers seeds.
- Session nonces, envelope nonces, BIP-340 `aux_rand`, the parent id and the parent secret all come from the CSPRNG. `SplitMix64` stays for the non-secret wire `aid` only.

**Cipher:** XChaCha20-Poly1305 (RustCrypto `chacha20poly1305`, `default-features = false`, features `alloc`). Task 2.2 checks on the registry that the release is on the same RustCrypto generation as `hkdf 0.13`, `hmac 0.13` and `sha2 0.11`.

**Envelope id:** the AEAD nonce, so it is authenticated implicitly.

**Associated data:** `"gs-env/1" ‖ sender_role ‖ message_class`, one byte each after the version string.

- The header in clear is the version byte, role and class, followed by the nonce, the ciphertext and the tag.
- Role and class are visible in the header (listed in "Relay-visible data"). They are not secret: the event's author key already reveals the role on Nostr.

**Plaintext:**

- `recipient_session` (16 bytes): all zeros for `here`, otherwise as below.
- `sender_session` (16 bytes).
- `seq` (u32): per sender session, starting at 0.
- The message, as the existing JSON shapes.
- Padding to a bucket ladder of 1, 4 and 8 KiB, chosen by message class. A message that does not fit its class's largest bucket is not sent: the core bundles fewer candidates per envelope instead. That is a sender bug, covered by a test.

**Sessions (answers review 2, C2 and I1):**

- **Drawing:** every `Link` run draws a fresh session nonce.
- **Candidates:** a session the receiver has not seen in a `here` becomes a _candidate_. The receiver answers once with its own `here`, whose `recipient_session` is the candidate. Replies are limited to one `here` per candidate and at most one per 5 s overall.
- **Confirmation:** the candidate becomes the peer's _current_ session only when an envelope arrives from it that carries the receiver's own current session as `recipient_session`. That proves the peer is live now.
- **Gating:** attempts and `request_offer` over Nostr are gated on a confirmed session. In the spec, "fresh" means exactly this.
- **Receiver checks** on every message other than `here`:
  - `recipient_session` must be the receiver's current session;
  - `sender_session` must be the peer's current confirmed session;
  - `seq` must be new for that sender session (bitmap window of 256).
- **Retirement:** a processed stop retires that sender session for good. Later `here` messages naming it are ignored, and the receiver keeps a bounded set of the last 16 retired sessions.
- **Replay window:** in addition, one envelope-id window per `Link` run, across all mailboxes, never cleared on a mailbox reopen: 4096 ids or 30 minutes, whichever is larger.
- **Effect:**
  - A stored stop from an earlier baby session is ignored, whether or not the parent restarted, and so is the mirror case.
  - An old `here` creates at most one candidate reply and never an attempt.

**Epoch:** checked only for Nostr addressing (§3), never on relay or LAN mailboxes. A wrong clock therefore cannot break those.

**Allowed relay-visible items, exhaustively (testable capture list):**

- **On Nostr:**
  - the kind K;
  - the event id;
  - `created_at`;
  - the daily public key;
  - the daily tag;
  - the BIP-340 signature;
  - random subscription ids;
  - filter fields (`kinds`, `authors`, `#t`, `since`, plus random throwaway filters for probes);
  - the envelope header (version, role, class), the nonce and the ciphertext with its tag.
- **On our relay:**
  - `hello`/`welcome`/`members`/`peer` with the derived room id, role, protocol version and client ids;
  - `signal` with `to`/`from` and the sealed envelope;
  - `ping`/`pong`;
  - error codes.

The log canary covers the PSK, every derived key, the seed, the parent secret, the room id and plaintext SDP. The tag is excluded only for relay-visible bytes.

**AGENTS.md §7.3 amendment (exact wording for task 1.1, from review 2, I8):** "No custom transport crypto: media and DataChannel use DTLS-SRTP/DTLS. HKDF-SHA256 with one info string per purpose derives every key from the PSK. HMAC-SHA256 is used only for the pairing and parent proofs, admission and the Nostr tag. XChaCha20-Poly1305 is used only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded freshly by the platform at every start; a nonce never repeats under a key. BIP-340 is used only to sign Nostr events, never to authenticate the peer. Secret types use `secrecy`/`zeroize`."

- **Pros:** operators see no SDP, candidates or fingerprints. The binding to the recipient's session defeats replay of stored events. One path for all kinds.
- **Cons:** a new primitive, so it needs vectors, fuzzing and a security review. No forward secrecy: a later PSK compromise decrypts retained envelopes (SDP and IPs, never audio). That is recorded in the threat model.
- **Recommendation:** adopt.

### 3. Nostr mailbox (layer: core)

**Epoch:** days since the Unix epoch, counted from **12:00 UTC**, as a u32 big-endian. The rollover therefore falls in the afternoon in Europe, not during the night.

- The core needs a wall clock. A new input, `wall_clock(unix_s)`, is sent at `start`, with every tick, and after the platform observes a time change.
- The core cannot tell that its own clock is wrong. It therefore compares clocks with the peer: `here` and the DataChannel heartbeat carry the sender's wall clock. If the two differ by more than 10 minutes, the core raises the hint `clock_skew` on both devices. That is testable.
- The `since` filter limits the tolerated skew to ±15 minutes.
- A relay refusal that names `created_at` (too old or too new) raises `clock_skew` and does not park the relay. Spike unknown 9 measures the `created_at` tolerance per relay software.

**Tag:** `base64url(HMAC-SHA256(tag_key, epoch_be32))[0..22]`.

**Signing key:** the secp256k1 scalar is `HKDF-Expand(sign_seed, "role|" ‖ role ‖ epoch_be32)`, reduced modulo n (bias about 2^-128, accepted) into a `NonZeroScalar`. If it is zero, there is no Nostr key for that epoch and role, and the device uses its other mailboxes. BIP-340 uses `k256` with `default-features = false` and features `schnorr` and `arithmetic`, with `aux_rand` from the CSPRNG.

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
  - once in reply to a new candidate session (§2), rate-limited;
  - while waiting, with backoff of 5, 15 and 60 s, then every 5 min.
- Over Nostr the baby starts an attempt only for a confirmed parent session, and the parent sends `request_offer` only to a confirmed baby session (§2).
- The attempt schedule of `link-supervision` still applies. Nostr adds this gating so the budget holds during long outages.

**Budget and pacing:**

- At most 2 events/s per relay.
- At most 6 events per attempt and side (candidates bundled; at most 2 trickle batches).
- At most 120 events per hour per mailbox. When the cap is reached, that mailbox carries only `here` until the hour rolls over. The link keeps escalating as normal, and the cause hint names the servers.
- Inbound: at most 60 events per minute per mailbox are verified. Above that, the relay is parked.

**Parking:** a refusal (`blocked`, `restricted`, `auth-required`, `pow`, `rate-limited`), invalid or forged events, or inbound floods park a relay with backoff of 1, 5 and 30 min.

**Delivery:** an `OK` is not delivery. If an offer, answer or `request_offer` is not answered within 6 s, the same envelope (same envelope id) is resent once to every open mailbox. On Nostr it is wrapped in a new event with a new `created_at`, because relays drop an identical event id as a duplicate. The attempt is still abandoned at 20 s, as today.

### 4. Parent pin with a parent secret (layer: core; pairing spec)

The first review's design used a bearer parent id inside the envelopes. A PSK holder can decrypt those, so review 2 (C1) showed the pin was ineffective. The owner chose the parent secret (2026-10-23).

**At the first verification:**

- The parent draws a random 32-byte `parent_secret` from the CSPRNG and sends it **only over the verified DTLS DataChannel**, never in an envelope.
- The baby confirms receipt and emits `parent_pinned`. The platform persists the secret: on Android encrypted next to the PSK, on the web in memory only. There a reload means a new pairing, as for the PSK.
- The parent persists the secret in the same way.

**Every later attempt:**

- Before the baby opens its microphone path for the parent, the parent proves itself over the DataChannel with `HMAC(parent_secret, "gs-parent-proof/1|" ‖ fp_parent ‖ fp_baby)`.
- The baby checks it in constant time with its own view of both fingerprints.
- A PSK holder who saw every relayed envelope cannot produce it. A PSK holder in the middle of two DTLS sessions is detected, because the fingerprints differ.

**Early refusal:**

- Envelopes from a pinned parent also carry `HMAC(parent_secret, envelope_nonce)[0..16]` inside the ciphertext.
- The baby refuses other parents before attempting, which saves budget, and emits `security_warning` for each refused parent, rate-limited.

**Interface:**

- `LinkConfig.pinned_parent: Option<ParentSecret>` and `LinkEvent::ParentPinned { secret }`, persisted as `paired_before` is today.
- It is listed in the bindings and in `api-snapshot`.

**What the baby shows:**

- After pinning, the parent device's name, sent over the DataChannel: "Connected to <name>".
- A "Not my device" action, which leads to "Forget".
- The copy says that forgetting on the parent alone leaves the baby pinned until it is forgotten there too.

**"Show new code" while paired:** a new PSK replaces the old pairing and pin only after the new verification. Until then the old pairing keeps monitoring.

**Our relay:** `PARENT_CAP` stays. A PSK holder can still squat the single parent slot on an own relay in setups that use only their own server. That is a denial of service only, because escalation still runs, and it is documented.

- **Pros:**
  - A photographed QR code, an old install or a PSK-holding relay cannot take over the baby after the first pairing.
  - It also detects a man in the middle on later attempts.
- **Cons:**
  - The first pairing still trusts whoever verifies first within the 5-minute invite. The name display and "Not my device" make that visible.
  - The PSK stays long-lived.
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

**Version skew:** the invite and the update carry hosts.

- **Android:** uses a received host even if its own shipped list lacks it, as long as the host passes the kind's URL rules.
- **Web:** uses only hosts its CSP allows (§6), and ignores the rest, which is accepted skew.
- **Both:** add up to 2 relays from their own list, chosen deterministically from the relay-pick seed, so devices with overlapping lists still meet.

**v3:** rejected as `not_our_code`. Nothing has been released.

### 6. Curated lists (repository `data/`; generator into both clients and the web CSP)

`data/nostr-relays.json` and `data/stun-servers.json`, seeded from `spike.md`.

**Vetting:**

- operator in the EU, EEA or CH;
- free for ephemeral events;
- accepts **16 KiB** events (an 8 KiB bucket is about 11–12 KB as an event);
- no known logging of contents;
- one entry per operator.

**Build check:** at least 6 usable relays from at least 4 operators, and at least 2 STUN servers from distinct operators. Otherwise the build fails. Entries older than 12 months are only a warning, plus an issue opened by the scheduled job; this is not a lint failure.

**Scheduled job:** the spike scripts are committed under `scripts/spike/` and run weekly as a non-blocking job. It needs a `docs/ci-gates.md` row.

**Web CSP (answers review 2, I9):**

- `connect-src` is `'self'`, plus the shipped relay list, plus the own relays configured **for the deployment**.
- The server's `--serve-web` and the nginx image emit the header from deployment settings, with the `<meta>` fallback for the defaults.
- The web client offers own relays only from the deployment list, so the user cannot type arbitrary origins, and it ignores invite hosts outside its CSP.
- A blanket `wss:` is never used.

### 7. ICE (layer: platform builds the ICE config from core-supplied lists)

- **With an own relay mailbox configured:** only its STUN.
- **Otherwise (the default):** the first 2 STUN entries of distinct operators (Nextcloud on 443/3478 and Telekom), queried in parallel by ICE.
- Only STUN hosts in `data/stun-servers.json` are allowed, enforced by a lint check on client source and configuration.

**TURN:** none by default; a user-entered TURN in Advanced. An attempt that fails with no relayed candidate maps to the existing `media_blocked` cause. Its level-2 troubleshooter copy is plain: "These two devices can't reach each other from where they are." Its actions are "Try again" and "Put both on the same Wi-Fi", plus "Advanced: TURN server" for experts. It never replaces the alarm.

**Diagnostics:** a local view shows the connection type of the last attempt. It never leaves the device.

**AGENTS.md §7.1 amendment (exact wording for task 1.1, from review 2, I8):** "Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` (criteria: `openspec/specs/signaling-mailboxes`, "Curated public services"), only as sealed envelopes, so operators see only the items in "Relay-visible data", with disclosure and an off switch on each device. Only STUN hosts in `data/stun-servers.json` are allowed, enforced by lint."

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

- **A bearer parent id inside envelopes:** a PSK holder decrypts it and copies it (review 2, C1).
- **A short code compared on both screens at first pairing:** it would also catch a man in the middle at the first pairing, but costs a setup step. The owner chose the name display with "Not my device" (2026-10-23).
- **Checking only `recipient_session`:** an old authentic stop from an earlier sender session would still mute escalation (review 2, C2).
- **A blanket `wss:` in the web CSP:** it would let an injected script connect anywhere.
- **Reachability probes with the pairing's tag and authors:** they would reveal the all-night monitoring pattern (review 2, I11).

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

## Review response (second critical review 2026-10-23, security sections)

| Finding                                 | Where it is answered                                                                                                                                                                                            |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 bearer parent id                     | §4 parent secret over DTLS and the parent proof; `specs/pairing`                                                                                                                                                |
| C2 sender session not checked           | §2 sessions (confirmed sender session, retirement on stop); `specs/signaling-mailboxes` "Session-bound freshness"                                                                                               |
| I1 replayed `here`                      | §2 candidate and confirmation, rate-limited replies, gating on a confirmed session                                                                                                                              |
| I2 de-duplication before authentication | §1; `specs/signaling-mailboxes` "Parallel mailboxes"                                                                                                                                                            |
| I3 window too short or reset            | §2 one window per run (4096 ids or 30 min) plus a `seq` bitmap per session                                                                                                                                      |
| I4 wall clock                           | §3 clock comparison with the peer, `clock_skew` hint, `created_at` refusals not parked; spike unknown 9                                                                                                         |
| I5 pin interface                        | §4 `LinkConfig.pinned_parent`, `LinkEvent::ParentPinned`; task 3.4                                                                                                                                              |
| I6 first-pairing race                   | §4 name display, "Not my device", `security_warning`, "Show new code" rule                                                                                                                                      |
| I7 seed and CSPRNG                      | §2 seed in `LinkConfig`, fresh per run, secret-class, `chacha20` `rng`                                                                                                                                          |
| I8 guardrail wording                    | §2 and §7 exact texts                                                                                                                                                                                           |
| I9 CSP versus skew                      | §5, §6                                                                                                                                                                                                          |
| I10 LAN cleartext contradiction         | `add-offline-lan` design and task 2.2                                                                                                                                                                           |
| I11 probes reveal the pattern           | §1 throwaway probes                                                                                                                                                                                             |
| M1–M7                                   | §2 (oversize, header), §3 (`NonZeroScalar`, `aux_rand`, new event on resend), §1 (`peer_stopped` reopens; a stop while verified goes over the DataChannel), `add-offline-lan` (admission squat, PC key storage) |
