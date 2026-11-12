# Tasks

Branch from `origin/main` (built on `feat/talk-back`, ditschi/GuardedSleep#55, until it merges; then rebase onto `origin/main`). Commit after each task; `just test-affected` after each task.

## 0. Prerequisites

- [ ] 0.1 Archive `alternate-roles-and-apk-size` (after the owner's hardware test). Then rebase this change: its `pairing` requirement "One stored pairing per role" becomes a MODIFIED or REMOVED delta here, superseded by "Saved pairings and connection names"; `openspec validate --strict`.
- [x] 0.2 Owner confirmed (2026-11-11) design §9 I1 (released links open 60 s every 10 min; a dropped parent escalates while not monitored).
- [ ] 0.3 After `quiet-link-no-rtp` is archived: reword "Supervision lives only in the core Link" (Link or baby hub) and "Audio only on demand" (per link) as MODIFIED deltas here; `openspec validate --strict`.

## 1. Core

- [ ] 1.1 Protocol v3 additions: `noise_ack`, `reopen_join`, `noise_acked_elsewhere`, status field `other_parent` (optional when parsing). Verify: protocol unit tests, conformance vectors blessed and reviewed, `just api-check` after `just api-snapshot`.
- [ ] 1.2 `Link` release operation (no stop notice) and released-link probe openings. Verify: unit test that release emits no stop notice and the parent escalates.
- [ ] 1.3 `BabyHub`: link-tagged inputs and actions, one noise gate and alert log with per-session event ids, join window, grace, last-parent rule with new window, rejoin (rate-limited), resume flags, `set_mic` per link incl. probe exclusivity, noise-ack forwarding for the current event only, `other_parent` status. Verify: unit tests per spec scenario; configuration-invariant tests.
- [ ] 1.3a Talk-back integration (`add-talk-back` design §6, this change lands second): move the baby-side `TalkCore` (`src/link/talk_flow.rs`: talker, cooldown, 90 s / 10 min budget, noise pause) into `BabyHub`, one per hub with one noise pause for the shared gate; the talk-back switch per saved parent pairing; every other trusted link's status gets `talker: other` and `noise_paused`; talk playback actions carry `link` next to the attempt handle; the baby enables exactly the talking link's receiver. Verify: unit tests (second parent denied while one talks, budget and cooldown hub-wide, switch per pairing), the existing talk tests still pass.
- [ ] 1.4 Parent `Link`: send `noise_ack` only on stop sound/vibration; on `noise_acked_elsewhere` stop ringing for that event without counting it as acknowledgment; `other_parent` in the parent model, hidden while the own link is down. Verify: unit tests.
- [ ] 1.5 Façades: `BabyHub` through `ffi` and `wasm`. Verify: `just api-check`, WASM conformance.
- [ ] 1.6 Sim scenarios: both join, one never joins, late joiner via probe opening, one drops 3 min / 6 min, release without stop notice, last parent drops, remote rejoin, shared noise ack incl. stale ack and continuing noise, snooze not shared, test during noise, resume with two parents, unverified second attempt gets no audio; log canary with two PSKs. Verify: `scripts/cargo.sh test -p guardedsleep-sim`, traces reviewed.

## 2. Android media

- [ ] 2.1 One `AudioDeviceModule` and `AudioSource`, one `AudioTrack` per `PeerConnection`, `set_mic` per link (other links closed while a probe runs). Coordinate with `quiet-link-no-rtp` 2.2 (per-sender detach). Talk-back: one talk receiver per parent link, only the talking link's receiver enabled; the parent's talk line stays `encodings.active=false` outside a talk (`TalkSendControl`, gate AC-11). Verify: JVM tests with fake senders; gate.

## 3. Android platform

Group 3.1 with the connection parts of 4.1 (names, list, default, Settings → Connections) is independent of the hub and ships first as its own PR.

- [x] 3.1 `PairingStore`: up to 2 sealed records per role with own Keystore alias, local id and connection name; migration of the single slot; default pairing per role. Verify: JVM tests (cap, migration, forget one, name never in `toString`).
- [ ] 3.2 Baby `LinkController`: one `BabyHub`, one `LinkRunner` per saved pairing, one foreground service, persisted resume flags; parent: chosen pairing for the session, resumed after process death. Verify: `LinkController` JVM tests per scenario.
- [ ] 3.3 Update `add-offline-lan` (design and listener delta): the Android listener tries up to 2 admission keys, one per saved parent pairing. Verify: `openspec validate --strict`; JVM tests when built.
- [ ] 3.4 Server: raise `DEFAULT_MAX_LIVE_PER_ADDR` to 6 with a test; note the setting for the homelab deployment. Verify: `cargo test -p guardedsleep-server`.

## 4. UI and copy

- [ ] 4.1 Android: connection list and choice, Settings → Connections (rename, default, forget), "Add parent device", touch-wake status with count and names, other-parent line, "acknowledged on the other parent device", "Let the other parent device rejoin" (baby after unlock, parent menu). Stable test tags. Verify: Robolectric tests; Roborazzi baselines re-recorded in the pinned image.
  - Connection subset done in the `named-connections` PR: connection list and choice on the parent home, Settings → Connections (rename, default, forget), test tags, Robolectric tests and baselines. Remaining: "Add parent device", touch-wake status, other-parent line, shared-acknowledgment line, rejoin. Until `BabyHub` lands the baby runs one link and uses its default parent pairing.
- [ ] 4.2 Web parent: other-parent line and shared acknowledgment from the core models. Verify: Vitest; contract mirror test.
- [ ] 4.3 Copy in all locales (`machineTranslated` markers); the shared-acknowledgment and other-parent keys are `critical`. Verify: `just gen`, `just lint-text`.
  - Connection copy (`native.ui.connections.*`, all 20 locales) is done in the same PR; the shared-acknowledgment and other-parent keys remain.

## 5. Gate

- [ ] 5.1 Three-device emulator scenario: both parents join, B drops 6 min then is stopped and escalates, A acknowledges noise and B stops ringing, remote rejoin; battery and bytes for 30 min with one parent absent recorded in design §8. Verify: `just gate`.

## 6. Docs

- [ ] 6.1 `AGENTS.md` §9: multi-parent row "up to 2 parents via separate pairings (`multi-parent-pairings`)"; `docs/backlog.md`: replace the multi-parent `DEFERRED` items with the remaining parts (more than 2 parents, join request without QR, F-1a primary failover, web baby unit with two parents); `docs/ci-gates.md` row for the gate scenario if it is a new job step.
