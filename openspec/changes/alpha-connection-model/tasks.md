# Tasks

Alpha (T0). Builds on `rebuild-web-client` group 1 (core invite codec) and group 4 (web link driver); `add-offline-lan` follows this change. Each task is a brief for the `implementer`:

- layer rules from AGENTS.md §5.1: all supervision, mailbox choice, timing and crypto in the core; the platforms own the sockets and supply randomness and clocks;
- the privacy guardrails as amended in task 1.1;
- new RustCrypto crates with `default-features = false`, on the generation already in use (`hkdf 0.13`, `hmac 0.13`, `sha2 0.11`);
- commit after each task, and run `just test-affected` after each task.

## 1. Guardrails, decisions and history

- [x] 1.1 Amend AGENTS.md §7.1 and §7.3 with the exact wording in `design.md` §2 and §7, and the §9 rows (internet mailboxes: alpha; TURN: user-entered only in the alpha). Mirror the change in `.cursor/rules`. Add a context note to ADR-018 in the decision archive README (tester setup: Nostr and relay mailboxes, no TURN), and update the rounds in `docs/contributing/alpha-hardware-test.md` (Nostr cross-network, network switch, a Persona A venue round). Verify: `just lint-text`.
- [x] 1.2 Threat model entries in `docs/concept/failure-model.md` (or the security doc it points to): operators learn IPs and timing; the fixed kind lets anyone count setups; the 3-relay set is stable across days; no forward secrecy for signaling; parent pin limits. Verify: `just lint-text`.

## 2. Core inputs and envelope

- [x] 2.1 New `Link` inputs: a 32-byte `LinkConfig.seed`, drawn fresh for every run, feeding an in-core `ChaCha20Rng` (`chacha20` 0.10 with feature `rng`, no `rand_chacha`); and `wall_clock(unix_s)` at start, with ticks and on time changes, used only for Nostr. Construction fails without a seed. The seed is secret-class: never logged, and recorded only in simulation traces. Traces record both inputs; existing traces are re-blessed with a fixed seed and must show no other diff. Update the bindings and `just api-snapshot`. Verify: core tests, sim tests, `just api-check`.
- [x] 2.2 `envelope` module:
  - HKDF-SHA256 extract with an empty salt, then expand per purpose (`guardedsleep seal v1`, `guardedsleep nostr tag v1`, `guardedsleep nostr key v1`);
  - XChaCha20-Poly1305 (`chacha20poly1305`, feature `alloc`); nonce from the CSPRNG; nonce = envelope id;
  - associated data `"gs-env/1" ‖ role ‖ class`;
  - a clear header (version, role, class) and a plaintext of `recipient_session`, `sender_session`, `seq`, message and padding over the bucket ladder (1, 4, 8 KiB), with a refusal to send anything oversized;
  - session candidates and confirmation, retirement on stop, a `seq` bitmap per session, and one replay window per run (4096 ids or 30 min) never cleared on a mailbox reopen;
  - rejection of a bad tag, own role, wrong recipient or sender session and replays; de-duplication only after authentication;
  - parser lint denies; no `Debug` of keys; proptests (round trip, no reflection, no replay).

  Verify: `scripts/cargo.sh test -p guardedsleep-core`, including the tests "stored stop from a retired sender session" and "corrupted copy first"; `just deny` with the dependency count re-blessed and justified; the registry check that `chacha20poly1305` and `k256` releases are on the `rand_core 0.10` generation.

- [x] 2.3 Envelope conformance vectors (valid, wrong key, own role, wrong session, replay, truncated, each bucket) through every binding, and a `cargo-fuzz` target for envelope parsing. Verify: core tests, `just test-wasm`, the fuzz target builds.

## 3. Core mailbox set

- [x] 3.1 Mailbox abstraction with the relay mailbox only: the `Sig` state becomes one `relay` mailbox behind `open_mailbox`, `mailbox_send`, `close_mailbox`, `mailbox_opened`, `mailbox_closed` and `mailbox_frame`. No behaviour change: existing traces replay unchanged apart from the action names, which are mapped in the trace format. Update the bindings and the Android and web drivers with a minimal adapter. Verify: core and sim tests, `just test`.
- [x] 3.2 Sealing on the relay mailbox: `signal` payloads become envelopes; the stop MAC is replaced by session binding; relay protocol docs updated. Verify: core tests, new sim scenario "stored stop after parent restart is ignored", `just smoke`.
- [x] 3.3 Invite v4 in the core codec:
  - compact entries `n:`, `r:` and `l:`;
  - ≤ 300 bytes and ≤ 6 entries;
  - the drop order;
  - rules per kind (RFC 1918 and ULA only for `l:`);
  - v3 rejected.

  Replace v3 in `ffi`, `wasm` and `packages/core-wasm`, with new conformance vectors and `just api-snapshot`. Verify: `just api-check`, the core and core-wasm tests.

- [x] 3.4 Presence, sessions and parent pin:
  - sealed `here` with the wall clock; offer direction, accepted senders, scheduling and causes over confirmed sessions (`specs/link-supervision`); the `clock_skew` hint;
  - the parent secret and device name over the DataChannel only, and the parent proof `HMAC(parent_secret, "gs-parent-proof/1|" ‖ fp_parent ‖ fp_baby)` before the microphone path opens;
  - early refusal with a MAC over the nonce, and a rate-limited `security_warning`;
  - `LinkConfig.pinned_parent` and `LinkEvent::ParentPinned`, in the bindings and `api-snapshot`;
  - closing the mailboxes while verified, throwaway-filter reachability checks and the `recovery_path_unavailable` hint; reopening on `peer_stopped`;
  - the mailbox update over the DataChannel.

  Verify: sim scenarios "attacker with the PSK and every envelope is refused", "man in the middle after pairing is refused", "all mailboxes dead at a reachability check", "mailbox update after relay churn", "stored baby stop after the baby restarted"; a capture test that no probe contains the tag or a key.

- [x] 3.5 Nostr mailbox:
  - epoch from 12:00 UTC (u32 BE), tag, per-role keys (`k256` with features `schnorr` and `arithmetic`; scalar reduced, zero rejected);
  - NIP-01 frames with ASCII base64url content only and 64 KiB caps;
  - subscription with `since` = now − 15 min and dual epochs for the hour around the change;
  - re-subscribe at the change and every 60 s while not connected;
  - attempts gated on a confirmed session, pacing of ≤ 2/s, ≤ 6 per attempt, ≤ 120 per hour, parking with 1/5/30-minute backoff (not for `created_at` refusals, which raise `clock_skew`), an inbound cap, and a resend once after 6 s in a new event;
  - `NonZeroScalar` keys (no key for that epoch on zero) and `aux_rand` from the CSPRNG;
  - pick the ephemeral kind and record it in the spec.
  - apply "Only same Wi-Fi or my own server" (and its hint) to mailbox updates as well as invites: a parent with the switch on ignores public `n:` entries from a baby's update (security review of 3.4, M6).
  - bundle ICE candidates into one `ice` payload with a short flush delay (deferred here from 3.2: on the relay each candidate is its own 8 KiB-class envelope), and the resend-once-after-6 s rule for all mailbox kinds (deferred from 3.2; the relay path is reliable and in order).

  Verify: core tests; NIP-01 id and signature vectors checked against an independent implementation; a fuzz target for NIP-01 frames; sim scenarios "relay accepts but never delivers", "clock five minutes off at the epoch change", "one-hour outage stays within budget".

- [x] 3.6 Curated lists:
  - `data/nostr-relays.json` and `data/stun-servers.json`, seeded from `spike.md`;
  - a generator into both clients and into the web CSP `connect-src`;
  - build minimums (6 relays from 4 operators, 2 STUN servers from distinct operators), a warning for stale entries, and a lint against unlisted STUN hosts in source;
  - the deterministic relay pick (3 from distinct operators, plus up to 2 own-list relays).

  Verify: generator tests; `just lint` fails below the minimums and on a Google STUN host.

  As built: design.md section 6 ("As built (task 3.6)") and the release gate in section 8. The own-list extras (`extra_relays`) exist in the core and its bindings, but no platform feeds them into the link config yet.

## 4. Go/no-go

- [x] 4.1 Commit the spike scripts to `scripts/spike/` and add a CLI soak prototype on the core (`scripts/spike/soak`), plus a weekly non-blocking job with its `docs/ci-gates.md` row. Verify: the job runs once on a branch.

  As built: `crates/gs-soak` (soak, burst and the event signer on the core's public API), the Node probes in `scripts/spike/`, `just soak [hours]` and `just relays-check`, `.github/workflows/relays-weekly.yml` and the runbook `docs/contributing/relay-soak.md`. The workflow cannot run on a branch before it exists on the default branch (`workflow_dispatch` rule); the same `just relays-check` was run locally instead, and the first scheduled or dispatched run after merging is the remaining check.

- [ ] 4.2 Owner-run gate: one night on 3 EU relays, a burst test behind one CGNAT IP, the tag-only filter, LTE / IPv6-only, and the `created_at` tolerance per relay software (spike unknown 9). Record the results in `spike.md`. Go needs at least 2 of 3 relays without loss and setup p95 < 10 s (owner, 2026-10-25). On no-go, Nostr moves to beta and tasks 5–6 drop their Nostr parts. Verify: the decision is recorded in `design.md` §9.
- [ ] 4.3 Owner task: contact the listed relay operators about acceptable use and logging, and record their answers in the data notes. Verify: the notes are present for every shipped relay.

## 5. Server and Android

- [ ] 5.1 `guardedsleep-relay` and the server carry sealed envelopes opaquely; protocol docs. Verify: `scripts/cargo.sh test -p guardedsleep-relay -p guardedsleep-server`, `just smoke`.
- [ ] 5.2 Android platform (also: feed up to 2 `extra_relays` from the device's own shipped list into `LinkConfig.mailboxes` at run time only — never into the invite or a mailbox update, not with `only_local`; the 6-entry/300-byte limits apply to invites only):
  - mailbox clients, one WebSocket per mailbox, opened and closed only on core actions;
  - the seed from `SecureRandom` and wall-clock reporting;
  - the ICE config from the core lists;
  - the off switch;
  - the parent id and mailbox set in the encrypted pairing record.

  Verify: JVM tests with fake sockets; `just verify-android`.

- [ ] 5.3 Android UI (BLOCKER for any build that ships 3.6 relay entries: disclosure and the off switch must exist before a release baby emits `n:` entries; security review of 3.5, M7; this task flips `PublicRelays.enabledFor` to on for release builds, see design.md section 8):
  - the Connection section with the disclosure and "Who are they?";
  - on the baby: "Connected to <name>" with "Not my device", the security warning for refused parents, and the copy that forgetting on the parent alone leaves the baby paired;
  - Advanced: the off switch, own relays and TURN;
  - the diagnostics view and the `media_blocked` troubleshooter copy;
  - "Make brighter" for the larger QR code;
  - a scan test of the v4 QR code inverted at 50 % (Robolectric with zxing);
  - measure real libwebrtc SDP sizes against the Description bucket (largest message is 16 bytes smaller since the pin tag); an oversized seal must surface as a visible error, not a silent outage (security review of 3.4, M7);
  - copy in `locales/*.json`, with critical tags for connection state.

  Verify: Roborazzi recorded in the pinned image, `just verify-android`.

- [ ] 5.4 Emulator gate scenarios: pairing through the local Nostr test relay; a network switch during monitoring; a stored stop replayed after a parent restart. Add `docs/ci-gates.md` rows. Verify: `DOZE_MINUTES=2 just gate`.

## 6. Web

- [ ] 6.1 Web link driver (also: ignore invite and update hosts outside the deployment CSP `connect-src`, design §5 version skew; feed `extra_relays` as runtime-only mailboxes, see 5.2): mailbox WebSockets; the seed from `crypto.getRandomValues`; the wall clock; the ICE config from the lists; the off switch; the CSP `connect-src` from the generated list. Verify: Vitest with fake sockets; the storage test still finds no PSK, derived value or parent id.
- [ ] 6.2 Web settings and copy as in 5.3 (same blocker as 5.3; this task flips `publicRelaysEnabled` to on for production builds), plus the QR scan test in Playwright (inverted, 50 %). Verify: Vitest, Playwright with axe, `just lint-text`.
- [ ] 6.3 Playwright: a minimal NIP-01 test relay in the `apps/e2e` utilities (no public relays in CI); two contexts pair through it; a mailbox drop during setup still verifies; a CSP check that only listed origins connect. Verify: `just e2e`.

## 7. Integration

- [ ] 7.1 Add the log canary (AGENTS.md §7.6): the simulation and gate runs pair with a known PSK, and CI fails if the PSK, a derived key, the seed, the parent secret (raw, its four HKDF subkeys and the `parent_pin` message text, which carries it), the room id or plaintext SDP appears in any log in any encoding. The tag is allowed in relay-visible bytes and not in logs. The canary list needs the parent secret from the first simulated pairing, because that secret is drawn at run time (security review of 3.4, M3). Verify: the canary test fails on a planted leak and passes otherwise.
- [ ] 7.2 `GS_STRICT=1 just ci` green on the wave branch.

## Workflow follow-up

- Archive `rebuild-web-client` first. Its deltas also modify `pairing` "Invite payload v3" and `signaling-relay` "Client ICE configuration" and add "One invite codec in the core", `web-client` "Copy code without sharing" and "Browser security". Re-check this change's MODIFIED blocks against the merged specs before archiving it.
- Second Critical Reviewer pass done (2026-10-23); its findings are answered in `design.md`. Security Architect review of tasks 2.2 and 3.4 diffs before merging them.
- Role reviews at the end of implementation: Technical Architect, Security Architect, UX.
- The owner decides project TURN for beta with the hardware-round data (alpha: no default TURN, owner 2026-10-23).
- `mise exec -- openspec archive alpha-connection-model --yes`, then `openspec validate --all --strict`.
