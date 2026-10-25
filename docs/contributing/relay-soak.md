# Relay soak and go/no-go (alpha-connection-model tasks 4.1 to 4.3)

The alpha finds the other device through a few public Nostr relays (`signaling-mailboxes`). Before Android and web build on that, the owner runs one night of measurements and decides go or no-go ([design.md section 9](../../openspec/changes/alpha-connection-model/design.md)). This page is the runbook for that gate (task 4.2) and for the operator mail (task 4.3).

**Go** needs at least 2 of 3 relays without data loss and a setup p95 below 3 s. **No-go** moves Nostr to beta: the alpha then ships with relay mailboxes (self-hosted or LAN) only.

## What exists

| Tool                                               | What it does                                                                                                                                                                     |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just soak [hours] [options]`                      | `scripts/spike/soak`: the `gs-soak` binary (`crates/gs-soak`) runs a real baby `Link` and a real parent `Link` in one process over real relays. Default 8 h on 3 relays.         |
| `scripts/spike/soak burst [options]`               | Offer, answer and candidates within about a second from two clients behind this IP, against each relay.                                                                          |
| `just relays-check [options]`                      | `scripts/spike/relays-check.mjs`: NIP-11, connect time, paced delivery (p50/p95), 2/4/8/16 KiB events, retention, `created_at` tolerance, burst limits, STUN Binding per server. |
| `scripts/spike/{nip11,relay-probe,stun-probe}.mjs` | The pieces of `relays-check`, each runnable alone (`node scripts/spike/relay-probe.mjs wss://host`).                                                                             |
| `relays-weekly.yml`                                | The weekly CI run of `just relays-check`; it only notifies and is not part of `ci-ok`.                                                                                           |

Events are signed by the Rust core through `gs-soak sign`, so there is no second Schnorr implementation. Everything uses throwaway keys, random content and a random pairing key per run. Reports hold relay hostnames and counters, never keys, frames, or addresses. All tools need network; without it they print `SKIPPED` (not a pass).

What the soak really exercises: presence, the challenge, `request_offer`, offer, answer, ICE bundles, the sealed envelopes, and the core's pacing, budget and parking per relay. What it fakes: the peer connection (a fake that only carries the messages of a verified attempt, with a fixed 150 ms for ICE once descriptions and a remote candidate are in) and the network between the two Links (both sit behind this machine's IP). Every `--loss-every-min` minutes (default 10) the link is cut, which makes the core run a full recovery over the relays.

## Task 4.2: the owner's run

Use a machine that stays awake and on mains power all night (`systemd-inhibit --what=sleep just soak 8` on Linux). Docker or a Rust toolchain is needed for the build (the first build takes a few minutes).

1. **Night soak (unknowns 1, 4, 9):** `just soak 8`. It prints one progress line per minute, writes a snapshot every 5 minutes and the final report to `.cache/relay-soak/soak-<unix time>.json`. A run killed half way leaves the last snapshot (`"complete": false`). To test fixed relays, add `--relays wss://a,wss://b,wss://c` (three distinct operators; the default is a random pick from the list like the core's `pick_relays`).
2. **Tag-only filter (unknown 3):** `just soak 1 --tag-only`. The harness drops the `authors` member of every `REQ`, so a relay that fails to match tags alone shows up as loss. Your own events then come back to you; the report counts them as `echoes`, and a growing `refused_park` or parking would mean the core treats them as forged.
3. **Burst behind one IP (unknown 2):** `scripts/spike/soak burst` and again with `--tag-only`. Two clients behind this IP publish 6 events each (an offer or answer plus 5 candidate bundles, 12 in all) within about a second. For a harder case: `--spacing-ms 0`. Run it once at home and once on the phone hotspot (CGNAT).
4. **LTE and IPv6-only (unknown 4):** put a laptop on the phone's hotspot (a hotspot with NAT64 if the carrier offers one) and run `just relays-check`, `scripts/spike/soak burst` and `just soak 1`. Docker's default network has no IPv6, so for an IPv6-only network build natively (`cargo build --release -p gs-soak`) and run `target/release/gs-soak soak --hours 1`. This approximates the Android path; a phone-only test stays part of the hardware round (`alpha-hardware-test.md`).
5. **`created_at` tolerance per relay software (unknown 9):** `just relays-check` lists, per relay, the offsets (seconds, negative is past) that were refused. Then `just soak 0.25 --parent-wall-offset-s 600 --relays <one relay>` shows what a parent whose clock is 10 minutes off does to that relay (`refused_created_at`, the `clock_skew` hint in `link.hints`).

### Reading the soak report

The markdown table and the JSON hold the same numbers. Per relay:

- **Sent / OK / Refused:** events the core published, `OK true` answers, refusals. Refusals are split into `created_at` (clock), `park` (rate limit, blocked, auth, proof of work: the core parks the relay) and `error`. `refusal_samples` has the relay's own words.
- **Deliverable:** events that were sent while the other Link had a subscribed socket (after `EOSE`) on that relay. **Delivered:** events that reached the other Link on that relay.
- **Lost:** deliverable, accepted (or not answered), never delivered within 20 s although the other Link's socket stayed up. This is the silent drop of spike unknown 8: a relay that says `OK` and does not forward. **Inconclusive:** the other side's socket changed meanwhile; not counted either way.
- **Unexpected closes / open failures:** the relay or the network ended an open socket, or a socket never opened (with the class: `dns`, `tcp`, `tls`, `handshake`).

Overall (`link`): `initial_setup_ms` is the first pairing from start to verified on both sides. `recoveries` are the times from the forced loss to both Links verified again (p50/p95/max, samples in `samples_ms`); a recovery longer than 120 s counts as failed with 120 s. `attempts_created` above one per recovery means the core restarted attempts.

Verdict and exit code (`just soak` prints the code and exits 0): **GO** (0), **NO-GO** (3), **INCONCLUSIVE** (4). A relay counts as clean with no loss and at least 5 deliverable events; the gate needs 3 relays tested, at least 5 recoveries, 2 clean relays and p95 below 3000 ms. Short or sparse runs are INCONCLUSIVE, never GO.

Caveats worth knowing before judging numbers:

- The core sends its opening presence to one relay at most every 3 minutes and otherwise waits for a 5 s repeat. A forced loss sooner than that after the previous setup inflates the recovery by about 5 s. `gs-soak` warns below 4 minutes; keep the default 10 for the go/no-go numbers.
- Recovery includes the fake 150 ms ICE (`--ice-ms`) and the core's own pacing of 2 events per second per relay, but not the 25 s the core needs to notice a dead link: the cut makes the peer connections fail at once, as ICE does after a network loss.
- Relays differ from run to run; one night on one random pick is a sample. Repeat with `--relays` for the ones that look doubtful.
- **The own signaling relay (mailbox 0).** The core also asks to open its own relay mailbox. The default `--relay-mailbox silent` never answers, so the public relays are measured alone. With `--relay-mailbox fail` the harness reports every open as failed, as a platform without an own server may. Then every report makes the core forget the parent's open `request_offer` (`signaling_down` clears it), the baby's answer is refused because its id no longer matches, and the attempt runs into its 20 s timeout: setups take about 22 s instead of about 5 s. This is a behaviour of the core and not of the relays; run `just soak 1 --relay-mailbox fail` once and record both numbers, because it decides what the platforms must report for a missing own server.
- **Silent relays.** A relay that answers `REQ` with `EOSE` and drops every `EVENT` (no `OK`, no delivery) counts as loss; the report names it. `relays-check` shows the same as `delivered 0/20`. It may be a kind filter: the spike's 2026-10-23 run used kind 25050 and this tool uses the core's kind 24861.
- `--trace` prints a timeline of the core's actions and the relays' answers, with the type and ids of every sealed message (`here`, `request_offer`, `sdp`, `ice`; never its content). Use it to see why a setup took long.

### Recording the result

1. Add a dated "Results" section to [`spike.md`](../../openspec/changes/alpha-connection-model/spike.md) answering unknowns 1 to 4 and 9: network, relays tested, duration, per-relay sent/delivered/lost, recovery p50/p95, refusals and the `created_at` table. Attach nothing generated; paste the markdown table.
2. Write the decision in [`design.md` section 9](../../openspec/changes/alpha-connection-model/design.md) (go, or no-go with the date) and tick task 4.2 in `tasks.md`. On no-go, tasks 5 and 6 drop their Nostr parts.
3. Update `data/nostr-relays.json` for any relay the runs showed to be unusable (`"status": "suspended"` and a note), then `pnpm run lists:check`.

## Task 4.3: contacting the operators

Write to each listed operator (the contact on the relay's NIP-11 document or website; one mail per operator) and record the answer in that relay's `notes` field in `data/nostr-relays.json`, replacing "Operator not yet contacted". Keep the mail short and send no personal data. Template:

> Subject: Low-volume ephemeral Nostr events on your relay (GuardedSleep)
>
> Hello,
>
> GuardedSleep is an open-source, privacy-first baby monitor (AGPL-3.0). Two devices of one family find each other through a few public Nostr relays. We would like to include your relay `wss://<host>` in the list that the app ships, and we would like to ask first.
>
> What the app sends: a few ephemeral events (kind 24861, end-to-end encrypted payloads of 2 to 12 KB) per connection setup, normally fewer than 10 per setup and at most 120 per hour and relay, never audio and never at a steady rate. Events are signed with throwaway keys and carry a daily rotating tag. Nothing identifies a person; the relay sees only the devices' IP addresses and the times of use.
>
> Questions: (1) Is this use acceptable for your relay? (2) Do you log or keep events or IP addresses, and for how long? (3) Which rate limits apply, and do you want a different limit or contact for us? (4) May we list the relay publicly, and may we remove it again if you prefer?
>
> You can answer by reply. We will note your answer in our public relay list and drop the relay if you would rather not be included.
>
> Thank you, and regards,
> <project contact>

Record, per relay: date, acceptable use (yes/no/conditions), logging and retention, rate limits, and who answered. A relay that does not answer within a month stays `active` with the note "no answer"; one that objects becomes `suspended`.

## The weekly job

`relays-weekly.yml` runs `just relays-check` every Monday and on `workflow_dispatch`, uploads the report as the artifact `relays-check` and, if an active entry fails, opens or updates one issue ("Weekly relay check: failing public relay or STUN entries"). Only that job holds `issues: write`. A GitHub workflow can be run by hand only once it exists on the default branch, so the first run happens after merging: `gh workflow run relays-weekly.yml`.

When it fails: open the artifact, run `just relays-check --only <host>` again, and if the entry stays down mark it `suspended` in `data/` with a note. A relay is judged failing when it does not connect, does not answer the subscription, delivers fewer than 90 % of 20 paced events, has a p95 of 3 s or more, or refuses 2 or 4 KiB events (it is probed a second time first). The build minimums (6 relays from 4 operators, 2 STUN servers) are checked by `pnpm run lists:check`.
