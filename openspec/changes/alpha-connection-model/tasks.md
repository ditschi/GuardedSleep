# Tasks

Alpha (T0). Builds on `rebuild-web-client` (archived); `add-offline-lan` follows this change. Revised 2026-11-02 (project server replaces Nostr, TURN in the alpha): **open work runs in the order 8.1 → 8.7 → 8.2–8.6 → 9 → 10 → 5 → 6 → 7** (8.2's two-relay twins need 8.7); task 9.1 is a standalone spike that runs first or in parallel with group 8. Groups 8–10 start only after the owner approved this revision. Each task is a brief for the `implementer`:

- layer rules from AGENTS.md §5.1: all supervision, mailbox choice, timing and crypto in the core; the platforms own the sockets and supply randomness and clocks;
- the privacy guardrails as amended in task 1.1;
- new RustCrypto crates with `default-features = false`, on the generation already in use (`hkdf 0.13`, `hmac 0.13`, `sha2 0.11`);
- commit after each task, and run `just test-affected` after each task.

## 1. Guardrails, decisions and history

- [x] 1.1 Amend AGENTS.md §7.1 and §7.3 with the exact wording in `design.md` §2 and §7, and the §9 rows (internet mailboxes: alpha; TURN: user-entered only in the alpha). Add a context note to ADR-018 in the decision archive README (tester setup: Nostr and relay mailboxes, no TURN), and update the rounds in `docs/contributing/alpha-hardware-test.md` (Nostr cross-network, network switch, a Persona A venue round). Verify: `just lint-text`.
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

- [x] 3.5 Nostr mailbox (**superseded 2026-11-02**: built, then removed by group 8; design under "Discarded alternatives"):
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

- [x] 3.6 Curated lists (**superseded 2026-11-02** for the relay list, the relay pick and the STUN default: removed or reduced by tasks 8.4 and 9.1):
  - `data/nostr-relays.json` and `data/stun-servers.json`, seeded from `spike.md`;
  - a generator into both clients and into the web CSP `connect-src`;
  - build minimums (6 relays from 4 operators, 2 STUN servers from distinct operators), a warning for stale entries, and a lint against unlisted STUN hosts in source;
  - the deterministic relay pick (3 from distinct operators, plus up to 2 own-list relays).

  Verify: generator tests; `just lint` fails below the minimums and on a Google STUN host.

  As built: design.md section 6 ("As built (task 3.6)") and the release gate in section 8. The own-list extras (`extra_relays`) exist in the core and its bindings, but no platform feeds them into the link config yet.

## 4. Go/no-go (dropped with Nostr, 2026-11-02)

- [x] 4.1 (**superseded 2026-11-02**: removed by task 8.4) Commit the spike scripts to `scripts/spike/` and add a CLI soak prototype on the core (`scripts/spike/soak`), plus a weekly non-blocking job with its `docs/ci-gates.md` row. Verify: the job runs once on a branch.

  As built: `crates/gs-soak` (soak, burst and the event signer on the core's public API), the Node probes in `scripts/spike/`, `just soak [hours]` and `just relays-check`, `.github/workflows/relays-weekly.yml` and the runbook `docs/contributing/relay-soak.md`. The workflow cannot run on a branch before it exists on the default branch (`workflow_dispatch` rule); the same `just relays-check` was run locally instead, and the first scheduled or dispatched run after merging is the remaining check.

- 4.2 **Dropped (owner, 2026-11-02; Nostr removed, the gate never ran).** Was: owner-run gate: one night on 3 EU relays, a burst test behind one CGNAT IP, the tag-only filter, LTE / IPv6-only, and the `created_at` tolerance per relay software (spike unknown 9). Record the results in `spike.md`. Go needs at least 2 of 3 relays without loss and setup p95 < 10 s (owner, 2026-10-25). On no-go, Nostr moves to beta and tasks 5–6 drop their Nostr parts. Verify: the decision is recorded in `design.md` §9.
- 4.3 **Dropped (owner, 2026-11-02; no volunteer operators any more).** Was: owner task: contact the listed relay operators about acceptable use and logging, and record their answers in the data notes. Verify: the notes are present for every shipped relay.

## 5. Server and Android

- [x] 5.1 `guardedsleep-relay` and the server carry sealed envelopes opaquely; protocol docs, including `welcome.iceServers` and the `ice_servers` message of task 9.2. Verify: `scripts/cargo.sh test -p guardedsleep-relay -p guardedsleep-server`, `just smoke`.
- [ ] 5.2 Android platform:
  - mailbox clients, one WebSocket per mailbox (one per relay since task 8.7), opened and closed only on core actions; no URL list or failover in the platform;
  - the seed from `SecureRandom` and wall-clock reporting;
  - the ICE configuration from the attempt's ICE servers (task 9.3) plus the user-entered TURN; no client-side STUN default (`RelayStun` goes; the core computes the relay-STUN fallback);
  - the off switch;
  - the core's mailbox set comes only from the pairing record (invite plus mailbox updates); no silent fallback to default or settings URLs when the record holds none (owner, 2026-11-07: a fallback would bypass the off switch; old alpha pairings without a stored set re-pair for internet use);
  - the parent id and mailbox set in the encrypted pairing record.

  Verify: JVM tests with fake sockets (the attempt's ICE list reaches the media engine unchanged); `just verify-android`.

- [ ] 5.3 Android UI (BLOCKER for any release build that puts the project server into an invite: the disclosure and the off switch must exist first; this task flips `ProjectServer.enabledFor` to on for release builds only when design §8's release gate holds: `data/project-server.json` is `live`, every item of 10.3 is ticked and the 9.1 result is recorded):
  - the Connection section with the disclosure (network addresses, usage times, encrypted relaying) and "Who runs it?" (operator, country, privacy policy, imprint from `data/project-server.json`);
  - on the baby: "Connected to <name>" with "Not my device", the security warning for refused parents, and the copy that forgetting on the parent alone leaves the baby paired;
  - Advanced: the off switch, own relays and the own TURN;
  - the diagnostics view (direct on the LAN, direct over the internet, relayed through TURN) and the `media_blocked` troubleshooter copy;
  - "Make brighter" for the larger QR code;
  - a scan test of the v4 QR code inverted at 50 % (Robolectric with zxing);
  - measure real libwebrtc SDP sizes against the Description bucket (largest message is 16 bytes smaller since the pin tag); an oversized seal must surface as a visible error, not a silent outage (security review of 3.4, M7);
  - copy in `locales/*.json`, with critical tags for connection state.

  Verify: Roborazzi recorded in the pinned image, `just verify-android`.

- [ ] 5.4 Emulator gate scenarios on the gate stack (local server with TURN configured and the gate's coturn): pairing through the server; a network switch during monitoring; a stored stop replayed after a parent restart; a relayed call (direct paths blocked) on a 60-second credential that stays connected for at least 11 minutes, past coturn's `--stale-nonce`; repeated failed relayed attempts across a network switch still allocate (no `Allocation Quota Reached`). Add `docs/ci-gates.md` rows. Verify: `DOZE_MINUTES=2 just gate`.

## 6. Web

- [x] 6.1 Web link driver (also: ignore invite and update hosts outside the deployment CSP `connect-src`, design §5 version skew): mailbox WebSockets; the seed from `crypto.getRandomValues`; the wall clock; one WebSocket per relay mailbox (task 8.7); the ICE configuration from the attempt's ICE servers plus the user-entered TURN; the off switch; the mailbox set only from the pairing record, no fallback to default or settings URLs (owner, 2026-11-07, as in 5.2); the CSP `connect-src` and the default mailbox from the generated project server origin only with the official-deployment flag (design §6), otherwise the deployment's own `/signaling`. Verify: Vitest with fake sockets; the storage test still finds no PSK, derived value, parent id or TURN credential.
- [ ] 6.2 Web settings and copy as in 5.3 (same blocker and release gate as 5.3; this task flips `projectServerEnabled` to on for production builds of the official deployment only), plus the QR scan test in Playwright (inverted, 50 %). Verify: Vitest, Playwright with axe, `just lint-text`.
- [ ] 6.3 Playwright on the local stack (no public server in CI): two contexts pair through the local server; a mailbox drop during setup still verifies with a second local relay; the `ice_servers` message reaches both contexts and a forced-relay run (`iceTransportPolicy: "relay"` in a test-only build) connects through the E2E coturn; a CSP check that only listed origins connect. Verify: `just e2e`.
- [x] 6.4 Staging web image for the owner's test deployment at `https://app.gs.mydata.rocks` (owner, 2026-11-08): Vite mode `staging` turns `projectServerEnabled` on together with the official-deployment flag (loopback stays off), `docker/web/Dockerfile` takes `WEB_BUILD_MODE` (`production` or `staging` only) and `VITE_GS_OFFICIAL_DEPLOYMENT`, and `docker-web.yml` publishes `guardedsleep-web-staging` from `main` (never for pull requests or tags). Release builds and the gate of 6.2 are unchanged. Verify: Vitest (`build-mode.test.ts`, `csp.test.ts`), `docker build` with the staging args shows `wss://gs.mydata.rocks` in `csp-header.conf`, a build with `WEB_BUILD_MODE=e2e` fails, `just lint-workflows`, `just lint-text`.

## 7. Integration

- [x] 7.1 Add the log canary (AGENTS.md §7.6): the simulation and gate runs pair with a known PSK, and CI fails if the PSK, a derived key, the seed, the parent secret (raw, its four HKDF subkeys and the `parent_pin` message text, which carries it), the room id, plaintext SDP or an issued TURN password appears in any log in any encoding. The canary list needs the parent secret from the first simulated pairing, because that secret is drawn at run time (security review of 3.4, M3). Verify: the canary test fails on a planted leak and passes otherwise.
- [ ] 7.2 `GS_STRICT=1 just ci` green on the wave branch.

## 8. Nostr removal (owner, 2026-11-02; design §11)

- [x] 8.1 Core: delete `nostr.rs`, `link/nostr_link.rs`, `conformance_nostr.rs`, `relay_pick.rs`, the `nostr` mailbox kind, the `n:` invite kind (now `not_our_code`), the epoch, budgets, parking, freshness and hygiene rules, `extra_relays`, the Nostr-only `clock_skew` sources (keep the peer comparison, moving `refresh_clock_hint` from `link/nostr_link.rs` into `link.rs`, and the wall-clock input; design §2 and §1 "Hints"), the Nostr HKDF info strings and the `k256` dependency; drop the Nostr vectors from `vectors/conformance.json` and add a vector that an `n:` invite is rejected; delete `fuzz/fuzz_targets/nostr_frame.rs`. Update `ffi`, `wasm`, `packages/core-wasm` and `just api-snapshot`. Verify: `scripts/cargo.sh test -p guardedsleep-core`, `just test-wasm`, `just api-check`, `just deny` with the dependency count re-blessed downwards, the fuzz targets build.
- [x] 8.2 Simulation: before deleting `world/nostr_sim.rs`, the `nostr-*` scenarios and the unprefixed Nostr scenarios (`flapping-relay-does-not-stall-nostr-setup`, `single-attempt-nostr-only-setup`, `single-attempt-relay-and-nostr-setup`), add relay-only twins of the generic ones (design §11 "Ported, not dropped") and write the requirement-to-scenario mapping for `specs/signaling-mailboxes` and `specs/link-supervision` into `crates/guardedsleep-sim/README.md`; every requirement keeps at least one scenario. Re-bless traces whose only diff is the removed Nostr actions. Verify: `scripts/cargo.sh test -p guardedsleep-sim`; the mapping lists no requirement without a scenario.
- [x] 8.3 Platforms and E2E: remove the Nostr parts of the Android and web drivers and controllers, `PublicRelays` / `public-relays.ts` (replaced by the project-server flag of design §8), the relay-pick feeding, `invite-mailboxes` Nostr entries, `32-web-nostr.spec.ts`, `utils/nostr-relay.ts` and the Nostr parts of `dual-context.ts`, `28-web-csp.spec.ts`, `connect-origins.json`, `csp.ts`, `docker/web/nginx.default.conf`. Verify: `just test`, `just e2e`.
- [x] 8.4 Tooling and data: delete `data/nostr-relays.json`, `crates/gs-soak`, `scripts/spike/`, `.github/workflows/relays-weekly.yml`, the `soak` and `relays-check` recipes, `docs/contributing/relay-soak.md` (and its link in `docs/contributing/README.md`), the matching `docs/ci-gates.md` rows; reduce `public-lists` to the STUN allowlist and the project server (task 9.2 adds it). Verify: `just lint`, `just --list` shows no Nostr recipe, `git grep -i nostr` hits only `openspec/`, `docs/backlog.md` and the archive.
- [x] 8.5 Docs: `docs/concept/failure-model.md` (drop the Nostr threats; add: the project operator sees IPs and timing of every default family, a relayed call reveals the peers' LAN addresses to it, TURN relays encrypted media, single point of failure, domain lapse or DNS account takeover, TURN exhaustion by many addresses), `docs/contributing/alpha-hardware-test.md` (rounds through the project server; a Persona A round with TURN), the ADR-018 context note in the archive README, `apps/e2e/README.md`, `crates/guardedsleep-sim/README.md`. Verify: `just lint-text`.
- [x] 8.6 AGENTS.md §3 (repository map: no `gs-soak`, `data/` without Nostr) and §7.3 with the exact wording of design §2 ("after the Nostr removal"). Verify: `just lint-text`.

- [x] 8.7 Several relay mailboxes in the core (design §1 "Several relay mailboxes", review 2026-11-02 C1): every `r:` entry of `LinkConfig.mailboxes` becomes its own relay mailbox (id, URL, host, room membership, relay client id, presence repeats, reachability check); "Ready" per relay; `request_offer`, offers and candidates fan out to every relay where the peer is a member; the platform URL list and failover go (Android `LinkRunner` and the web driver open exactly the URL of each `open_mailbox`). Bindings and `just api-snapshot`. Verify: core tests; sim scenarios with two relays ("one relay dies during setup", "a flapping relay does not stall setup", "both relays deliver the same envelope", "one relay accepts but never delivers"); the spec scenarios "One relay goes down during setup" and "A failing relay mailbox does not stall the setup" pass; `just test`.

## 9. ICE servers and TURN credentials (design §7)

- [x] 9.1 Allocation lifetime spike (runs first; design §7): with the gate's coturn and `--use-auth-secret`, an allocation made with a 60-second credential still relays (Refresh, permission and channel renewals succeed) after at least 11 minutes, past `--stale-nonce`; `turnutils_uclient` or Chrome against the gate stack. Record the result in design §7. **If it fails:** stop and ask the owner for the fallback (credential TTL of 12 hours with the same gate), which then amends the spec lifetime, the server clamp and the 9.3 parser bound together. Verify: the test is committed under `scripts/poc/` or the `apps/e2e` utilities and wired into `just gate` or `just e2e`.
- [x] 9.2 Relay and server (design §7 "Who does what"):
  - `guardedsleep-relay`: the effect `IssueIceServers { conn }` when a room holds at least one baby and one parent, ordered before the `members`/`peer` update that completes the room; a half-TTL renewal timer on `on_tick` while both roles stay joined; one current credential per member (a renewal supersedes the previous one in every count) and `TURN_MAX_LIVE_PER_ADDR` per address (IPv4 or IPv6 /64); never into a one-role room; no client request message; no secret, clock or hash in the relay.
  - `guardedsleep-server`: `PUBLIC_HOST`, `STUN_URLS`, `TURN_URLS` (refuse to start when a URL names another host), `TURN_SECRET` (`secrecy`, never logged), `TURN_TTL_S` (default 600, clamped to 600), `TURN_MAX_LIVE_PER_ADDR` (default 3), each with a doc-linked comment; signs the coturn REST credential (`<expiry>:<16 random bytes base64url>`, base64 HMAC-SHA1 with RustCrypto `hmac` and `sha1`) and sends `ice_servers`; `welcome.iceServers` with STUN only.
  - `data/project-server.json` with `status: pending` and its generator outputs and build check (design §6); `data/stun-servers.json` reduced to `gs.mydata.rocks`.

  Verify: relay unit tests (gate, ordering, renewal, caps, no issuance after a role leaves), a credential vector against coturn's documented algorithm, a log test that no TURN user name or password is logged, a check that `sha1` is absent from the `core` and `relay` dependency graphs, `scripts/cargo.sh test -p guardedsleep-relay -p guardedsleep-server`, `just smoke`.

- [x] 9.3 Core: parse `welcome.iceServers` and `ice_servers` per relay mailbox (parser lint denies; ≤ 8 URLs; `stun:`/`turn:`/`turns:` only; every host equal to that mailbox's host; user name and credential ≤ 128 ASCII; `ttl` 1–600); keep the latest valid set per relay with its monotonic receipt time; build each attempt's list as the union of sets still valid for ≥ 30 s plus `stun:<host>:3478` for each relay without a set, and nothing without a relay mailbox; credentials redacted in `Debug`, never in traces outside test keys. Bindings and `just api-snapshot`. Verify: core tests (foreign host dropped whole, expired set not attached, union over two relays, fallback per relay), a fuzz target for the `ice_servers` frame, sim scenario "TURN credentials renewed across a long outage", `just api-check`.
- [x] 9.4 Lint: extend `stun-sources` to `turn:`/`turns:` URLs in client source and configuration; only the project server host (from `data/project-server.json`), `data/stun-servers.json` hosts and test hosts under `apps/e2e`, `docker/` and `scripts/poc/` are allowed. Verify: the lint's self-test catches a planted `turn:` URL of another host.

## 10. Public deployment (design §9 and §10)

- [x] 10.1 `docker/public-server/` (design §9): `compose.yaml` (signaling pinned to a `vX.Y.Z` tag from a `signaling-server-v*` release cut after group 9, `STUN_DISABLED=1`, `SIGNALING_ALLOWED_ORIGINS`, `PUBLIC_HOST` and the TURN variables, Traefik labels in the owner's Oracle style with router `guardedsleep-secure`, `secure-headers@file`, certresolver `letsencrypt` and the entrypoint name from `.env`, external network `traefik`, no published ports; coturn on host networking with `--use-auth-secret`, `--no-rfc5780`, `--user-quota=5`, `--max-allocate-lifetime=120`, `--total-quota` equal to the relay port range, the other abuse limits and denied peer ranges of `docker/turn/compose.yaml` plus the host's own public IPv4 and IPv6, public addresses as listening and relay IPs; a `web` profile documented as off on the signaling origin), `.env.template` (every variable, a doc link per version-specific setting, `TURN_SECRET` generated with `openssl rand -hex 32`), `.gitignore` for `.env`, and `README.md` (first deployment, update by `git pull && docker compose up -d`, firewall rules with `ufw logging off`, the log search, the monitoring checks of design §9 with the exact URL and expected answer and the TURN allocation check, secret rotation, self-hosting variants). Fold `docker/stack/compose.traefik.yaml`, its env example and `docs/contributing/signaling-traefik.md` into it and delete them (update links). Measure coturn's Binding response to a bare 20-byte request: ≤ 44 bytes and ≤ 2.2 × or report to the owner (design §7). Choose the coturn log settings that keep client addresses out of the logs and prove it: a relayed test call against a local run of the stack, then a search of every container log for the test client's address, including coturn's denied-peer lines, finds nothing. Image tags pinned (no `latest`), versions from the registries, never invented. Verify: `docker compose config` with the template, the log test, `just lint`.
- [x] 10.2 `turns:` on 5349 (now the lego alternative, `compose.lego.yaml`; the default is 10.4): a dedicated certificate for `gs.mydata.rocks` from a separate pinned ACME client (HTTP-01 through a Traefik route, or DNS-01 with a zone-scoped Cloudflare token), preferred over exporting Traefik's `acme.json` (design §9); reload coturn on renewal; add the `turns:` URL to `TURN_URLS`. Verify: a `turns:` allocation against a local run with a test certificate; the README documents the renewal path.
- [x] 10.4 `turns:` through the reverse proxy by default (owner, 2026-11-08; design §9): `docker/public-server/compose.yaml` runs coturn with `--no-tls` and no certificate volume (the lego `acme` service and coturn's `--cert`/`--pkey` moved to `compose.lego.yaml`, selected with `COMPOSE_FILE` or `-f`; `TURN_TLS_ARG_1`/`_2` carry the two coturn flags because an override replaces `command`), `.env.template` lists `turns:` in `TURN_URLS`, and the README documents the Traefik `turns` entrypoint (`readTimeout: 0`), the TCP router with `HostSNI(*)`, `certResolver` and `defaultGeneratedCert`, the trade-offs and the lego text as the alternative. Verify: `just public-server-turns-traefik` (TLS allocation through Traefik with relayed data, 75 s keep-alive with a control without `readTimeout: 0`, no-SNI default certificate, TLS 1.1 refused, wrong credential rejected) and `just public-server-turns` (lego variant) both pass, `docker compose config` passes for both file sets.
- [ ] 10.3 Owner tasks (not for the implementer; the owner ticks them; all of them gate the release flags of 5.3 and 6.2):
  - [ ] privacy policy and imprint published at stable `https://` URLs and entered in `data/project-server.json` with `status: live` (controller, processing of IP addresses and timing in memory, the peers' network addresses a relayed call reveals, relayed encrypted media, legal basis, no IP logging, Contabo as processor, the monitoring host, contact and abuse address);
  - [ ] telecom law check (TKG notification to the Bundesnetzagentur, TDDDG duties) for a public signaling and TURN relay;
  - [ ] AGPL §13 source offer for users of the public server (repository link in the imprint or the health page);
  - [ ] data processing agreement with Contabo;
  - [ ] Traefik in the homelab repository (`compose/contabo/traefik`, access log off, pinned tag, network `traefik`) with real IPv6 client addresses (IPv6-enabled network or host-mode ports), the `turns` entrypoint on `:5349` (`readTimeout: 0`, port 5349 published) and the TCP router to coturn's `<host IPv4>:3478` (design §9, README "turns: through Traefik");
  - [ ] repository and registry access on the host while the repository is private (read-only deploy key; public GHCR package or a read-only token);
  - [ ] first deployment of `docker/public-server/`;
  - [ ] firewall on the Contabo host: 22/tcp, 80/tcp, 443/tcp, 3478/udp+tcp, 5349/tcp, the relay range/udp, IPv4 and IPv6, everything else dropped; `ufw logging off`;
  - [ ] the live log search after a relayed test call: `docker compose logs`, `docker logs` of Traefik and `journalctl` (kernel and system) contain no test client address;
  - [ ] the Uptime Kuma checks from the Oracle host (`/signaling/healthz` and the TURN allocation check);
  - [ ] renewal reminders for `mydata.rocks`, `ditscher.me` and the Contabo contract; CAA records for `gs.mydata.rocks`; two-factor login on the Cloudflare and Contabo accounts;
  - [ ] SSH hardening follow-up (non-root user, key only).

## Workflow follow-up

- `rebuild-web-client` and `add-turn-credentials` are archived (2026-11-02). Re-check this change's MODIFIED blocks (`signaling-relay` "Client ICE configuration" and "Metadata only, no IP logging", `device-setup` "Settings in Basic and Advanced" and "TURN server entry", `pairing` "Invite payload v3") against the merged specs before archiving.
- Second Critical Reviewer pass done (2026-10-23); its findings are answered in `design.md`. Security Architect review of tasks 2.2 and 3.4 diffs before merging them.
- Critical Reviewer pass on the 2026-11-02 revision: first verdict "rework"; every finding answered in `design.md` ("Review response (critical review 2026-11-02 …)"), then re-checked. Security Architect review of the 8.7, 9.2 and 9.3 diffs and of the 10.1 stack before merging them.
- Role reviews at the end of implementation: Technical Architect, Security Architect, UX.
- Project TURN is in the alpha (owner, 2026-11-02); the hardware round measures the relayed share and the server load for the beta sizing.
- `mise exec -- openspec archive alpha-connection-model --yes`, then `openspec validate --all --strict`.
