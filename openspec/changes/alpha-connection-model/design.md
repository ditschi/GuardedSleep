# Design

## Context

See `proposal.md` (Why).

**State before this change:**

- **Signaling:** one transport, our Rust relay (`signaling-relay`), with rooms, `members`/`peer` presence and an opaque `signal` payload. The relay is untrusted: the pairing proof and the DTLS fingerprint binding (`pairing`) carry confidentiality and integrity, but the relay saw plaintext SDP and ICE candidates. Release builds shipped without a signaling URL. The invite (v3) carried a URL list.
- **The core `Link`** (`crates/guardedsleep-core/src/link.rs`) is sans-IO. It has a monotonic clock that includes sleep, a `SplitMix64` PRNG seeded by `start{entropy}` for wire ids (not cryptographic), and a single `Sig` state machine bound to relay membership.
- **ICE:** only the relay's built-in STUN; third-party STUN is opt-in. TURN: a user-entered server (`add-turn-credentials`, archived 2026-11-02).

**State of this change on 2026-11-02:** tasks 1–3 and 4.1 are built on `main`: the envelope, the mailbox abstraction with the relay mailbox, invite v4, sessions, presence and the parent pin, **and** the Nostr mailbox, the curated lists, the relay pick and the soak tooling (`crates/gs-soak`, `scripts/spike/`, `relays-weekly.yml`). The owner-run go/no-go (4.2) and the operator contact (4.3) never ran.

**Owner decisions of 2026-11-02 (reverse the 2026-10-23 decision "no project-run server"):**

1. **One project-run server is the default internet mailbox:** `gs.mydata.rocks` (DNS only at Cloudflare, no proxy; CNAME → `contabo.ditscher.me`, A `167.86.115.143`, AAAA `2a02:c207:3004:4659::1`). Reason: volunteer relays mean operator outreach, list churn, rate limits, soak tooling and a lot of core code; one own server with sealed envelopes leaks the same metadata (IP address and timing) to fewer parties.
2. **Nostr is removed completely**, not parked: the core Nostr mailbox, the NIP-01 codec, BIP-340/`k256`, the daily epoch, budgets and parking, the relay pick, `data/nostr-relays.json`, `crates/gs-soak`, `scripts/spike/*`, `relays-weekly.yml`, the soak and relays-check recipes, `docs/contributing/relay-soak.md`, the `nostr_frame` fuzz target, and the Nostr parts of the Android and web drivers and of the E2E suite. **Kept:** the sealed envelope, the mailbox abstraction (`relay` and `lan`), invite v4 and the parent pin.
3. **TURN is in the alpha, on the same server:** coturn on host networking serves STUN and TURN on 3478 (`guardedsleep-server` runs with `STUN_DISABLED=1` there; the built-in STUN stays for self-hosting and the local PC mode). Short-lived HMAC credentials (`--use-auth-secret`, TTL ≤ 10 min) are issued by the signaling server, only to a room with two members. The server also hands out the TURN and STUN URLs, so only the signaling host is baked into the apps. The abuse limits of `docker/turn/compose.yaml` stay.
4. **Deployment:** Traefik comes from the owner's homelab repository (`compose/contabo/traefik`, copied from `compose/oracle/traefik`, access log off, pinned tag, external network `traefik`). The GuardedSleep stack (server and coturn, optionally web) lives in this repository as a documented example in the owner's Oracle style, deployed by a clone on the Contabo host (`git pull && docker compose up -d`). Monitoring: an Uptime Kuma check from the owner's Oracle host.
5. **Owner tasks:** privacy policy and imprint for the public service, no IP logging, the firewall.

**Owner decisions of 2026-10-23 that still hold:**

- A sealed envelope on every mailbox.
- Offline LAN in the alpha, as a separate change (`add-offline-lan`) applied after this one.
- The baby device pins the first verified parent device with a parent secret that travels only over the verified DTLS DataChannel (second review), and shows the parent device's name with "Not my device".
- Internet mailboxes are closed while an attempt is verified.

**Reversed 2026-10-23 decisions** (kept under "Discarded alternatives"): the Nostr EU set as the default internet mailbox with no project-run server; automatic, disclosed third-party EU STUN; no default TURN in the alpha.

**Critical reviews:** two passes on 2026-10-23 and one on 2026-11-02 for this revision. Every CRITICAL and IMPORTANT finding is answered in the decisions below and in the review-response tables at the end.

## Goals / Non-Goals

**Goals:**

- Pairing and monitoring without any manual address, with internet on any networks, including networks without a direct path (TURN).
- Mailbox and TURN operators learn only IP addresses, timing and opaque values; the project server is the only such party by default, and it is disclosed and can be switched off.
- A relay can never mute escalation, fake "monitored", or swap the parent device.
- Low upkeep: one server the owner already runs, no list of volunteer operators.

**Non-Goals:**

- The LAN listener and the local PC mode (`add-offline-lan`). This change only defines the `lan` mailbox kind and leaves room for it.
- Failover to a second project instance (`IDEA`).
- Web Push.
- iOS.
- Remote pairing without a QR code.
- A post-pairing key change (stays `IDEA`).
- `turns:` on port 443 behind Traefik (`IDEA`; the alpha offers `turns:` on 5349).

## Decisions

### 1. Mailbox set in the core (layer: core decides; platforms own sockets)

The `Link` holds a set of mailboxes instead of the single `Sig` machine. Kinds:

- `relay`: a GuardedSleep signaling server, the project server (§3) or an own relay;
- `lan`: defined here, produced by `add-offline-lan`.

The `nostr` kind is removed (§11; history under "Discarded alternatives").

**Interface:**

- Actions: `open_mailbox(id, kind, url)`, `mailbox_send(id, frame)`, `close_mailbox(id)`.
- Inputs: `mailbox_opened(id)`, `mailbox_closed(id, reason)`, `mailbox_frame(id, bytes)`.
- The core speaks the relay wire protocol (`hello`/`welcome`/`signal`/`ice`).

**Behaviour:**

- **Fan-out and de-duplication:** every envelope is sent to every open mailbox. Inbound envelopes are de-duplicated by envelope id only **after** they authenticate and pass the session checks (§2), so a corrupted copy from one mailbox cannot block the genuine copies.
- **While an attempt is verified:**
  - The core closes all internet mailboxes. A `lan` listener stays (owner decision).
  - Every 30 minutes the core opens them briefly for a reachability check without any pairing identifier: a connect and the relay's application `ping`, no `hello`. Nothing is published.
  - A stop sent while verified goes over the DataChannel only, because no internet mailbox is open.
  - If none is reachable, the core emits a hint, `recovery_path_unavailable`. The UI shows it calmly; it is not an alarm.
- **On loss, and on entering `peer_stopped`:** the core reopens every mailbox at once, because it then has to hear the peer's new session.

Attempts, liveness, escalation and listen are unchanged. Loss is still detected on the DataChannel.

**Closed mailboxes as built (task 3.4):**

- **When:** the relay mailbox closes when the attempt becomes a _link_: verified and, on a baby with a pinned parent, after the parent proof (§4). The core clears the room membership and the confirmed peer's relay client id, with no backoff and no failure counted. The peer's confirmed session stays, so the sealed state survives the closing.
- **Check:** every 30 minutes the core opens the relay socket, sends the relay's application `ping` and closes it again: no `hello`, so no room id, no sealed envelope and no tag leave. The socket counts as reachable once it opens. A connect that fails, or does not open within 10 s, raises the hint `recovery_path_unavailable` (`LinkEvent::Hint`), and the next successful check clears it. The hint is also cleared when the attempt is lost. A platform needs nothing new: it opens, feeds and closes the socket the core asks for.
- **Reopen:** on loss of the verified attempt, and when the peer stops it (`peer_stopped`), the housekeeping of the same call reopens the mailbox. A stop while verified goes over the DataChannel only, because no mailbox is open.
- **Consequences:** a restarted peer cannot reach a verified peer until that peer's liveness timeout (25 s by default) reopens its mailbox, so recovery after a peer restart takes up to that long plus a presence round trip. While a pair is verified the room is empty, so a stranger that knows the room id can take the baby or parent slot of a relay (the project server or an own relay); the real device then gets `BABY_TAKEN` or `PARENT_CAP` on its next reconnect. This is the denial of service of §4 ("Our relay"), and escalation still runs.
- **Hints:** `clock_skew` is raised when a `here` of the peer's confirmed session carries a wall clock more than 10 minutes from ours (our last report plus the monotonic time since). The heartbeat does not carry the clock (it carries a monotonic time). The hint stays after the Nostr removal (§11), because the pin window (§4) compares the baby's invite expiry with the parent's wall clock; the Nostr-only sources of the hint (a relay refusing `created_at`, no wall clock 5 s after start for a Nostr mailbox) go.
- **Mailbox update:** after every trusted verification the baby sends `LinkConfig.mailboxes` over the DataChannel as `{"type":"mailboxes"}`. The parent validates the set with the invite grammar (one to six entries, `allow_dev_loopback` as configured) and emits `LinkEvent::MailboxesUpdated` for the platform to store; a set that fails validation is dropped.

- **Pros:** an own relay or a LAN mailbox next to the project server makes the outage of either invisible. Closing the mailboxes saves battery on old phones and denies the server an all-night connection pattern. The abstraction costs little now that only two kinds remain, and it is what `add-offline-lan` needs.
- **Cons:** a large refactor of `link.rs` (done). Recovery is about 0.3–1 s slower, because mailboxes must reopen after a loss. With default settings there is one internet mailbox, so its outage stops internet recovery (§3, risks).
- **Recommendation:** keep, as built.

### 2. Sealed envelope (layer: core)

**Keys:** one HKDF-SHA256 extract over the PSK with an empty salt, then a separate expand per purpose:

| Purpose           | Info string            | Output   |
| ----------------- | ---------------------- | -------- |
| Envelope seal key | `guardedsleep seal v1` | 32 bytes |

- The room id and the relay-admission key keep their existing derivations on the same PRK.
- The Nostr tag key (`guardedsleep nostr tag v1`), the Nostr signing key seed (`guardedsleep nostr key v1`) and the relay pick seed (`guardedsleep relay pick v1`) are removed with Nostr (§11). Their info strings are retired and never reused for another purpose.
- The raw PSK is also the HMAC key of the existing pairing proof. That predates this change; it is noted, and no new use of the raw PSK is added.

**CSPRNG and seed:**

- The CSPRNG is `chacha20` 0.10 with its `rng` feature (`ChaCha20Rng`), already in the lockfile through `rand 0.10`, so no duplicate `rand_core` appears.
- It is seeded with 32 bytes in `LinkConfig.seed`, so construction fails without one. The platform draws a **fresh seed for every `Link` run** (Android `SecureRandom`, web `crypto.getRandomValues`). Reusing a seed would repeat nonces under a long-lived key.
- The seed is secret-class: it predicts every nonce, session nonce and parent secret. Real seeds never appear in release or gate logs or traces. Only simulation traces with test PSKs record them, and the log canary covers seeds.
- Session nonces, envelope nonces, the parent id and the parent secret all come from the CSPRNG. `SplitMix64` stays for the non-secret wire `aid` only.

**Cipher:** XChaCha20-Poly1305 (RustCrypto `chacha20poly1305`, `default-features = false`, features `alloc`). Task 2.2 checks on the registry that the release is on the same RustCrypto generation as `hkdf 0.13`, `hmac 0.13` and `sha2 0.11`.

**Envelope id:** the AEAD nonce, so it is authenticated implicitly.

**Associated data:** `"gs-env/1" ‖ sender_role ‖ message_class`, one byte each after the version string.

- The header in clear is the version byte, role and class, followed by the nonce, the ciphertext and the tag.
- Role and class are visible in the header (listed in "Relay-visible data"). They are not secret: the relay's `hello` already names the role.

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
- **Gating:** attempts and `request_offer` are gated on a confirmed session (on our relay additionally on room membership, below). In the spec, "fresh" means exactly this.
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

**Wall clock:** no mailbox depends on it. It is used only for the invite expiry, the pin window (§4) and the `clock_skew` comparison, so a wrong clock cannot break signaling.

**Relay mailbox as built (task 3.2):**

- **Wire:** a relay `signal` payload is `{"env": "<base64url of the envelope>"}`; the relay forwards it unread. Every offer, answer, candidate, `request_offer` and `stop` goes through `SessionState`; an open error changes nothing and sends nothing.
- **Presence:** on our relay the room membership is the trigger. When a member with the peer's role appears (or the mailbox reopens with one present) the `Link` sends a sealed `here` to it, and repeats it after 5, 15 and 60 s, then every 5 minutes, while no session of that member is confirmed. A repeat also carries the last challenge we still wait on, so a lost confirming echo costs seconds, not the challenge's 60 s.
- **Ready:** attempts and `request_offer` need a confirmed peer session **and** a room member with the relay client id that session last spoke from. A member that never confirms (a stranger) gets presence only, never an offer. `peer_absent` is shown for "no confirmed peer", not only for "nobody in the room".
- **Stop:** the stop message carries nothing. A stop from the peer's confirmed session closes the attempt even when it is verified, and retires that session; the DataChannel stop retires it too. The old MAC (`gs-stop/3`), the link epoch and the stop nonce list are gone.
- **Sizes (Chrome 152, audio sendonly plus a DataChannel):** offer message 1.9 KB without and 2.5 KB with four host candidates, answer 1.7 and 2.0 KB, one ICE candidate message about 250 bytes. The Description bucket holds 4,026 bytes, so everything fits with margin. One candidate per envelope costs a whole 8 KiB bucket (about 11 KB on the wire); candidates are therefore bundled into one `ice` message (as built in task 3.5, kept for all mailbox kinds).
- **Wrong key in a shared room:** envelopes do not authenticate, so the pairing proof is never reached; both sides keep running and escalate with `peer_absent`. The `pairing_key_mismatch` terminal stays reachable for a PSK holder that fails the proof or the DTLS binding.

**Allowed relay-visible items, exhaustively (testable capture list):**

- **On a relay (the project server or an own relay):**
  - `hello`/`welcome`/`members`/`peer` with the derived room id, role, protocol version and client ids;
  - `signal` with `to`/`from` and the sealed envelope (header: version, role, class; the nonce; the ciphertext with its tag);
  - `ice` sent by the relay (§7): ICE URLs and a short-lived TURN credential;
  - `ping`/`pong`;
  - error codes.
- **On the TURN server:** the TURN user name (`<expiry>:<random id>`), the client's addresses and ports, the permitted peer addresses (the other device's candidates), and the volume and timing of DTLS-SRTP and DTLS packets it relays. Never the PSK, the room id or plaintext media.

The log canary covers the PSK, every derived key, the seed, the parent secret, the room id, plaintext SDP and the TURN credential (password) a relay issued.

**AGENTS.md §7.3 amendment as applied in task 1.1 (from review 2, I8; superseded by the next paragraph once Nostr is removed):** "No custom transport crypto: media and DataChannel use DTLS-SRTP/DTLS. HKDF-SHA256 with one info string per purpose derives every key from the PSK. HMAC-SHA256 is used only for the pairing, parent and baby proofs, the pin tags, admission and the Nostr tag. XChaCha20-Poly1305 is used only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded freshly by the platform at every start; a nonce never repeats under a key. BIP-340 is used only to sign Nostr events, never to authenticate the peer. Secret types use `secrecy`/`zeroize`."

**AGENTS.md §7.3 after the Nostr removal (exact wording for task 8.6, 2026-11-02):** "No custom transport crypto: media and DataChannel use DTLS-SRTP/DTLS. HKDF-SHA256 with one info string per purpose derives every key from the PSK. HMAC-SHA256 only for the pairing, parent and baby proofs, the pin tags and admission. XChaCha20-Poly1305 only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded freshly by the platform at every start; a nonce never repeats under a key. HMAC-SHA1 only in the signaling server, to issue coturn REST credentials (coturn's format), never in the core or the clients. Secret types use `secrecy`/`zeroize`."

- **Pros:** operators see no SDP, candidates or fingerprints. The binding to the recipient's session defeats replay of stored events. One path for all kinds, and the project server is no more trusted than any relay.
- **Cons:** a new primitive, so it needs vectors, fuzzing and a security review. No forward secrecy: a later PSK compromise decrypts retained envelopes (SDP and IPs, never audio). That is recorded in the threat model.
- **Recommendation:** adopt.

### 3. Project signaling server as the default internet mailbox (owner, 2026-11-02)

**Default:** a baby device with default settings puts `r:gs.mydata.rocks/signaling` into its invite and its mailbox set (`wss://gs.mydata.rocks/signaling`). The host comes from `data/project-server.json` (§6), so it is one value in the repository, not a literal in each client.

**What the server is:** the ordinary `guardedsleep-server` in Open mode (`signaling-relay` "Relay admission": no challenge on the public server), behind the owner's Traefik, with coturn next to it (§7, §9). It carries only sealed envelopes (§2) and is exactly as untrusted as any own relay: the pairing proof, the DTLS fingerprint binding, the session binding and the parent pin hold against it.

**What its operator learns:** the IP addresses of both devices, when they pair, recover and check reachability, the derived room id (stable per pairing), the role, and the size class of each envelope; with TURN (§7) also the relayed volume and timing. This is the same metadata a Nostr operator saw, now seen by one party instead of three to five, and the party is the project itself, named in the disclosure (§8).

**Outage:** while the server is down, verified monitoring continues (internet mailboxes are closed while verified, §1) but a lost link cannot recover over the internet: the parent escalates normally and the troubleshooter shows `server_unreachable`. Mitigations:

- the Uptime Kuma check from the owner's Oracle host alerts the owner (§9);
- an own relay in Advanced adds a second internet mailbox, and the LAN mailbox (`add-offline-lan`) recovers at home without any server;
- the mailbox update (§5) lets a later release move the pairing to another host without a re-pair;
- the domain `mydata.rocks` and the Contabo contract are owner tasks with renewal reminders (§10).

**Off switch:** "Only same Wi-Fi or my own server" (§8) removes the project server from the device's set, its invites and the mailbox updates it accepts, and with it the project STUN and TURN.

- **Pros:** no volunteer operators, no list churn, no rate limits outside our control, no soak tooling, and the largest single block of core code (Nostr) goes. Fewer parties see metadata. TURN for Persona A comes with it at no extra party.
- **Cons:** the owner runs a public service (privacy policy, imprint, monitoring, abuse handling). One server is a single point of failure for internet recovery for every family on default settings. The bus factor is one person's domain and server.
- **Recommendation:** adopt (owner decision). Revisit a second instance for beta with the alpha's uptime data (`IDEA` in `docs/backlog.md`).

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

- `"r:<host[:port][/path]>"` for a relay: the project server (`r:gs.mydata.rocks/signaling`, §3) or an own relay (always `wss://`);
- `"l:<rfc1918-or-ula>:<port>"` for a LAN mailbox (`add-offline-lan`; IPv6 link-local is not allowed).

**Size:**

- At most 6 entries and **≤ 300 bytes** encoded, about QR version 13 at level M.
- When over budget, the baby drops own relays from the last one backwards. A `lan` entry, the project server entry and the first own relay are never dropped.
- The `n:` kind (Nostr) is removed (§11). An invite with an `n:` entry is rejected as `not_our_code`; nothing with `n:` was released.
- A scan test renders the QR inverted at 50 % brightness and decodes it with zxing (Playwright and Robolectric).

**Mailbox update:** after verification the baby may send a sealed `mailboxes` message over the DataChannel. The parent replaces its stored set, so pairings survive a move of the project server to another host, a changed own relay and app updates without a re-pair.

**Version skew:** the invite and the update carry hosts.

- **Android:** uses a received relay host as long as it passes the kind's URL rules.
- **Web:** uses only hosts its CSP allows (§6), and ignores the rest, which is accepted skew.
- The relay pick and the extra relays from a device's own list are removed with Nostr (§11).

**v3:** rejected as `not_our_code`. Nothing has been released.

**Development builds (as built in 3.3):** `parse_invite` and `build_invite` take an explicit `allow_dev_loopback` flag, off by default. With it on, an `l:` entry may also name `127.0.0.0/8`, `[::1]` or `localhost`, so `just dev` and the e2e suite (`ws://127.0.0.1:…`) work. Android passes `BuildConfig.DEBUG` and the web passes `import.meta.env.DEV`, so release builds refuse loopback. The local PC mode puts the PC's LAN address in an `l:` entry instead (`add-offline-lan`).

**QR rendering:** a 4-module quiet zone (ISO/IEC 18004) on both platforms. A fully used invite (one own relay, three Nostr relays, one LAN entry; 284 bytes) reached QR version 12 at level M, and 300 bytes reaches version 13. The default invite after this revision (the project server and one LAN entry) is about 120 bytes, so the 300-byte budget leaves room for several own relays.

### 6. Shipped data (repository `data/`; generator into both clients and the web CSP)

**`data/project-server.json` (new; replaces `data/nostr-relays.json`):** one entry with the signaling host and path (`gs.mydata.rocks`, `/signaling`), the operator (the GuardedSleep project, run by the owner), the country (DE, the Contabo data centre), the hoster, the privacy policy URL and the imprint URL (§10) and `last_vetted`. The `public-lists` generator emits from it:

- the default mailbox entry (`r:gs.mydata.rocks/signaling`) for both clients;
- the operator line of the disclosure (§8), so operator names never live in locale copy;
- the web CSP `connect-src` origin (`wss://gs.mydata.rocks`).

**`data/stun-servers.json` (kept as the STUN allowlist):** STUN hosts the clients may contact. After this revision no client uses a third-party STUN server by default (§7); the file holds the project host and stays the lint's allowlist ("Only STUN hosts in `data/stun-servers.json`", AGENTS.md §7.1). The third-party EU entries (Nextcloud, Telekom, …) are removed from the shipped defaults; whether they stay in the file as allowed-but-unused entries is decided in task 9.1 by one rule: an entry stays only if a client can actually reach it, otherwise it goes (the file must not suggest a use that does not exist). The build minimum "2 STUN servers from distinct operators" goes with the default.

**Build check:** the generator fails without exactly one active project server entry with an `https://` privacy policy and imprint URL. A `last_vetted` older than 12 months only warns, as before.

**Web CSP (answers review 2, I9; unchanged in shape):**

- `connect-src` is `'self'`, plus the project server origin, plus the own relays configured **for the deployment**.
- The server's `--serve-web` and the nginx image emit the header from deployment settings, with the `<meta>` fallback for the defaults.
- The web client offers own relays only from the deployment list, so the user cannot type arbitrary origins, and it ignores invite hosts outside its CSP.
- A blanket `wss:` is never used.
- ICE servers are not governed by `connect-src`; the host-binding rule of §7 restricts them instead.

### 7. ICE and TURN from the signaling server (layers: server issues; core validates and selects; platform builds the ICE config)

**Source of ICE servers, in order:**

1. **ICE servers a relay mailbox supplied** (below): STUN and TURN on that relay's own host. For the project server: `stun:gs.mydata.rocks:3478`, `turn:gs.mydata.rocks:3478?transport=udp`, `turn:gs.mydata.rocks:3478?transport=tcp` and, once certificates are in place (task 10.2), `turns:gs.mydata.rocks:5349?transport=tcp`.
2. **A relay that supplies nothing** (an older or self-hosted server without the setting): `stun:<relay host>:3478`, the built-in STUN, as today (`relay-stun`).
3. **A user-entered TURN server** (`device-setup` "TURN server entry") is added by the platform next to the above, as today.
4. **With "Only same Wi-Fi or my own server" and no own relay:** no STUN and no TURN at all; host candidates only.

No client contacts a third-party STUN server by default any more; the curated EU STUN default is discarded (see "Discarded alternatives"). Only hosts of the pairing's relays, a user-entered TURN host and the hosts in `data/stun-servers.json` may appear in client ICE configuration, enforced by the existing lint (`stun-sources`, extended to `turn:` and `turns:` URLs in client source).

**Wire (relay protocol v3, additive):**

- `welcome` gains an optional `ice` field with STUN URLs only (no credential), so a lone member can gather server-reflexive candidates.
- A new server message `{"type":"ice","urls":[…],"username":"<expiry>:<id>","credential":"<base64>","ttl":<s>}` carries TURN. The relay sends it to **each** member, with a credential of its own, when the room holds a baby and a parent, **before** the `members`/`peer` update that shows the room complete, so a client always has it before presence can start an attempt. While both stay joined, the relay sends a fresh one at half the TTL. It is never sent into a room with one member.
- A client never asks for credentials; there is no request message to abuse.

**Credential (coturn REST scheme, `--use-auth-secret`):**

- User name `<expiry unix s>:<id>`, where `id` is 16 random bytes in base64url, **never** the room id or anything derived from the PSK (coturn logs user names at some levels).
- Password `base64(HMAC-SHA1(TURN_SECRET, username))`, coturn's fixed format. HMAC-SHA1 is used only in the server for this purpose (AGENTS.md §7.3 wording in §2).
- TTL from `TURN_TTL_S`, default 600, clamped to at most 600 s.
- `TURN_SECRET` (32 random bytes, hex) is shared by the server and coturn through the deployment `.env` (§9), never committed, never in a binary. The server holds it in a `secrecy` type and never logs it; the issued password is on the log-canary list.
- Without `TURN_SECRET` and `TURN_URLS` the server sends no `ice` message; with only `STUN_URLS` it sends the `welcome` field only.

**Gate ("only a room with two members"):** the server cannot see the pairing proof, and relayed media needs TURN **before** the DTLS handshake can verify anything, so issuing "after the pairing proof" (backlog `IDEA`, 2026-10-25) is impossible; it is discarded. The gate is:

- a room holding exactly one baby and one parent, each joined with `hello` and a room id of the derived shape (v3);
- the existing join and new-room rate limits per IP (ADR-020 §6);
- issuance per member at most once per half TTL;
- coturn quotas sized for audio (below).

Residual risk, accepted (owner, 2026-10-25 and 2026-11-02): two self-made clients can open a room with any room id of the right shape and get credentials. The quotas make that a slow, rate-limited 1 Mbit/s relay that lives 10 minutes per credential, bounded by `total-quota`.

**Core (sans-IO):**

- The core parses `welcome.ice` and `ice` frames of a relay mailbox with the parser lint denies: at most 8 URLs, schemes `stun:`, `turn:`, `turns:` only, every URL host exactly equal (lower-case) to that mailbox's host, user name and credential at most 128 ASCII characters, `ttl` 1–600. A frame that fails is dropped as a whole.
- **Host binding:** a relay can never point clients at another host's TURN, so a compromised or malicious relay cannot move media metadata to a third party.
- It stores the latest valid set per mailbox with its monotonic receipt time, and attaches to every new attempt the ICE servers of the set that is still valid for at least 30 s (receipt + TTL − 30 s by the monotonic clock). Expired sets are dropped. Attempts over a `lan` mailbox get none.
- The platform builds the WebRTC ICE configuration from the attempt's servers plus a user-entered TURN. Credentials are redacted in `Debug`, never logged, never persisted, never put into the invite or the mailbox update.

**Allocation lifetime past the credential TTL (to verify, task 9.4):** a relayed call must survive an 8-hour night, but the credential expires after 10 minutes and a running `RTCPeerConnection` cannot take new credentials without an ICE restart. Expected behaviour: coturn checks the REST timestamp when it derives the key for the session's first authenticated request and keeps the session key, so `Refresh`, `CreatePermission` and `ChannelBind` keep working after the expiry. Task 9.4 proves it with the gate's coturn: an allocation made with a 60-second credential still relays after 15 minutes. **If it does not,** relayed media would drop every TTL; the fallback is a TTL of 12 hours per credential with the same gate, and the owner decides (recorded in this section).

**coturn profile (task 10.1; abuse limits carried over from `docker/turn/compose.yaml`):**

- `--use-auth-secret --static-auth-secret=${TURN_SECRET}`, `--realm=gs.mydata.rocks`, `--fingerprint`;
- host networking; on Contabo the public IPv4 and IPv6 sit on the interface (no 1:1 NAT), so `--listening-ip` and `--relay-ip` are the public addresses and no `--external-ip` mapping is needed;
- `--no-tcp-relay`, `--no-multicast-peers`, `--stale-nonce=600`, `--user-quota`, `--total-quota`, `--max-bps` as in `docker/turn/compose.yaml`;
- the `--denied-peer-ip` ranges of `docker/turn/compose.yaml`, **plus the host's own public IPv4 and IPv6**, so TURN can never relay into Traefik, the signaling server or any other service on the host;
- the relay port range from `.env` (default 49160–49359, 200 ports, about 100 relayed pairs), matched by the firewall;
- logging: no client IP address in any log (§9); the profile is checked by a log test in task 10.1.

**TURN:** the troubleshooter copy for `media_blocked` stays plain: "These two devices can't reach each other from where they are." Its actions are "Try again" and "Put both on the same Wi-Fi", plus "Advanced: TURN server" for experts. With the project TURN, `media_blocked` should become rare; the hardware round measures it. It never replaces the alarm.

**Diagnostics:** a local view shows the connection type of the last attempt (direct on the LAN, direct over the internet, relayed through TURN). It never leaves the device.

**AGENTS.md §7.1 amendment (exact wording, 2026-11-02, replaces the 2026-10-23 text):** "Media, alerts and heartbeats never transit a third party. They may transit only an encrypted TURN: one the user entered, or the TURN that a signaling server of the pairing hands out on its own host (by default the project server `gs.mydata.rocks`), with short-lived credentials. Signaling goes only to the pairing's own mailboxes (the project server, own relays, the LAN listener), only as sealed envelopes, so operators see only the items in "Relay-visible data"; each device discloses the project server and has an off switch for it. STUN goes only to those signaling hosts or to hosts in `data/stun-servers.json`, enforced by lint. The project server logs no client IP addresses (signaling server, reverse proxy and TURN)."

The 2026-10-23 text ("Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` …") is kept under "Discarded alternatives".

### 8. Disclosure and off switch (layer: UI over contract models)

**Release gate (replaces the 2026-10-25 gate on `n:` entries):** a release build puts the project server into invites and mailbox sets only when (a) the disclosure and the off switch exist on that platform and (b) the privacy policy and imprint URLs in `data/project-server.json` answer (task 10.3). One build-time flag per platform decides, replacing the Nostr flags in place: Android `ProjectServer.enabledFor(BuildConfig.DEBUG)` and the web `projectServerEnabled(import.meta.env)`, on for debug, `vite dev`, `--mode e2e` and the emulator gate, off for release until tasks 5.3 and 6.2 flip them. A test asserts the release value on each platform. Dev and E2E builds never reach the public server: they use the local server of `just dev`, the E2E stack or the gate stack.

**Basic settings (Connection) and setup:**

- One sentence: "To find each other over the internet, your devices use the GuardedSleep server in Germany. It sees this device's internet address and when it is in use. If the devices can't connect directly, the sound travels through it encrypted, so it can never listen."
- A "Who runs it?" line that expands to the operator, the country and the privacy policy and imprint links. They come from `data/project-server.json` (§6), not from locale copy.

**Advanced:** the switch "Only same Wi-Fi or my own server".

- It acts on its own device: no project server, project STUN or project TURN is contacted from it.
- If the peer still announces the project server, the UI shows a hint.

### 9. Public deployment (`gs.mydata.rocks` on the owner's Contabo host)

**Host:** Contabo VPS, Ubuntu 26.04, Docker 29, public IPv4 `167.86.115.143` and IPv6 `2a02:c207:3004:4659::1` on the interface. DNS: `gs.mydata.rocks` is a DNS-only CNAME at Cloudflare to `contabo.ditscher.me`, so Cloudflare never proxies or sees the traffic.

**Traefik (owner's homelab repository, not this one):** `compose/contabo/traefik`, copied from `compose/oracle/traefik`, pinned tag, **access log off**, the Let's Encrypt resolver `letsencrypt`, the `secure-headers@file` middleware, external network `traefik`. Idle timeouts of at least 35 s on the WebSocket path (`signaling-relay` "Metadata only, no IP logging").

**GuardedSleep stack (this repository, `docker/public-server/`, task 10.1):** a documented example in the owner's Oracle style, deployed by a clone on the host (`git pull && docker compose up -d`):

- `compose.yaml` with:
  - `signaling`: the `guardedsleep-signaling` image (GHCR, pinned tag or digest), on the external `traefik` network only, no published ports, `STUN_DISABLED=1`, `SIGNALING_TRUST_PROXY=1` with one hop, `TURN_SECRET`, `TURN_URLS`, `STUN_URLS`, `TURN_TTL_S`, read-only, `cap_drop: ALL`, `no-new-privileges`; Traefik labels `traefik.http.routers.guardedsleep-secure.rule` for `Host(gs.mydata.rocks) && PathPrefix(/signaling)`, `tls.certresolver=letsencrypt`, `middlewares=secure-headers@file`, and the entrypoint name of the owner's Oracle files (set in `.env`, not guessed in the example);
  - `coturn`: the profile of §7, host networking, pinned tag, read-only with tmpfs, `cap_drop: ALL` plus `NET_BIND_SERVICE`;
  - `web` (optional profile): the `guardedsleep-web` image at `https://gs.mydata.rocks/` with the CSP of §6, plus the static privacy policy and imprint pages if the owner hosts them there (§10).
- `.env.template` with every variable and a comment per setting with its doc link; `.env` is git-ignored and holds `TURN_SECRET`.
- `README.md`: first deployment, update (`git pull && docker compose up -d`), the firewall rules, the log check, the Uptime Kuma check, and how a self-hoster adapts it (another domain, without coturn, with the built-in STUN).

**`turns:` on 5349:** coturn needs the certificate as files, Traefik keeps it in `acme.json`. Task 10.2 adds a pinned certificate-export sidecar (or coturn's own ACME-free reload of an exported file) and the `turns:` URL; until then the server hands out `turn:` only. `turns:` on 443 through Traefik's TCP router with SNI is an `IDEA`.

**Logs and IP addresses:**

- Traefik access log off (homelab repository); Traefik's own log at `ERROR` level.
- `guardedsleep-server` logs no IP addresses or room ids (existing requirement); its per-IP rate limits keep addresses in memory only.
- coturn: logs must not contain client addresses. coturn prints session and allocation lines with addresses at `info`; the profile logs at a level that omits them, or to `/dev/null` if no level does, and task 10.1 proves it with a relayed test call and a search of `docker compose logs` for the test client's address.
- Docker `json-file` logs capped (`max-size`, `max-file`) as in the existing compose files.

**Firewall (owner task, §10):** inbound 22/tcp (SSH), 80/tcp and 443/tcp (Traefik), 3478/udp and 3478/tcp (STUN and TURN), 5349/tcp (`turns:`, once 10.2 lands), the relay range (default 49160–49359/udp); everything else dropped, IPv4 and IPv6 alike. Docker's published ports bypass `ufw`; the stack publishes none (Traefik is the only publisher, coturn uses host networking), and the README says so.

**Monitoring:** an Uptime Kuma check from the owner's Oracle host: an HTTPS keyword check on the server's health response (`https://gs.mydata.rocks/signaling` answers a plain GET without upgrade with a fixed status; task 10.1 names the exact check) and a STUN or TCP check on 3478. Alerts go to the owner; nothing about users is collected.

- **Pros:** reuses the owner's Traefik and habits; the stack is a working example for self-hosters; no new tooling in CI.
- **Cons:** the deployment lives partly outside this repository (Traefik), so the README must name the assumptions; no automated deployment, so an update needs the owner.
- **Recommendation:** adopt. Automated image updates on the host (Watchtower or similar) are out of scope; the owner pulls.

### 10. Owner tasks

Not implementable by an agent; listed in `tasks.md` group 10 and tracked there:

- **Privacy policy and imprint** for the public service (German law: Impressum per DDG §5 for a service offered in Germany; GDPR Art. 13 information): controller, what is processed (IP addresses and timing, in memory; relayed encrypted media), purpose and legal basis (legitimate interest in running the service), no logging of IP addresses, retention (none beyond memory and the capped container logs, which hold no addresses), Contabo as processor, contact. Published at stable URLs that go into `data/project-server.json`.
- **Data processing agreement with Contabo** (GDPR Art. 28).
- **No IP logging:** Traefik access log off in the homelab repository; check after the first deployment with the log search of task 10.1.
- **Firewall** on the Contabo host (§9), and SSH hardening later (key-only root login now; a non-root user is a follow-up).
- **Uptime Kuma check** on the Oracle host.
- **Domain and contract renewal:** `mydata.rocks` and the Contabo contract with reminders, because an expired domain would let someone else receive every default pairing's signaling metadata and break internet recovery for all of them (the envelopes and the pin still hold).
- **Abuse contact:** an address in the imprint for TURN abuse reports.

### 11. Nostr removal (what goes, what stays)

**Goes (task group 8):**

- Core: `nostr.rs`, `link/nostr_link.rs`, `conformance_nostr.rs`, `relay_pick.rs`, the `nostr` mailbox kind, the `n:` invite kind, the epoch, the Nostr budgets, parking, freshness window and hygiene rules (§3 as built, under "Discarded alternatives"), the `extra_relays` interface, the Nostr-only `clock_skew` sources, and the `k256` dependency; the Nostr vectors in `vectors/conformance.json`; the `nostr_frame` fuzz target.
- Bindings: every Nostr and relay-pick type in `ffi`, `wasm` and `packages/core-wasm` (`just api-snapshot`).
- Simulation: `world/nostr_sim.rs` and the `nostr-*` scenarios.
- Platforms: the Nostr parts of the Android and web drivers, `PublicRelays` / `public-relays.ts`, the relay pick feeding; E2E `32-web-nostr.spec.ts` and `utils/nostr-relay.ts`.
- Tooling and data: `data/nostr-relays.json`, `crates/gs-soak`, `scripts/spike/`, `.github/workflows/relays-weekly.yml`, the `soak` and `relays-check` recipes, `docs/contributing/relay-soak.md`, their `docs/ci-gates.md` rows; the Nostr parts of `public-lists`.

**Stays:** the envelope and its vectors, sessions, presence, the parent pin, the mailbox abstraction with `relay` and `lan`, invite v4 with `r:` and `l:`, the mailbox update, the wall-clock input (§2), the bundled `ice` candidates and the 6-second resend for all mailbox kinds, the 30-minute reachability check.

**Ported, not dropped:** sim scenarios whose subject is generic get a relay-only twin before the Nostr version is deleted: "one of three relays dies during setup" (own relay plus project server), "relay accepts but never delivers", "flapping relay does not stall setup", "all mailboxes dead at a reachability check", "reachability check reveals nothing of the pairing", "mailbox update after relay churn", "single attempt" setups, and the stored-stop replay after a restart. Coverage of each requirement in `specs/signaling-mailboxes` must stay with at least one scenario; task 8.2 lists the mapping.

**Order:** the removal lands before the TURN work, so the TURN tasks touch the smaller core. Each task keeps `just test` green on its own.

### Discarded alternatives

**From the 2026-10-23 design (reasons as recorded then; reversals marked):**

- **Project-run public signaling instance as default:** a single operator and single point of failure, and owner workload (owner, 2026-10-23). **Reversed 2026-11-02:** now the default (§3); the single point of failure is accepted and mitigated there.
- **Nostr plus a project fallback instance:** the owner would still run a server. (2026-11-02: the owner runs the server, and Nostr goes; see below.)
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
- **Automatic Cloudflare TURN like Signal; own EU TURN:** a US operator or a server run by the owner. Decided for beta with data. **Reversed 2026-11-02 for the own EU TURN:** it is in the alpha on the project server (§7); Cloudflare TURN stays discarded (a US third party in the media path).

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

**From the 2026-11-02 revision:**

- **Keep Nostr, parked behind a flag, as a fallback mailbox:** the code would still need maintenance, vectors, fuzzing and security review, and a parked path rots untested. The owner chose complete removal (2026-11-02); the git history keeps it, and an optional user-entered Nostr relay is an `IDEA` in `docs/backlog.md`.
- **Two project instances in the default invite (Contabo and Oracle):** removes the single point of failure, but doubles the operator work and the parties that see metadata before the alpha has shown it is needed. `IDEA` for beta with uptime data.
- **Third-party EU STUN by default next to the project server:** adds parties that see IP and timing, and gains nothing, because without the project server there is no internet signaling to use a reflexive candidate for (the LAN needs no STUN).
- **TURN credentials issued only after the pairing proof** (backlog `IDEA`, 2026-10-25): impossible, since the proof runs over a DataChannel that may itself need TURN to come up, and the server cannot see it.
- **TURN credentials to any joined member (one-member rooms):** anyone could fetch credentials with one socket. The two-member gate raises the cost a little and keeps lone sockets out; the quotas carry the rest.
- **A static TURN user and password in the apps:** extractable from any APK or bundle, cannot be revoked per session, and lives forever (backlog `IDEA` of 2026-10-25: "no static credential in any binary").
- **TURN URLs from the invite or a client-side constant instead of the server:** every host change would need an app release; the server-supplied set with the host-binding rule keeps only the signaling host in the apps.
- **TURN on a different host than the signaling server:** would need a second trust rule; the host binding keeps the rule simple and stops a relay from pointing media at a third party.
- **Play Integrity / App Attest as the TURN gate:** Google dependency, breaks IzzyOnDroid, privacy (backlog `IDEA`, 2026-10-25).
- **Caddy as the reverse proxy of the stack** (backlog `PLANNED` T1 bundle): the owner already runs Traefik on the host; the example follows it. A Caddy variant for self-hosters stays possible later.
- **Cloudflare proxy (orange cloud) in front of the server:** Cloudflare would see every connection's metadata and terminate TLS; DNS only.

**2026-10-23 wording of AGENTS.md §7.1 (superseded 2026-11-02):** "Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` (criteria: `openspec/specs/signaling-mailboxes`, "Curated public services"), only as sealed envelopes, so operators see only the items in "Relay-visible data", with disclosure and an off switch on each device. Only STUN hosts in `data/stun-servers.json` are allowed, enforced by lint."

#### Discarded: public Nostr relays as the default internet mailbox (decided 2026-10-23, built in tasks 3.5, 3.6 and 4.1, removed 2026-11-02)

**Why it was chosen:** no project-run server and no single operator (owner, 2026-10-23); see `spike.md`.

**Why it was removed (owner, 2026-11-02):** volunteer relays mean operator outreach (task 4.3), list churn, rate limits outside our control, a go/no-go soak (task 4.2) and weekly re-vetting, and the Nostr mailbox was the largest and most intricate part of the core (epochs, BIP-340 keys, budgets, parking, inbound hygiene, freshness windows). One own server with sealed envelopes leaks the same metadata (IP addresses and timing) to fewer parties. The go/no-go never ran.

The design text as it stood on 2026-11-02 follows unchanged, for the record.

##### Former §3: Nostr mailbox (layer: core)

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

##### Former §6: Curated lists (repository `data/`; generator into both clients and the web CSP)

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

##### Former §7: ICE without project STUN or TURN

- **With an own relay mailbox configured:** only its STUN.
- **Otherwise (the default):** the first 2 STUN entries of distinct operators (Nextcloud on 443/3478 and Telekom), queried in parallel by ICE.
- Only STUN hosts in `data/stun-servers.json` are allowed, enforced by a lint check on client source and configuration.

**TURN:** none by default; a user-entered TURN in Advanced. An attempt that fails with no relayed candidate maps to the existing `media_blocked` cause. Its level-2 troubleshooter copy is plain: "These two devices can't reach each other from where they are." Its actions are "Try again" and "Put both on the same Wi-Fi", plus "Advanced: TURN server" for experts. It never replaces the alarm.

**Diagnostics:** a local view shows the connection type of the last attempt. It never leaves the device.

**AGENTS.md §7.1 amendment (exact wording for task 1.1, from review 2, I8):** "Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` (criteria: `openspec/specs/signaling-mailboxes`, "Curated public services"), only as sealed envelopes, so operators see only the items in "Relay-visible data", with disclosure and an off switch on each device. Only STUN hosts in `data/stun-servers.json` are allowed, enforced by lint."

##### Former §8: Disclosure and off switch (layer: UI over contract models)

**Release gate (task 3.6, security review of 3.5, M7):** until the disclosure and the off switch exist, a release build must not put `n:` entries into invites, because that would send connection metadata to third parties without the user seeing or refusing it. One build-time flag per platform decides: Android `PublicRelays.enabledFor(BuildConfig.DEBUG)` (on for debug builds and the emulator gate, off for release) and the web `publicRelaysEnabled(import.meta.env)` (on for `vite dev` and `--mode e2e`, off for `--mode production`, which the release build and the web image use). A test asserts the release value is off on each platform. Tasks 5.3 and 6.2 flip these flags when the disclosure and the off switch land. The dev and E2E builds additionally let a set `guardedsleep:dev-nostr-relays` list (even an empty one) replace the public relays, so no test reaches a public relay.

**Basic settings (Connection) and setup:**

- One sentence: "To find each other over the internet, your devices use a few independent services in Europe. They see this device's internet address and when it is in use, never the baby's sound."
- A "Who are they?" list that expands. Operator names come from the data files, not from locale copy.

**Advanced:** the switch "Only same Wi-Fi or my own server".

- It acts on its own device: no public relay or STUN is contacted from it.
- If the peer still announces public mailboxes, the UI shows a hint.

##### Former §9: Cut list and go/no-go

A **go/no-go gate** sits after the core work (tasks 2–3) and before Android and web (5–6).

- **What it tests:** a CLI soak prototype (`scripts/spike/soak`) on the core runs one night on 3 EU relays, plus the spike's unknowns 1–4 (8 h subscriptions, bursts behind one CGNAT IP, tag-only filter, LTE / IPv6-only).
- **Go:** at least 2 of 3 relays without data loss and setup p95 < 10 s (owner, 2026-10-25; was 3 s). Loss detection takes 25 s and level 2 comes 60 s after the last contact, so a recovery within 10 s still ends long before the red alarm. The first real-network soaks measured recovery p50 of about 2.5 s and p95 of 3.6–5.8 s over five to six serial relay hops.
- **Discarded:** keeping 3 s and shortening the handshake (merging the challenge round into the opening presence). It saves one relay round (about 0.5–1 s), stays unreliable on slow relays, and needs a protocol change with a security review.
- **If no-go:** the alpha ships with relay mailboxes (self-hosted or LAN) only. Nostr moves to beta, and the owner decides again.

**Deferred regardless:**

- second-PC forwarding (`add-offline-lan`);
- refreshing the lists at run time;
- the events-per-hour cap beyond the fixed limit in §3.

**Operator contact:** an owner task before release. Contact the listed operators about acceptable use and logging, and record their answers in the data notes.

## Risks / Trade-offs

| Risk                                                                              | Mitigation                                                                                                                                                                      |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The project server is down: no internet recovery for families on default settings | verified links continue (mailboxes closed while verified); Uptime Kuma alerts the owner; own relay and LAN mailbox as second paths; the mailbox update can move the host        |
| The domain or the Contabo contract lapses; someone else gets `gs.mydata.rocks`    | renewal reminders (§10); envelopes, session binding and the parent pin hold against a hostile relay; its ICE servers are host-bound, so it learns metadata and can deny service |
| TURN abuse by self-made clients                                                   | two-member gate, 10-minute credentials, per-IP join limits, coturn quotas and `max-bps`, denied peer ranges including the host's own addresses                                  |
| A relayed night drops when the TURN credential expires                            | task 9.4 verifies allocation refresh past the TTL before release; fallback TTL of 12 h decided by the owner                                                                     |
| The owner sees IP addresses and usage times of every default family               | disclosed with the operator's name; no IP logging (server, Traefik, coturn) with a log test; privacy policy; off switch                                                         |
| coturn logs client addresses                                                      | log level chosen and proved by the log search in task 10.1                                                                                                                      |
| No forward secrecy for signaling                                                  | recorded in the threat model; envelopes never contain audio                                                                                                                     |
| A stranger with the room id squats a slot while a pair is verified (room empty)   | denial of service only; escalation still runs (§4 "Our relay"); room ids are HKDF-derived                                                                                       |
| Removing Nostr loses test coverage of generic mailbox behaviour                   | relay-only twins of the generic scenarios before deletion (§11), with a requirement-to-scenario mapping in task 8.2                                                             |
| Persona A (venue Wi-Fi, mobile ↔ mobile) needs TURN                               | the project TURN serves it now; the hardware round measures the relayed share                                                                                                   |

## Migration Plan

Nothing is released, so no pairing needs migration; invites with `n:` entries simply stop parsing.

- **Order:** Nostr removal (group 8) → server TURN credentials and core ICE (group 9) → the stack and deployment (group 10) → server protocol docs (5.1) → Android (5.2–5.4) → web (6.x) → integration (7) → `add-offline-lan`.
- **Rollback:** a device with "Only same Wi-Fi or my own server" or an own relay works without the project server; the project server can be stopped without any app change, and families fall back to the LAN path once `add-offline-lan` lands.

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
