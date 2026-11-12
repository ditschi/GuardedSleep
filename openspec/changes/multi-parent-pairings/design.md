# Design

Roles applied: Product Owner, Technical Architect, Security/Privacy Architect, UX; Critical Reviewer findings and answers in §9.

## 1. How a second parent joins

**Option A — one pairing per parent device (recommended).** The baby unit holds up to 2 independent pairings and runs one core `Link` per pairing.

- Pros: no change to pairing, pins, envelope or relay; every pairing keeps all its proofs; removing a parent is forgetting its pairing (no re-keying of the other); per-parent listen, snooze and escalation come for free because each `Link` keeps its own state; the core already runs several `Link`s in one process (no global state; one CSPRNG per `Link`, `tests/common/mod.rs`).
- Cons: one QR scan per parent device; the baby keeps one room per parent; a cross-link layer is needed for the shared rules.

**Option B — one pairing, several parents sharing the PSK.**

- Pros: one room; matches the original Phase 2 sketch.
- Cons: the `Link` is single-peer throughout (one attempt, one peer session, one pin, scalar listen and escalation state, `link.rs`); the baby closes its mailbox once linked, so a second parent could not reach it; it needs multiple pins with per-parent tag keys, an open invite after the first pin, per-member targeting and revocation by re-keying. Size L in core and pairing, plus a new security model.

Recommendation: A.

## 2. Baby hub (core)

A new sans-IO type `BabyHub` in `guardedsleep-core` owns at most 2 baby `Link`s, identified by a hub-local `LinkId`, and is the only thing the baby platform talks to while monitoring. Inputs are the existing `Link` inputs tagged with `LinkId` plus hub inputs (`reopen_join`); outputs are the existing actions tagged with `LinkId` plus `set_mic { link, enabled }`. The platform keeps doing I/O only. The parent side keeps a plain `Link` (one active pairing).

Rules (all timers on the core's monotonic clock input; "trusted" = pin-proven attempt that is alive):

| Rule        | Behaviour                                                                                                                                                                                                                                                                                                       |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Release     | A new `Link` operation that closes the attempt and the mailboxes **without** a stop notice, so the parent sees an outage and escalates (never `peer_stopped`). A released link opens its mailboxes for 60 s every 10 min (`RELEASED_PROBE`), so its parent can come back alone.                                 |
| Join window | Opens at monitoring start; closes at `max(start + 2 min, first trusted + 2 min)`. Links never trusted this session are released when it closes.                                                                                                                                                                 |
| Grace       | A trusted link that goes down while another link is trusted recovers for 5 min, then is released. A deliberate parent stop releases it at once (the baby link does not wait in `peer_stopped`).                                                                                                                 |
| Last parent | While no link is trusted, no link is released; released links reopen; the next trusted link opens a new join window.                                                                                                                                                                                            |
| Rejoin      | `reopen_join` (unlocked baby UI, or a message from a trusted parent; at most once per 10 s; no-op when nothing is released) opens released links for 2 min.                                                                                                                                                     |
| Resume      | After process death, links trusted in the interrupted session start in grace; the hub persists only non-secret flags (pairing local id → trusted this session).                                                                                                                                                 |
| Noise gate  | One gate and one alert log in the hub; every noise event gets a per-session id, carried in `noise_alert`; a rejoining parent receives missed events since its release.                                                                                                                                          |
| Mic         | `set_mic { link, enabled }`: noise opens every trusted link; listen and test open only the asking link; while a probe tone runs every other link is closed.                                                                                                                                                     |
| Noise ack   | Only "stop sound" / "stop vibration" send `noise_ack { event }`; forwarded only for the current event as `noise_acked_elsewhere { event }`; it stops the other parent's ringing but is not its acknowledgment (its noise escalation timer keeps running). Never for connection alarms, tests, snooze or listen. |
| Status      | Each baby status carries `other_parent: none                                                                                                                                                                                                                                                                    | connected | absent` for that recipient; the parent hides it while its own link is down. |

Talk-back (`add-talk-back` design §6): the hub owns the single talk arbitration (`TalkCore`: one talker, cooldown, budget, noise pause) next to the single noise gate; the talk-back switch is per saved parent pairing; other trusted links see `talker: other`; talk playback actions carry `link`.

Constants: `JOIN_WINDOW = 120 s`, `PARENT_GRACE = 300 s`, `RELEASED_PROBE = 60 s every 600 s`, `REJOIN_MIN_INTERVAL = 10 s`, `MAX_PARENT_LINKS = 2`, checked by the configuration-invariant tests.

## 3. Protocol (additive, v3)

New DataChannel messages: parent→baby `noise_ack { event }`, `reopen_join`; baby→parent `noise_acked_elsewhere { event }`. New optional fields: `event` in `noise_alert`, `other_parent` in the status. Unknown messages are already ignored by older peers, so a mixed pair keeps working without the new features. Conformance vectors and traces blessed and reviewed by hand. No wire change to envelope, invite or proofs.

## 4. Microphone per link

**Option A — one audio source, one track per parent link (recommended).** Android: one `AudioDeviceModule` and `AudioSource`, `createAudioTrack` per `PeerConnection`, `setEnabled`/sender detach per track (compatible with `quiet-link-no-rtp`'s per-sender detach). The noise gate keeps one capture tap.

- Pros: one microphone open; per-link gating at the WebRTC sender.
- Cons: media engine refactor; probe-tone injection must run per track.

**Option B — one engine per link.** Cons: N audio device modules opening the microphone at once; undefined on many devices. Discarded.

The probe tone is mixed into the shared capture buffer (`CaptureTap`), upstream of every track, so it cannot be isolated per track without a second source; instead every other link is closed while the probe runs (a few seconds).

## 5. Android platform

- `PairingStore` per role becomes a list of up to 2 sealed records, each with its own Keystore alias and a stable local id; `alternate-roles-and-apk-size`'s single slot migrates as the first entry.
- `LinkController` (baby) runs one `BabyHub` and one `LinkRunner` per saved pairing on one foreground service; parent keeps one runner for the chosen pairing.
- LAN listener: `add-offline-lan` (design.md, "The fix" under admission) says the Android listener serves one pairing and identifies the room by its admission key. With two parent pairings it tries up to 2 keys, as the PC relay already loops over up to 4; that change is updated before either is built (task 3.3).
- TURN: at home 4 credential holders (2 baby links, 2 parents) can share one address; the server default `DEFAULT_MAX_LIVE_PER_ADDR = 3` (`guardedsleep-server/src/turn.rs`) is raised to 6, and the homelab config follows.

## 6. UX

- Single saved pairing: today's screens, no list.
- Parent home with 2 saved baby pairings: the default connection preselected with its name, "Change" opens the list; Settings → Connections: rename, set default, forget.
- Baby home: "Add parent device" (shows a new invite) only while not monitoring and with fewer than 2 parents saved; touch-wake status "2 of 2 parent devices connected" / "1 of 2 …"; after kiosk unlock "Let the other parent device rejoin".
- Parent monitor screen: a quiet line "Other parent device: connected / not connected"; a noise alert acknowledged elsewhere stops and shows "Acknowledged on the other parent device".
- Names: free text, 1–30 characters, default the peer device name, never sent to the peer.

## 7. Security and privacy

- Trust: every saved parent is a fully trusted parent of this baby (it scanned the baby's QR). Audio separation is between verified and unverified attempts, not between the two parents; both hear room noise.
- Mic gate per link closes the risk of audio reaching an attempt that has not passed its proof while another link opened the microphone.
- Metadata: during the join window and while a parent is missing with no other parent connected, the baby keeps one waiting room per pairing open, as it does today for its single pairing. Otherwise rooms close (join window, grace), so the night-long open room is limited to "nobody is connected".
- Metadata: two rooms used from one address with correlated timing let a relay operator link the two pairings to one household; it learns no more than that (no names, no content).
- The other-parent status tells each parent whether the other is monitoring; both are trusted parents of the same baby, so this is intended.
- Trust: a compromised second parent could stop the other's _ringing_ for current noise events, but not its noise escalation, connection alarms, snooze or listening (§2 Noise ack).
- Connection names are local, encrypted with their pairing, never logged or sent.
- A remote `reopen_join` comes only from a verified parent and only admits pairings already saved on the baby; it cannot add a device.
- The log-canary test runs with two PSKs in the new sim and gate scenarios.

## 8. Cost

Each waiting link holds one relay socket per mailbox (app ping 15 s, server ping 10 s); only during the join window, while a parent is missing alone, or within grace. Measured in the gate run (battery and bytes for 30 min with one parent absent).

## 9. Critical review

Reviewer verdict on the first draft: rework. Every CRITICAL and IMPORTANT finding is answered below.

| #   | Finding                                                                                            | Answer                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | Hub "stop" would send a stop notice, so the dropped parent goes `peer_stopped` and never escalates | **Fixed.** New core release without stop notice (§2 Release, spec "Released link", scenario "Release sends no stop notice").                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| C2  | Shared ack had no event identity and would cancel the other parent's escalation                    | **Fixed.** Per-session event id from the hub's single gate; only stop-sound/vibration ack, current event only, stale ids ignored; it never counts as the other parent's acknowledgment; snooze and listen never shared (spec "Shared noise acknowledgment", "Limits of shared acknowledgment").                                                                                                                                                                                                                                                               |
| I1  | A released parent cannot recover alone and escalates all night; late starter at 22:00 locked out   | **Fixed; owner confirmed 2026-11-11.** Released links open for 60 s every 10 min, so a returning or late parent connects within 10 min without help; a new join window after the last-parent rule. The dropped parent still escalates while it is not monitored (intended: it is not monitored).                                                                                                                                                                                                                                                              |
| I2  | Spec conflicts                                                                                     | "Deliberate stop is not an outage" and "Process death" now MODIFIED here. "Supervision lives only in the core Link" and "Audio only on demand" are modified by `quiet-link-no-rtp`: this change is archived after it and then rewords them (task 0.3); until then the ADDED "Baby hub" and "Audio per parent link" state the extension. "Forget pairing" (from `alternate-roles-and-apk-size`, stop monitoring) stays valid: forgetting one parent stops monitoring, then deletes exactly that pairing (task 0.1). `add-offline-lan` listener keys: task 3.3. |
| I3  | Probe-tone isolation claim false                                                                   | **Fixed.** Other links are closed while the probe runs (§4, scenario "Test during noise").                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| I4  | Shared noise gate undefined                                                                        | **Fixed.** One gate and alert log in the hub; missed events on rejoin (spec "Shared noise gate and event ids").                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| I5  | TURN cap 3 per address too low                                                                     | **Fixed.** Default raised to 6 (task 3.4, homelab config).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| I6  | Process death restarts the window and locks out a parent in backoff                                | **Fixed.** Resume puts previously trusted links in grace; parent resumes the session's pairing (MODIFIED "Process death").                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| M1  | `reopen_join` abuse                                                                                | Rate-limited to once per 10 s, no-op without released links; vacuous sentence removed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| M2  | "verified" ambiguous                                                                               | "Trusted" = pin-proven and alive, defined in "Baby hub".                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| M3  | Stale other-parent line; privacy                                                                   | Hidden while own link is down; privacy noted in §7.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| M4  | Web baby unit contradicted                                                                         | "Add parent device" is scoped to the Android baby unit.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| M5  | Relay can link two rooms to one household                                                          | Documented in §7.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| M6  | Named connections and parent-side list are independent                                             | Kept in this change but built first as their own task group and PR (group 3.1 + 4.1 subset), so they can ship before the hub.                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| M7  | Missing scenarios                                                                                  | Added: release without notice, late joiner, stale ack, snooze not shared, test during noise, resume with two parents, own link down.                                                                                                                                                                                                                                                                                                                                                                                                                          |

## Discarded alternatives

- §1 Option B (shared PSK), §4 Option B (engine per link).
- Hub logic in the platform: violates "safety logic lives once, in the Rust core".
- Sharing connection-alarm acknowledgment: a lost link means _this_ parent is not monitoring; nobody else can acknowledge that.
- Sending the acknowledging parent's name: avoids moving device names between parents; "the other parent device" is unambiguous with a cap of 2.
