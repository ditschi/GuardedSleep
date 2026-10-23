# Tasks

Alpha (T0). Builds on `rebuild-web-client` group 1 (core invite codec) and group 4 (web link driver). Folds in `add-offline-lan` and `add-stun-server-choice`. Each task is a brief for the `implementer`: layer rules from AGENTS.md §5.1 (all supervision, mailbox choice and timing in the core; the platforms own the sockets), privacy guardrails as amended by this change, commit after each task, and `just test-affected` after each task.

## 1. Guardrails and decisions in place

- [ ] 1.1 Amend AGENTS.md §7.1 (vetted EU mailboxes and STUN by default with disclosure and an off switch), §7.3 (sealed signaling envelope with a standard AEAD), §7.8 (the LAN listener in the alpha) and the §9 rows (offline LAN: alpha; LAN ↔ internet handoff: parallel mailboxes; TURN: user-entered only in the alpha), plus the matching `.cursor/rules` mirror. Verify: `just lint-text`.
- [ ] 1.2 Withdraw `add-offline-lan` and `add-stun-server-choice` (delete the change directories, note it in the decision archive README), and move the backlog items they referenced. Verify: `openspec validate --all --strict`.

## 2. Core: sealed envelope

- [ ] 2.1 `envelope` module in `guardedsleep-core`: key `HKDF-SHA256(PSK, "guardedsleep signal seal v1")`; XChaCha20-Poly1305 (`chacha20poly1305`, added with `cargo add`); the platform supplies the 24-byte nonce as an input (sans-IO); associated data is version, role, epoch and attempt id; 4 KiB padding buckets; envelope id; a replay window. Rejects bad tags, its own role, out-of-window epochs and replays. Parser lint denies; unit tests and proptests (round trip; no reflection or replay); no `Debug` of keys. Verify: `scripts/cargo.sh test -p guardedsleep-core`, `just deny` with the dependency count re-blessed and justified.
- [ ] 2.2 Envelope conformance vectors (valid, wrong key, own role, old epoch, replay, truncated, wrong bucket) through every binding, and a `cargo-fuzz` target for envelope parsing. Verify: core tests, `just test-wasm`, the fuzz target builds.

## 3. Core: mailbox set and Nostr

- [ ] 3.1 Mailbox set in `Link`: kinds `relay`, `lan` and `nostr`; actions `open_mailbox`, `send`, `close_mailbox`; inputs `mailbox_opened`, `mailbox_closed`, `mailbox_received`; fan-out to every open mailbox and de-duplication by envelope id; presence (`here`) with backoff replacing relay `members`/`peer` as the attempt trigger; the outage of every mailbox escalates as a relay outage does today. Bless the sim traces. Verify: core tests and new `guardedsleep-sim` scenarios ("one of three mailboxes dies during setup", "all mailboxes down", "Wi-Fi to mobile data").
- [ ] 3.2 Nostr codec: daily epoch, tag `HMAC(PSK, "gs-mailbox/1|"‖epoch)`, per-role per-epoch BIP-340 keys (`k256` with `schnorr`), event build and verify, a fixed ephemeral kind (pick and record it in the spec), REQ/EVENT/OK/NOTICE/CLOSED handling, parking refusing relays, a ±15 min dual epoch, the events-per-attempt budget (≤ 6 per side) and no heartbeats. Verify: core tests; NIP-01 vectors (event id and signature) checked against an independent implementation; proptest that no PSK-derived value other than the tag and public key is ever emitted.
- [ ] 3.3 Invite v4 in the core codec (`{"v":4,"k","m","x"}`, 1–6 mailboxes, ≤ 480 bytes, kind-specific URL rules, v3 rejected), replacing v3 in the ffi and wasm façades and `packages/core-wasm`. Update the conformance vectors and run `just api-snapshot`. Verify: `just api-check`, the core and core-wasm tests.
- [ ] 3.4 Curated lists: `data/nostr-relays.json` and `data/stun-servers.json` (seeded from `spike.md`, with operator, country, policy, software and vetted date), validated and generated into both clients by a small generator (drops entries older than 12 months, fails on duplicate operators). The relay pick is deterministic from the PSK, 3 relays from distinct operators. Verify: generator tests; `just lint` fails on a stale or duplicate entry.

## 4. Relay and server

- [ ] 4.1 `guardedsleep-relay` and the server carry the sealed envelope opaquely; protocol and docs updated; local mode from `add-offline-lan` 1.3 completed (static UI and loopback API on 127.0.0.1/::1, `Origin`/`Host` checks, LAN signaling port, forwarding for a second PC, admission Required on LAN). Verify: `scripts/cargo.sh test -p guardedsleep-relay -p guardedsleep-server`, `just smoke`, a security test for a foreign `Origin`.

## 5. Android

- [ ] 5.1 Platform mailbox clients (one WebSocket per mailbox, opened and closed only on core actions), nonce supply, the ICE config from the core lists (own relay STUN plus 2 listed STUN servers; user TURN only), and the "Use only my own server" switch. Verify: JVM tests with fake sockets; a lint check that no Google STUN host appears in source.
- [ ] 5.2 Embedded LAN listener on the `relay` crate (only while monitoring, Wi-Fi and multicast locks, opaque NSD token, admission, 5 s unauthenticated close) and the private-address cleartext rule (network security config plus one platform check; the release-APK assertion is updated). Verify: JVM tests, `just verify-android`, `just build-android` with the release check.
- [ ] 5.3 Settings → Connection (operator disclosure, own-server-only, own relay URLs, TURN, connection-type diagnostics), setup disclosure, "No direct connection between these networks" copy, all copy in `locales/*.json` with critical tags where it belongs to connection state. Verify: Roborazzi recorded in the pinned image, `just verify-android`.
- [ ] 5.4 Emulator gate scenarios: LAN-only pairing without internet, Nostr-only pairing (public relays reached from the emulator, skipped with `SKIPPED:` when offline), and a network switch during monitoring. `docs/ci-gates.md` rows. Verify: `DOZE_MINUTES=2 just gate`.

## 6. Web

- [ ] 6.1 Web link driver: mailbox WebSockets, nonce from `crypto.getRandomValues`, ICE config from the lists, own-server-only; local PC mode detection (`http://localhost` served by the binary) adds the `lan` mailbox. Verify: Vitest with fake sockets; the storage test still finds no PSK or derived values.
- [ ] 6.2 Settings → Connection, setup disclosure, no-direct-path copy, diagnostics view, in the web catalogue groups. Verify: Vitest, Playwright with axe, `just lint-text`.
- [ ] 6.3 Playwright: two browser contexts pair through a local Nostr relay test double (a minimal NIP-01 relay in `apps/e2e` test utils, no public relays in CI) and through the local PC mode; a mailbox drop during setup still verifies. Verify: `just e2e`.

## 7. Integration

- [ ] 7.1 Host simulation and log canary: no PSK, tag, key or plaintext SDP in any log or relay-visible byte in any encoding. Verify: `scripts/cargo.sh test -p guardedsleep-sim`, the canary test.
- [ ] 7.2 Manual soak, owner-run: one night on three public EU relays with real devices (home Wi-Fi ↔ mobile data); record reconnects and the connection types from the diagnostics view in `spike.md`. Verify: the results are recorded.
- [ ] 7.3 `GS_STRICT=1 just ci` green on the wave branch.

## Workflow follow-up

- Archive `rebuild-web-client` first. Its deltas also modify `pairing` "Invite payload v3" and `signaling-relay` "Client ICE configuration", so re-check these MODIFIED blocks against the merged specs before archiving this change (copy any scenarios they added).

- Role reviews: Technical Architect (mailbox set in the core, layers), Security Architect (envelope, Nostr key and tag derivation, LAN listener, disclosure), Product Owner and UX (alpha scope, copy, Persona A–C), then the Critical Reviewer gate (scope, security model and guardrail changes); the originator answers every CRITICAL and IMPORTANT finding.
- Owner decides project TURN for beta with the 7.2 data (alpha: no default TURN, owner 2026-10-23).
- `mise exec -- openspec archive alpha-connection-model --yes`, then `openspec validate --all --strict`.
