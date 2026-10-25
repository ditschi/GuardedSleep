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

**Closed mailboxes as built (task 3.4):**

- **When:** the relay mailbox closes when the attempt becomes a _link_: verified and, on a baby with a pinned parent, after the parent proof (§4). The core clears the room membership and the confirmed peer's relay client id, with no backoff and no failure counted. The peer's confirmed session stays, so the sealed state survives the closing.
- **Check:** every 30 minutes the core opens the relay socket, sends the relay's application `ping` and closes it again: no `hello`, so no room id, no sealed envelope and no tag leave. The socket counts as reachable once it opens. A connect that fails, or does not open within 10 s, raises the hint `recovery_path_unavailable` (`LinkEvent::Hint`), and the next successful check clears it. The hint is also cleared when the attempt is lost. A platform needs nothing new: it opens, feeds and closes the socket the core asks for.
- **Reopen:** on loss of the verified attempt, and when the peer stops it (`peer_stopped`), the housekeeping of the same call reopens the mailbox. A stop while verified goes over the DataChannel only, because no mailbox is open.
- **Consequences:** a restarted peer cannot reach a verified peer until that peer's liveness timeout (25 s by default) reopens its mailbox, so recovery after a peer restart takes up to that long plus a presence round trip. While a pair is verified the room is empty, so a stranger that knows the room id can take the baby or parent slot of an own relay; the real device then gets `BABY_TAKEN` or `PARENT_CAP` on its next reconnect. This is the denial of service of §4 ("Our relay"), and escalation still runs.
- **Hints:** `clock_skew` is raised when a `here` of the peer's confirmed session carries a wall clock more than 10 minutes from ours (our last report plus the monotonic time since). The heartbeat does not carry the clock (it carries a monotonic time).
- **Mailbox update:** after every trusted verification the baby sends `LinkConfig.mailboxes` over the DataChannel as `{"type":"mailboxes"}`. The parent validates the set with the invite grammar (one to six entries, `allow_dev_loopback` as configured) and emits `LinkEvent::MailboxesUpdated` for the platform to store; a set that fails validation is dropped.

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

**Sessions (answers review 2, C2 and I1; amended by the security review of tasks 2.1–2.3, C1):**

- **Drawing:** every `Link` run draws a fresh session nonce.
- **Candidates:**
  - A `here` from a session the receiver has not confirmed makes that session a _candidate_. This holds even when the `here` names the receiver's current session.
  - The receiver answers once with its own `here`, which carries a fresh 16-byte CSPRNG **challenge** for that candidate. The challenge is single-use and valid for 60 s.
  - Replies are limited to one per candidate, at most one per 5 s, and at most 10 per 10 minutes while a confirmed session exists.
- **Confirmation:**
  - A candidate becomes the peer's _current_ session only through a `here` that echoes the open challenge issued for it.
  - That proves the peer is live now. Merely naming the receiver's session proves only that the peer was alive at some point in this run.
  - Non-`here` messages from a candidate never promote it; they are dropped.
  - A new confirmation replaces the previous current session and retires it.
  - If the confirming echo is lost, the peer repeats its `here` with its open challenge. The confirmed side answers such a repeat with its echo again: one per 5 s, within the reply budget. So a lost echo costs seconds, not the 60 s challenge lifetime.
- **Gating:** attempts and `request_offer` over Nostr are gated on a confirmed session. In the spec, "fresh" means exactly this.
- **Receiver checks** on every message, `here` included, from the confirmed session:
  - `recipient_session` must be the receiver's current session (except for an initial `here`, where it is all zeros);
  - `sender_session` must be the peer's current confirmed session;
  - `seq` must be new for that sender session (bitmap window of 256, across all classes).
- **Retirement:** a processed stop, or a replacement, retires that sender session. Retired sessions are kept for the whole run without a fixed small limit, bounded only by the replay cap below, so "retired for good" holds.
- **Replay window:** in addition, one envelope-id window per `Link` run, across all mailboxes, never cleared on a mailbox reopen: 4096 ids or 30 minutes, whichever is larger. A hard cap of 65,536 ids applies, counted only for authenticated ids.
- **Plaintext length:** a u16 length field after `seq`, inside the ciphertext, lets the receiver strip the padding.
- **Message classes and buckets:** Here and Control (`request_offer`, `stop`) 1 KiB, Description 4 KiB, Candidates 8 KiB. Here and Control share a size, so they cannot be told apart on the wire. Task 3.2 settles the sizes against real SDP and candidate sizes.
- **Error locality:** open failures never cause a reply on any mailbox (no error or timing oracle).
- **Test-only paths:** fixed-nonce sealing and key export exist only behind a `conformance` cargo feature, enabled for the test builds of the façades, never in release builds.
- **Effect:**
  - A stored stop from an earlier session is ignored, whether or not the parent restarted, and so is the mirror case.
  - A delayed or withheld `here` from a superseded session cannot displace the live session, because it cannot answer a challenge issued after its sender died.
  - An old `here` creates at most one bounded reply and never an attempt.

**Epoch:** checked only for Nostr addressing (§3), never on relay or LAN mailboxes. A wrong clock therefore cannot break those.

**Relay mailbox as built (task 3.2):**

- **Wire:** a relay `signal` payload is `{"env": "<base64url of the envelope>"}`; the relay forwards it unread. Every offer, answer, candidate, `request_offer` and `stop` goes through `SessionState`; an open error changes nothing and sends nothing.
- **Presence:** on our relay the room membership is the trigger. When a member with the peer's role appears (or the mailbox reopens with one present) the `Link` sends a sealed `here` to it, and repeats it after 5, 15 and 60 s, then every 5 minutes, while no session of that member is confirmed. A repeat also carries the last challenge we still wait on, so a lost confirming echo costs seconds, not the challenge's 60 s.
- **Ready:** attempts and `request_offer` need a confirmed peer session **and** a room member with the relay client id that session last spoke from. A member that never confirms (a stranger) gets presence only, never an offer. `peer_absent` is shown for "no confirmed peer", not only for "nobody in the room".
- **Stop:** the stop message carries nothing. A stop from the peer's confirmed session closes the attempt even when it is verified, and retires that session; the DataChannel stop retires it too. The old MAC (`gs-stop/3`), the link epoch and the stop nonce list are gone.
- **Sizes (Chrome 152, audio sendonly plus a DataChannel):** offer message 1.9 KB without and 2.5 KB with four host candidates, answer 1.7 and 2.0 KB, one ICE candidate message about 250 bytes. The Description bucket holds 4,026 bytes, so everything fits with margin. One candidate per envelope costs a whole 8 KiB bucket (about 11 KB on the wire); bundling candidates is part of the Nostr event budget (task 3.5).
- **Wrong key in a shared room:** envelopes do not authenticate, so the pairing proof is never reached; both sides keep running and escalate with `peer_absent`. The `pairing_key_mismatch` terminal stays reachable for a PSK holder that fails the proof or the DTLS binding.

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

**AGENTS.md §7.3 amendment (exact wording for task 1.1, from review 2, I8):** "No custom transport crypto: media and DataChannel use DTLS-SRTP/DTLS. HKDF-SHA256 with one info string per purpose derives every key from the PSK. HMAC-SHA256 is used only for the pairing, parent and baby proofs, the pin tags, admission and the Nostr tag. XChaCha20-Poly1305 is used only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded freshly by the platform at every start; a nonce never repeats under a key. BIP-340 is used only to sign Nostr events, never to authenticate the peer. Secret types use `secrecy`/`zeroize`."

- **Pros:** operators see no SDP, candidates or fingerprints. The binding to the recipient's session defeats replay of stored events. One path for all kinds.
- **Cons:** a new primitive, so it needs vectors, fuzzing and a security review. No forward secrecy: a later PSK compromise decrypts retained envelopes (SDP and IPs, never audio). That is recorded in the threat model.
- **Recommendation:** adopt.

### 3. Nostr mailbox (layer: core)

**Epoch:** days since the Unix epoch, counted from **12:00 UTC**, as a u32 big-endian. The rollover therefore falls in the afternoon in Europe, not during the night.

- The core needs a wall clock. A new input, `wall_clock(unix_s)`, is sent at `start`, then every 60 s, and after the platform observes a time change. Minute resolution is enough for the epoch and `created_at`, and keeps traces small.
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

**Kind:** one fixed ephemeral kind K, **24861** (task 3.5; chosen against the kinds table of the NIPs repository on 2026-10-25, where the neighbours are 24133 Nostr Connect and 24242 Blossom).

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

**Nostr mailbox as built (task 3.5):**

- **Set:** one Nostr mailbox per `n:` entry of `LinkConfig.mailboxes` (mailbox ids from 1; the relay stays 0), for both roles. The baby's own invite does not add Nostr entries before task 3.6, but the core accepts and uses them when an invite or a mailbox update brings them: the parent adopts the Nostr entries of a valid update, drops relays that are no longer named and keeps the set in its config. `LinkConfig.only_local` ("Only same Wi-Fi or my own server") makes the core ignore every `nostr` entry of the set and of updates (the UI and its hint belong to 5.3 and 6.2). Development builds also accept `n:<loopback>:<port>` (`ws://`) so the E2E can run a local test relay.
- **Opening:** a platform opens exactly the `url` of an `open_mailbox` for a Nostr mailbox and passes text frames (frames over 64 KiB are dropped on the platform and in the core). Without a wall clock no Nostr mailbox opens; after 5 s without one the hint `clock_skew` is raised (it also stays up for 30 minutes after a relay refused an event for its creation time, and while the peers' clocks differ by more than 10 minutes).
- **Wire:** one subscription per socket with a random id (16 random bytes, 22 base64url characters) and one filter per listened epoch (`kinds`, `authors`, `#t`, `since` = now − 15 min on the first request, then 30 s before the newest verified event seen); content is the base64url of the sealed envelope, one `t` tag. The listened epochs are the current one plus its neighbour within 30 minutes before or after the change. The subscription is re-issued on any change of that set and every 60 s while not connected; a relay that does not answer within 10 s (EOSE or CLOSED) counts as closed and is retried after 2, 5, 15, 60 and then 300 s. A `CLOSED` or `AUTH` from the relay parks it or closes the socket.
- **Presence:** a relay that just opened gets a `here` at once; unsolicited `here`s then go to all open relays after 5, 15 and 60 s and every 5 minutes while no link is verified. Replies to candidate sessions use the existing reply budget.
- **Freshness:** an attempt or a `request_offer` over Nostr needs a confirmed peer session that spoke over Nostr within the last 7 minutes (its `here` repeats at least every 5 minutes), so a peer that went away is not asked again and again; a stop, a new session or the closing of all Nostr mailboxes clears it.
- **Budget (per relay):** events leave at least 500 ms apart; at most 120 events in an hour window (then presence and a stop only); at most 6 setup envelopes per attempt and side, counted once for the whole set because every envelope goes to every relay (reset at a new `request_offer` and when an attempt is created; the 6 s resend counts); at most 60 inbound events per minute are looked at, the 61st parks the relay.
- **Parking:** `OK false` and `CLOSED` with blocked, restricted, auth-required, pow or rate-limited (and any other rejection that is not a duplicate), a `NOTICE` that names a rate limit, `AUTH`, three invalid or out-of-scope events within a minute and the inbound flood park a relay for 1, then 5, then 30 minutes (back to 1 after 2 hours without a park). A refusal that names the creation time (`created_at`, "too far", "future", "timestamp") raises `clock_skew` and parks nothing.
- **Delivery, for all mailbox kinds:** the last offer, answer or `request_offer` is sent once more after 6 s with the same envelope id (the relay copy is identical, the Nostr copy a new event) unless the peer went on in between (any further authenticated message other than a stop, or the DataChannel opening).
- **Candidates, for all mailbox kinds:** local ICE candidates are bundled into one `ice` message (`candidates`, at most 24; the single-candidate shape is still accepted). The first batch leaves 400 ms after the first candidate, the second no sooner than 2 s after the first; later batches use only the other mailboxes, because over Nostr at most two batches go.
- **Reachability check while verified:** every 30 minutes the relay as before, and each Nostr relay that is not parked: connect, `REQ` with `{"ids":[<random 64 hex characters>],"limit":1}` and a random subscription id (no kind, no tag, no author), and EOSE counts as reachable. Nothing of the pairing leaves (a sim assertion checks every frame sent while connected, and that neither the kind nor any tag or key of the pairing appears).
- **Hygiene after the security review of 3.5:**
  - Inbound order: a known event id (the last 256 verified ones per relay) is dropped first, before it is counted or verified; an event whose `created_at` is outside [wall − 15 min, wall + 10 min] is dropped; an out-of-scope or forged event counts toward the three that park a relay; distinct in-scope events beyond 60 a minute are dropped for the rest of the window without a penalty. Any frame over 300 a minute from one relay closes the socket (reconnect with backoff, no park).
  - Sign of life: the peer counts as present, and its `here` clock as a comparison, only when the signed `created_at` of the event that carried it is at most 7 minutes old; a stored or replayed event proves nothing about now.
  - Clock refusals are kept per relay: the hint `clock_skew` needs two relays to refuse within 30 minutes (or our own clock unknown, or the peer reporting skew); a relay that refuses three times while another relay accepted is parked.
  - Relay answers: an unsolicited `AUTH` is ignored (only `auth-required:` in an `OK` or `CLOSED` parks); `error:` closes the socket and reconnects with backoff; `mute:` is ignored.
  - Flapping: the reconnect backoff resets only after a socket stayed open for a minute (an EOSE does not reset it); the opening `here` goes to a relay at most every 3 minutes; beyond the hourly cap at most 30 presence events an hour go to one relay.
- **Evidence:** NIP-01 ids, BIP-340 signatures, tags, keys and frames are in `vectors/conformance.json`; their expected values come from a pure-Python implementation of BIP-340 and HKDF/HMAC that shares no code with the core (the first official BIP-340 vector is also a unit test). Fuzz target `nostr_frame`. Sim scenarios `nostr-*` against simulated relays (retention like strfry, accepts-but-never-delivers, rate limit, creation-time window, outage); a Node test relay and `32-web-nostr.spec.ts` for the browser.

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

**Security review of task 3.4 (2026-10-25), binding:**

- **Subkeys:** the parent secret is never used directly. `HKDF-SHA256(parent_secret, info)` derives one key each for `gs-parent-tag/1`, `gs-baby-tag/1`, `gs-parent-proof/1` and `gs-baby-proof/1`.
- **Both directions tagged (C1):** after pinning, the parent's and the baby's envelopes both carry a tag inside the ciphertext. Each side refuses untagged or wrongly tagged envelopes from the peer role before any session check. A PSK holder can therefore neither confirm a session, nor send a stop, nor retire the real peer's session once the pairing is pinned and acknowledged.
- **The tag covers the whole envelope (I1):** `HMAC(tag_key, nonce ‖ AD ‖ plaintext before the tag)[0..16]`, not the nonce alone. A re-sealed forgery under a withheld genuine nonce then fails, and verbatim replays are dropped by the replay windows.
- **The first-pairing window is bounded by the invite expiry (I2, I4):**
  - The parent persists `{secret, acked, until = invite expiry}` and sends the raw secret only before `until`.
  - After `until` it behaves as pinned (proofs only), whatever `acked` says.
  - The baby treats a pairing as complete only once it has pinned, accepts `parent_pin` only while its invite is valid, and an unpinned baby with `paired_before` grants no trust and shows "pair again".
  - The secret is never re-sent outside the window, including when the peer reports "not pinned".
- **Persist before send is binding (I3):**
  - The platform durably persists `ParentPinned` (an fsync before the rename on Android) before executing any later action of the same output.
  - The core emits `ParentPinned` again with every repeat while the pin is unacknowledged, which is idempotent.
- **Recovery:** on repeated missing baby proofs the parent says "The baby device does not recognise this device: show a new code on it". "Show new code" (physical access) is the only recovery path.
- **Hygiene:** device names are cleaned of control, format and bidi characters (Cc, Cf, Zl, Zp) before display. The `parent_pin` action is redacted in `Debug`, and the parent secret and the `parent_pin` text are on the log-canary list.

**Mutual proof (added after the 3.4 implementation review):** the baby answers with `HMAC(parent_secret, "gs-baby-proof/1|" ‖ fp_baby ‖ fp_parent)` over the DataChannel. On a pinned pairing the parent treats an attempt as trusted (connected, outage bookkeeping reset, restoration, listen, missed events) only after a valid baby proof. Without this, a PSK holder in the middle could keep the parent briefly "connected" in every cycle and hold escalation below level 2.

**Persist before send:** the parent's core emits the secret for persistence before the action that sends it, and the platform persists it before it executes later actions. A parent that restarts after sending still has the secret, so a lost acknowledgement cannot lock the real parent out.

**As built (security review follow-up):**

- `pair_again` is raised on a baby that holds no secret for a parent and whose invite window is over, as soon as an envelope of a parent arrives (its pairing never completed, or the record was lost). `baby_unrecognised` is raised on a parent whose secret the baby never acknowledged and whose window is over, after three refused baby envelopes or baby proofs in a run. An acknowledged pairing never raises it, so a PSK holder cannot push the parent into that message. Both are sticky status causes, cleared by a trusted attempt; neither changes the escalation level.
- A parent or baby without a wall clock treats a bounded window as closed: it sends no raw secret and accepts no `parent_pin` until the platform has reported the clock (the platforms do at start).
- The sim runs the window through `pin_window_s` on a peer and loses exactly the secret message with `drop_data` `message: "parent_pin"`.
- The web client keeps the same fields in memory; it has nothing to persist before sending, but its baby completes the pairing at `parent_pinned`, like Android.
- Android's pairing record is written with an fsync before the rename; a failed write throws, the runner drops the rest of the batch and the core repeats `ParentPinned` with its next resend.

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

**Parent pin as built (task 3.4):**

- **Messages (DataChannel, protocol v3):** `parent_pin {secret, name}` (parent to baby, first verification only), `pin_ack` (baby to parent), `parent_proof {mac, name}` (parent to baby, every later attempt), `mailboxes`. The secret is 32 bytes from the in-core CSPRNG, 64 hex characters on the wire. The name is cleaned (control characters removed, at most 64 characters) and is the only identifying text the parent sends; the platform supplies a model name (Android) or a browser-and-system name (web), never an identifier or a fingerprint.
- **Persist before send:** the parent draws the secret at the first verification and emits `ParentPinned` **before** the action that sends `parent_pin`. The platform must persist it before it executes the later actions (Android writes the pairing record synchronously in that order, the web keeps it in memory). A parent that restarts before the baby's acknowledgement arrives therefore still holds the secret the baby pinned and proves it like any later attempt. Within the same run the parent repeats the secret until the acknowledgement arrives (an unpinned or already pinned baby takes the same one) instead of proving it. One window remains: if the baby never received `parent_pin` at all (the DataChannel died right after the verification) and the parent restarts, the parent proves a secret the baby does not hold and is refused until the pairing is forgotten.
- **Proof:** `HMAC-SHA256(parent_secret, "gs-parent-proof/1|" ‖ fp_parent ‖ fp_baby)` over the raw 32-byte SHA-256 digests of the two DTLS certificates in the SDPs, as each side saw them (the parent's own certificate and the baby's as the parent saw it; on the baby the reverse). With `require_dtls_fingerprint` the remote one was also checked against the certificate the platform read from the WebRTC stats. A man in the middle between two DTLS sessions has two different fingerprints on the two legs, so a proof computed on one leg never matches on the other; the baby compares in constant time.
- **Mutual proof:** after the parent proof checked out, the baby sends `baby_proof = HMAC-SHA256(parent_secret, "gs-baby-proof/1|" ‖ fp_baby ‖ fp_parent)` over the same raw digests. On a pinned pairing the parent treats an attempt as untrusted until a valid baby proof arrives (constant-time check, 10 s): no connected status, no outage bookkeeping, no `link_restored`, listen or missed events, so its escalation keeps running from the last real contact however often a man in the middle cycles attempts. A missing or wrong proof closes the attempt, with the rate-limited warning `wrong_key_attempt`. In the first pairing (nothing pinned, or a secret the baby has not acknowledged) the parent trusts at once, as before. A baby that holds no pin cannot answer a pinned parent, so a re-installed baby needs the pairing forgotten on both devices.
- **Untrusted attempts:** between the DTLS-level verification and the proof (at most 10 s) the attempt is _untrusted_: the microphone path stays closed (it is also closed whenever no trusted attempt exists), no status, noise alert or missed-event summary is sent, the heartbeat carries no sound level, every message except the proof or secret is ignored, and nothing about the attempt counts as recovery, link or outage bookkeeping. A proof that arrives before the baby's own verification (the DTLS report is asynchronous) is held and delivered right after `Verified`.
- **Early refusal:** the tag `HMAC-SHA256(parent_secret, envelope_nonce)[0..16]` sits in the last 16 bytes of the plaintext (the padding region, so the buckets and wire sizes are unchanged and the largest message of each class is 16 bytes smaller). A baby with a pinned parent refuses a parent envelope without a valid tag before it touches any session state ([`OpenError::NotPinned`]), without any reply, and warns with `security_warning {unknown_parent_attempt}`, at most once per 5 minutes. The tag depends on the nonce only: a PSK holder that rewrites an envelope may reuse its nonce and keep a valid tag, which is why the proof exists.
- **Persistence:** `LinkEvent::ParentPinned` is the one event that carries a secret; platforms persist it like the PSK and pass it back as `LinkConfig.pinned_parent`. `Debug` of the secret, the config and the event is redacted, the core wipes its copy of the config field, and the Android runner wipes its own copy after the core took it. The web keeps it in memory with the PSK.

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

**Development builds (as built in 3.3):** `parse_invite` and `build_invite` take an explicit `allow_dev_loopback` flag, off by default. With it on, an `l:` entry may also name `127.0.0.0/8`, `[::1]` or `localhost`, so `just dev` and the e2e suite (`ws://127.0.0.1:…`) work. Android passes `BuildConfig.DEBUG` and the web passes `import.meta.env.DEV`, so release builds refuse loopback. The local PC mode puts the PC's LAN address in an `l:` entry instead (`add-offline-lan`).

**QR rendering:** a 4-module quiet zone (ISO/IEC 18004) on both platforms. A fully used invite (one own relay, three Nostr relays, one LAN entry; 284 bytes) reaches QR version 12 at level M, and 300 bytes reaches version 13.

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

**As built (task 3.6):**

- **Data:** `data/nostr-relays.json` (11 relays, 11 operators; `nos.lol` and `nostr.mom` are one operator and one entry) and `data/stun-servers.json` (10 entries from 9 operators; Nextcloud twice, on 443 and 3478). Each entry has operator, country, privacy policy (or null), `last_vetted` and notes; relays also have `hoster`, software and `keeps_ephemeral`. A `status` of `suspended` keeps an entry in the file without shipping or counting it.
- **Generator and build check:** the `public-lists` crate (`crates/public-lists`, outside the core) loads the files with a typed, `deny_unknown_fields` loader and `check` enforces the entry rules (EU/EEA/CH country, `https://` policy link, lower-case DNS hosts, one active relay per operator, no duplicates, no future dates) and the minimums (6 usable relays from 4 operators, 2 STUN operators); a `last_vetted` older than 12 months only warns. `web` writes the TypeScript module and the CSP origins, `android` writes `PublicLists.kt`; nothing generated is committed. `just lint` runs `check` and `stun-sources`, which fails on any STUN host outside the list in `apps/`, `crates/`, `docker/`, `scripts/` and `.github/` (a built-in self-test proves that `stun.l.google.com` is caught before the scan runs).
- **Relay pick:** `guardedsleep_core::relay_pick`. The platform passes its shipped list as `RelayCandidate { host, operator, hoster }`. Each candidate scores `HMAC-SHA256(K, lower-case host)` with `K = HKDF(PSK, "guardedsleep relay pick v1")`; the lowest scores win (rendezvous hashing, so a list that differs by one entry moves at most one pick). `pick_relays` walks the ranking, skips parked hosts and any operator already taken, and prefers a hoster not yet taken, relaxing only that preference when slots would stay empty. `extra_relays` runs the same walk over the device's own list for the up to 2 extra relays beyond the invite's hosts. Conformance vectors `relay pick: …` run through every binding.
- **ICE:** the generator resolves the default set (every entry of the first two distinct STUN operators: Nextcloud 443 and 3478, Telekom) into `DEFAULT_STUN_URLS` / `PublicLists.defaultStunUrls`. The platforms use it only when no own relay is configured; with `only_local` they use no public STUN.
- **Web CSP:** `connect-src` is `'self'` plus the generated relay origins, `connect-origins.json` and `GS_CSP_CONNECT_ORIGINS`. The web build writes `csp-connect-origins.json` (read by `guardedsleep-server --serve-web`) and `csp-header.conf` (the nginx `add_header` line, which the image moves to `/etc/nginx` and four `include`s pick up), so the header, the `<meta>` fallback and the server all follow the same list.

### 7. ICE (layer: platform builds the ICE config from core-supplied lists)

- **With an own relay mailbox configured:** only its STUN.
- **Otherwise (the default):** the first 2 STUN entries of distinct operators (Nextcloud on 443/3478 and Telekom), queried in parallel by ICE.
- Only STUN hosts in `data/stun-servers.json` are allowed, enforced by a lint check on client source and configuration.

**TURN:** none by default; a user-entered TURN in Advanced. An attempt that fails with no relayed candidate maps to the existing `media_blocked` cause. Its level-2 troubleshooter copy is plain: "These two devices can't reach each other from where they are." Its actions are "Try again" and "Put both on the same Wi-Fi", plus "Advanced: TURN server" for experts. It never replaces the alarm.

**Diagnostics:** a local view shows the connection type of the last attempt. It never leaves the device.

**AGENTS.md §7.1 amendment (exact wording for task 1.1, from review 2, I8):** "Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` (criteria: `openspec/specs/signaling-mailboxes`, "Curated public services"), only as sealed envelopes, so operators see only the items in "Relay-visible data", with disclosure and an off switch on each device. Only STUN hosts in `data/stun-servers.json` are allowed, enforced by lint."

### 8. Disclosure and off switch (layer: UI over contract models)

**Release gate (task 3.6, security review of 3.5, M7):** until the disclosure and the off switch exist, a release build must not put `n:` entries into invites, because that would send connection metadata to third parties without the user seeing or refusing it. One build-time flag per platform decides: Android `PublicRelays.enabledFor(BuildConfig.DEBUG)` (on for debug builds and the emulator gate, off for release) and the web `publicRelaysEnabled(import.meta.env)` (on for `vite dev` and `--mode e2e`, off for `--mode production`, which the release build and the web image use). A test asserts the release value is off on each platform. Tasks 5.3 and 6.2 flip these flags when the disclosure and the off switch land. The dev and E2E builds additionally let a set `guardedsleep:dev-nostr-relays` list (even an empty one) replace the public relays, so no test reaches a public relay.

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
