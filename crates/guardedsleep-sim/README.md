# guardedsleep-sim

Deterministic host-side system simulation of the GuardedSleep link (ADR-015 AC-9/AC-10, ADR-016
Stage 1). Two `Link`s (baby, parent) run in virtual time against a simulated relay that mirrors
`crates/guardedsleep-relay` (welcome/members/peer, `to` routing, BABY_TAKEN, parent cap, WebSocket
liveness eviction within 20 s, v3 room-id format) and a fake media layer (synthetic SDPs with
`a=fingerprint` and `sendonly`/`recvonly` lines, so the pairing proof and baby SDP check run for
real). A seed fully determines latencies, jitter and fault timing. Ticks come every 250 ms after
activity and every 500 ms otherwise (the ADR-017 contract), and a stopped or dead link is not
ticked, so a simulated hour runs in a fraction of a second.

```bash
# in the build image (see scripts/core-build.sh); cargo is not on the host
cargo test -p guardedsleep-sim                         # all scenarios x 20 seeds, lint, determinism
cargo run -p guardedsleep-sim --bin gs-sim -- list
cargo run -p guardedsleep-sim --bin gs-sim -- run wifi-baby-90s --seed 3 --events
cargo run -p guardedsleep-sim --bin gs-sim -- run happy-path --trace-out /tmp/traces
cargo run -p guardedsleep-sim --bin gs-sim -- check --seeds 50
cargo run -p guardedsleep-sim --bin gs-sim -- bless-traces      # rewrite the curated traces
cargo run -p guardedsleep-sim --bin gs-sim -- lint-trace crates/guardedsleep-core/vectors/traces
```

## Log canary (AGENTS.md section 7.6)

`cargo test -p guardedsleep-sim` (and `gs-sim check`) run every scenario with the log canary on
(`run_scenario_with_canary`). The simulation pairs with the known synthetic PSKs; `canary.rs` builds
the list of every value a log must never contain: the PSK (raw, base64url, standard base64), the
envelope key, the relay admission key, the room id, the invite text, each run's seed, the parent
secret with its four HKDF subkeys and the `parent_pin` message text (both learned from the first
pairing, because the parent's core draws the secret at run time), an issued TURN password and, by
markers (`v=0`, `a=fingerprint:`, `a=ice-ufrag:`, `a=ice-pwd:`, `m=audio`), plaintext SDP. Each value is
searched raw, as lowercase and uppercase hex, as standard and URL-safe base64 at every alignment
(padded or not) and as percent-escaped standard base64.

"The log" is the `{:?}` text of every input the core receives, of every action and event it
returns and of every config: the worst a careless driver could print. Wire payloads (mailbox frames,
SDP in transit) may carry the room id and SDP by design, so for them only the secrets count. A leak
becomes a violation of the run that names the canary, the encoding, the actor and the time, never
the value. The scanner proves itself first (`CanarySet::self_check`, `gs-sim canary-scan
--self-check`), and the tests plant a leak per encoding.

Committed traces are recordings, not logs: they contain the synthetic test PSK and seed by design
and are guarded by the shareable lint instead (`lint_trace`: only synthetic keys, no real material).
The same scanner serves the emulator gate (`scripts/poc/canary-scan.sh`, `gs-sim canary-scan`).

## Scenarios (`scenarios/*.json`, `gs-scenario/1`)

Verbs (`at` in simulated ms): `start`, `stop`, `end`, `acknowledge`, `input` (any `LinkInput`),
`levels` and `talk_levels` (`peer`, `rms`, `for_ms`: the baby microphone's level and the incoming talk level, one `audio_level` / `talk_level` input every 100 ms; the fake platform answers `play_talk_chime` with `talk_chime_done` after 600 ms and `set_talk_mic` with `talk_mic_attached` after 30 ms),
`network_down` / `network_up` (per `peer`; the platform emits `network_available` on up),
`path_switch` (new address: old socket and media path stay dead), `signaling_down` /
`signaling_up` (server restart), `kill_process`, `restart_process` (fresh `Link`,
`paired_before: true`), `drop_data`, `delay`, `duplicate`, `reorder` (`from`, `p`, `ms`,
`for_ms`, `via: data|signaling`), `clear_faults`, `time_jump` (global Doze gap; `peer_traffic:
kept` delivers one heartbeat per link at wake-up), `stranger_join` (`mode`: `wrong_psk`,
`reflect`, `forged_offer`; `spoof_from` models a relay that forges `from`), `stranger_leave`,
`ghost_join` (silent socket holding a slot until the liveness sweep), `replay_stop` (`peer`: the relay re-delivers the last recorded signaling `stop_notice`), `reject_hello` (`for_ms`, default forever: every `hello` is answered with `UNSUPPORTED_PROTOCOL`), `relay_rewrite` (`mode`: an `sdp_rewrite` value, absent for an honest relay: the relay turns hostile or honest at run time). `restart_process` takes an optional `mailboxes` list (the new run's mailbox set, as after an app update). Relay options: `reuse_client_ids` (same client id per role on every connection),
`alias_rooms` (test-only, lets peers with different PSKs share a room), `parent_cap` and
`sdp_rewrite: "mitm_recompute"` (a PSK-holding man in the middle between two DTLS sessions: every SDP carries its own certificate per leg, it reuses the original envelope nonce so the parent tag stays valid, and it recomputes the pairing proof for each leg; it cannot compute the parent proof) and
`sdp_rewrite: "fingerprint_confusion"` (a relay that moves each SDP's real `a=fingerprint` into an
`s=` line and substitutes its own certificate; it forwards the DataChannel both ways, so only the
core's fingerprint parsing and DTLS binding keep the link from verifying). The fake media layer
reports the remote DTLS fingerprint (`attempt_dtls_fingerprint`) like the platforms do.

Peers take `sdp_direction` and `sdp_talk_direction` (the directions of the first and second audio m-line in their local descriptions, to break the direction rule) and `mailboxes` (baby: the set it sends after each verification) and `device_name` (parent). A scenario may cap its seeds with `max_seeds` (for many simulated hours).

`relay_count` (default 1) is the number of simulated relays the peers' default mailbox set names; mailbox `i` of a peer talks to relay `i` (a peer's own `mailboxes` replace the default, the URLs are labels). The verbs `signaling_down`, `signaling_up` and `reject_hello` take an optional `relay` index (default: every relay), `replay_stop` and `ghost_join` use `relay` too (default 0), and `relay_drop_signals` (`mode: on|off`, default on) makes a relay accept frames but deliver no `signal`. Trace configs carry the labels `wss://relay-<n>.invalid`, which the shareable lint allows.

`wall_start_s` sets the true wall clock at time 0 (default a synthetic value), `wall_offset_s` puts one device's clock off, and `only_local` is "Only same Wi-Fi or my own server". A socket whose path is dead drops frames silently, like a half-open connection.

Assertions per `peer`: `event` (+`where` subset match, `within_ms`, `count`/`count_min`/
`count_max`, `since_last_contact_ms`), `never`, `state` + `at_ms` (+`key`, `level`), `max_level`,
`reaches_level`, `sent_signal`, `joined`, `preauth` (every frame the peer sent on a socket that never said `hello`, the reachability probes, equals the given text). `expected_fail: "<reason>"` marks a known core problem;
the suite fails if such a scenario starts passing.

## Requirement to scenario mapping

Every requirement of the `alpha-connection-model` deltas for `signaling-mailboxes` and
`link-supervision` names at least one simulation scenario (`scenarios/`) or, where the behaviour is
a pure function of one device's inputs, the core test that holds it (`crates/guardedsleep-core`).
"Pending" marks requirements whose behaviour is built by a later task of the change; that task adds
the scenario.

### `signaling-mailboxes`

| Requirement                          | Scenario or test                                                                                                                                                                        |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parallel mailboxes                   | `two-relays-one-dies-during-setup`, `two-relays-a-flapping-relay-does-not-stall-setup`, `two-relays-one-down-for-the-whole-run`                                                         |
| De-duplication after authentication  | `two-relays-both-deliver-the-same-envelope`; core `envelope::tests::a_corrupted_copy_first_does_not_block_the_genuine_copy`, `a_duplicate_through_a_second_mailbox_is_processed_once`   |
| Mailboxes closed while connected     | `night-holds-no-mailbox`, `all-mailboxes-dead-at-reachability-check`, `two-relays-all-mailboxes-dead-at-reachability-check`, `signaling-restart-connected`, `parent-restart`            |
| Sealed signaling envelope            | core `envelope::tests` (round trip, bit flips, wrong key) and the envelope vectors in `conformance.json`; `attack-psk-holder-refused-after-pinning`                                     |
| Session confirmation                 | core `envelope::tests::an_old_here_makes_one_challenge_reply_and_never_confirms`, `a_delayed_here_from_a_superseded_session_does_not_displace_the_newer_one`; `wrong-psk-first-pairing` |
| Session-bound freshness              | `stored-stop-after-parent-restart`, `stored-baby-stop-after-baby-restart`, `attack-replayed-stop-after-reconnect`                                                                       |
| Replay state per run                 | core `envelope::tests::a_replayed_envelope_is_rejected_and_changes_nothing`, `a_reflected_envelope_is_rejected_by_role`; `attack-reflection`                                            |
| Relay-visible data                   | core `parent_pin::the_secret_never_goes_through_a_mailbox_or_a_log_line`; the `preauth` assertion of `night-holds-no-mailbox` (a check sends a ping and nothing of the pairing)         |
| Delivery is acknowledged by the peer | `two-relays-one-accepts-but-never-delivers`; core `delivery::a_request_nobody_answers_is_sent_once_more_with_the_same_envelope`                                                         |
| Mailbox update                       | `mailbox-update-after-relay-churn`; core `parent_pin::the_baby_sends_its_mailbox_set_after_verification_and_the_parent_stores_it`                                                       |
| Project signaling server             | pending: tasks 5.x and 9.2 (data file, default invite, server TURN)                                                                                                                     |
| Disclosure and off switch            | pending: tasks 5.3 and 6.2 (UI); the core side is `LinkConfig.only_local`, reserved                                                                                                     |

### `link-supervision`

| Requirement                             | Scenario or test                                                                                                                                                            |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Supervision lives only in the core Link | `wifi-baby-20s`, `wifi-parent-20s`, `path-switch-baby`, `path-switch-parent`, `parent-restart`; core `link::tests::construction_needs_a_32_byte_seed`                       |
| Offer direction                         | `single-attempt-setup`, `two-relays-both-deliver-the-same-envelope`                                                                                                         |
| Accepted signaling senders              | `attack-forged-offer`, `attack-forged-offer-spoofed`, `attack-stranger-parent-cap`                                                                                          |
| Deterministic attempt identity          | core `link::tests::fresh_attempt_ids_are_deterministic_128_bit_hex`, `traces::committed_traces_replay_identically`, `link::tests::construction_needs_a_32_byte_seed`        |
| Attempt scheduling                      | `wifi-baby-20s`, `wifi-parent-90s`                                                                                                                                          |
| Error classification                    | `stale-baby-socket-rejoin`, `stale-parent-socket-rejoin`, `attack-unsupported-protocol-during-outage`; the per-relay refusal rule: core `link_pair::unsupported_protocol_*` |
| Never give up while monitoring          | `all-mailboxes-dead-at-reachability-check`, `signaling-restart-during-outage`; core `link_pair::a_member_that_never_answers_gets_presence_on_a_backoff_and_no_attempt`      |
| Deliberate stop is not an outage        | `stop-baby`, `stop-parent`                                                                                                                                                  |
| Authenticated signaling stop            | `attack-forged-stop-outage`, `stored-stop-after-parent-restart`, `attack-replayed-stop-after-reconnect`                                                                     |
| Signaling transport liveness            | `signaling-restart-connected`, `signaling-restart-disconnected`; core `link_pair::stale_signaling_socket_is_detected_after_35_seconds_without_traffic`                      |
| Clock and time jumps                    | `time-jump-no-peer-loss`, `time-jump-with-peer-loss`; core `parent_pin::clocks_more_than_ten_minutes_apart_raise_the_hint_on_both_devices`                                  |
| Loss detection                          | `drop-data-baby`, `ac4-baby-killed`, `baby-restart`                                                                                                                         |

### Removed with Nostr, spec delta pending

The Nostr mailbox rules (daily epoch and tag, event budgets, parking, the freshness window and the
relay hygiene rules, clock refusals) no longer exist in the core, and no requirement of the deltas
above names them; the as-built text of design section 3 is superseded by section 11. Their
scenarios were deleted with the simulation's Nostr relays: `nostr-clock-five-minutes-off-at-the-epoch-change`,
`nostr-one-hour-outage-stays-within-budget`, `nostr-rate-limited-relay-is-parked`,
`nostr-relay-closes-right-after-eose`, `nostr-retention-and-failing-attempts-never-park-the-relay`
and `nostr-third-party-replay-does-not-park-the-relay`. The generic ones live on as relay-only
twins (the `two-relays-*` scenarios, `single-attempt-setup`, `all-mailboxes-dead-at-reachability-check`
and the stored-stop scenarios above).

## Traces

`--trace-out` and `bless-traces` write `gs-trace/1` documents (`class: simulated`,
`privacy: shareable`, synthetic PSK `GS-SYNTHETIC-PSK...`, synthetic `SYNT...` fingerprints, no
wall-clock times) per `Link` instance. The curated set is declared by `export_traces` in the
scenario files and lives in `crates/guardedsleep-core/vectors/traces/`; Rust
(`tests/traces.rs`), WASM (`js/conformance.test.mjs`) and Kotlin (`ConformanceTest`) replay it.
After an intentional core change run `bless-traces` and review the diff. `lint-trace` implements
test-schemas section 1.4.

Names changed with the mailbox set (`alpha-connection-model` task 3.1): `open_signaling` /
`send_signaling{text}` / `close_signaling` became `open_mailbox{id,kind,url}` /
`mailbox_send{id,frame}` / `close_mailbox{id}`, and `signaling_opened` / `signaling_message{text}` /
`signaling_closed` became `mailbox_opened{id}` / `mailbox_frame{id,bytes}` /
`mailbox_closed{id,reason}`. The relay is mailbox `0`, `url` is empty (the platform still owns the
URL list) and `reason` is `closed`. The curated traces were re-blessed with that mapping and
checked to be identical otherwise.
