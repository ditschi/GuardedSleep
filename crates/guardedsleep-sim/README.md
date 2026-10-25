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

## Scenarios (`scenarios/*.json`, `gs-scenario/1`)

Verbs (`at` in simulated ms): `start`, `stop`, `end`, `acknowledge`, `input` (any `LinkInput`),
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

Peers take `mailboxes` (baby: the set it sends after each verification) and `device_name` (parent). A scenario may cap its seeds with `max_seeds` (for many simulated hours).

Public Nostr relays (`alpha-connection-model` task 3.5): `nostr` lists simulated NIP-01 relays (`url`; `retention_s` keeps ephemeral events and serves them to new subscriptions like strfry's 300 s; `drops` accepts events with `OK true` and never delivers them; `created_at_window_s` refuses events too far from the relay's clock; `rate_limit {events, per_ms}`; `down`). A peer's `mailboxes` name them with `kind: nostr` (the core opens them; the own relay stays mailbox 0), `wall_start_s` sets the true wall clock at time 0 (default a synthetic value; pick one just before an epoch change at 12:00 UTC to cross it), `wall_offset_s` puts one device's clock off, and `only_local` is "Only same Wi-Fi or my own server". Verbs `nostr_down` / `nostr_up` (`id`: the relay url) and `nostr_drop_delivery` (`id`, `mode: on|off`). A socket whose path is dead drops frames silently, like a half-open connection. `close_after_eose` makes a relay close every socket right after EOSE; verb `nostr_replay` (`id`, `for_ms`, `gap_ms`) lets a third party re-deliver the events it captured from a relay; assertion `nostr_closes_max` bounds how many Nostr sockets the peer closed. Assertions `nostr_sent_max` (no relay got more `EVENT`s than this from the peer within `within_ms`), `nostr_used` (the peer sent to each of these relays) and `nostr_probe` (every frame sent while the link was connected is the throwaway `REQ` or a `CLOSE`, with no author and no daily tag or key of the pairing).

Assertions per `peer`: `event` (+`where` subset match, `within_ms`, `count`/`count_min`/
`count_max`, `since_last_contact_ms`), `never`, `state` + `at_ms` (+`key`, `level`), `max_level`,
`reaches_level`, `sent_signal`, `joined`, `preauth` (every frame the peer sent on a socket that never said `hello`, the reachability probes, equals the given text). `expected_fail: "<reason>"` marks a known core problem;
the suite fails if such a scenario starts passing.

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
