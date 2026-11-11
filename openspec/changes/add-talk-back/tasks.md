# Tasks

Branch `feat/talk-back` from `origin/main`. Commit after each task; `just test-affected` after each task. Every implementation task is a brief for the `implementer` sub-agent: layer rules of AGENTS.md §5.1 apply (decisions in the core, I/O in the platform, UI renders contract models only, no literals in UI code).

## 0. Coordination

- [ ] 0.1 Record the owner decisions in design §11 and fold the chosen options into specs and tasks (move rejected options to "Discarded alternatives"). Verify: `openspec validate add-talk-back --strict`.
- [ ] 0.2 Check whether `multi-parent-pairings` (PR ditschi/GuardedSleep#54) has landed. If yes, tasks 1.2 and 1.3 implement talker arbitration and the noise pause in `BabyHub`; if not, add a task to `multi-parent-pairings/tasks.md` that moves them into the hub. Verify: the chosen path is written into design §6.

## 1. Core (`crates/guardedsleep-core`, façades, sim)

- [ ] 1.1 SDP rule: replace `audio_is_sendonly` with a direction check of exactly two audio m-lines (baby: `sendonly`, `recvonly`; parent: `recvonly`, `sendonly`|`inactive`, port 0 counts as inactive), applied to both roles; failure keys `baby_sdp_bad_direction` / `parent_sdp_bad_direction` (add to `message_keys()` only if user-visible). Parser-module lints (`unwrap_used`, `indexing_slicing`, …) apply. Verify: unit tests for each scenario of the "Sans-IO interface contract" delta, sim scenario `baby-sdp-not-sendonly` updated, new `parent-sdp-sends-monitoring`; fake SDP in `guardedsleep-sim/src/raw.rs` gets two m-lines.
- [ ] 1.2 Protocol v3 messages `talk_request { id }`, `talk_grant { id, max_ms }`, `talk_deny { id, reason }`, `talk_end { id }` and `device_status` fields `talk`, `talker` (additive; unknown reasons parse as `None`). Verify: `parse_data_v3` vectors (valid, missing fields, unknown reason, extra field) blessed with `examples/bless_vectors.rs` and reviewed by hand.
- [ ] 1.3 Talk state machine per design §3 and noise pause per §5: parent input `set_talk`, baby inputs `talk_chime_done`, `set_talk_back { enabled, level }`; actions `set_talk_mic`, `set_playback_ducked`, `play_talk_chime`, `set_talk_playback`; events `talk_changed`, `talk_active`, `noise_paused`; constants per §3 with configuration tests; hand-written `Debug` for new `LinkAction` variants. Trust gate = `live_verified()`. Verify: unit tests for every `talk-back` scenario that is core-observable (grant, deny reasons, rate, deadline without parent messages, loss during talk, release before grant, audio-without-grant discards by not enabling playback), noise-pause tests (quiet room no alert, cry during talk alert within limit + tail + hold, running event continues), `link` vectors.
- [ ] 1.4 Façades: `just api-snapshot`, review the `api.txt` and `core.d.ts` diffs. Verify: `just api-check`.
- [ ] 1.5 Sim: scenarios `talk-happy-path`, `talk-deadline`, `talk-link-loss`, `talk-off`, `talk-noise-pause-quiet`, `talk-cry-during-talk`, `talk-parent-inactive-line`; the log canary runs in each. Verify: `scripts/cargo.sh test -p guardedsleep-sim`, traces blessed and reviewed.

## 2. Android media (`apps/android/platform`)

- [ ] 2.1 Spike (blocking for 2.3): on the emulator and one real device, show (a) a `RECV_ONLY` talk transceiver on the baby and a `SEND_ONLY` one on the parent negotiate with the pinned webrtc-sdk, (b) remote talk playout on `USAGE_ALARM` reaches the loudspeaker while the app's DND rule is active and the ringer is silent, (c) setting and restoring the alarm stream volume works, (d) a connected Bluetooth headset case. Record results in design §7. Verify: a written result plus an instrumented or gate check for (a)–(c).
- [ ] 2.2 Parent capture: `RECORD_AUDIO` requested at the first press with the plain reason; a parent-only `AudioSource` with voice processing on; mic track created on `set_talk_mic { true }` and attached to the talk sender, detached and disposed on `false`; `MonitorService` adds the `microphone` foreground-service type only while a talk runs; `set_playback_ducked` mutes the parent's playout of the baby. Verify: JVM tests with a fake engine (no track outside a talk, permission refused → no request), manual check of the system mic indicator.
- [ ] 2.3 Baby playout: talk transceiver `RECV_ONLY`; remote talk track disabled and ADM speaker muted except between `set_talk_playback { true }` and `{ false }`; chime generator (two soft tones, envelope, ~600 ms) reporting `talk_chime_done`; alarm-stream level set and restored (restore also at the next start after a crash); 800 ms fade-in and a peak limiter on the remote gain. Verify: JVM tests for the gating (fake port: playback enabled only inside a granted talk), restore-after-crash test.

## 3. Android UI and copy (`apps/android/app`, `ui-contract`, `locales`)

- [ ] 3.1 Contract: talk state (`idle`, `requesting`, `talking { untilMs }`, `denied { reason }`, `unavailable { reason }`), `noisePaused`, baby `parentTalking`, baby talk settings in `UiContract.kt`; models in `ParentModel`/`BabyModel` from core events only. Verify: JVM tests of the mapping.
- [ ] 3.2 Parent talk button per design §9 (hold only, haptic on grant, countdown, denial snackbar, unavailable reason, noise-paused line), tags `parent_talk`, `parent_talk_state`, `parent_talk_unavailable_reason`, `parent_noise_paused`. Verify: Robolectric tests; Roborazzi baselines recorded in the pinned image (report `SKIPPED` without it).
- [ ] 3.3 Baby: dim kiosk indicator `baby_talking` without waking the screen, touch-wake status line, settings rows `baby_talk_toggle`, `baby_talk_volume`, setup checklist line. Verify: Robolectric tests.
- [ ] 3.4 Copy: all new keys in `locales/en.json` and the other locales with `machineTranslated` markers; `critical` on denial reasons and the noise-paused lines. Verify: `just gen`, `just lint-text`, literal lint.

## 4. Web (`apps/web`, `apps/e2e`) — per the owner's web decision

- [ ] 4.1 Attempt: baby adds the talk transceiver `recvonly` and never plays it; parent sets the talk transceiver `sendonly`, `getUserMedia` (processing on) on `set_talk_mic { true }`, `replaceTrack(null)` and `track.stop()` on `false`; baby status `talk: unsupported` from the web baby. Verify: Vitest with fake media (no track outside a talk; tracks stopped on release; permission refused path).
- [ ] 4.2 Parent UI: talk button and states with the shared ids; mirrors the Android contract. Verify: Vitest, `svelte-check --fail-on-warnings`, Playwright spec (fake media device) pressing `parent_talk` against a web baby shows the unsupported reason.

## 5. Gate (`scripts/poc`)

- [ ] 5.1 Gate scenario: parent holds talk for 5 s; assert the baby receives talk RTP only inside the grant, no noise alert reaches the parent during and 5 s after the talk at high volume, the alarm-stream index is restored, and no talk RTP is sent outside a talk. Update `docs/ci-gates.md`. Verify: `just gate` (report `SKIPPED` without KVM).

## 6. Docs and guardrails

- [ ] 6.1 AGENTS.md §9: replace the Talk-back row (built by this change; constraints: half duplex, trusted link, baby-side limits). `docs/backlog.md`: delete the `DEFERRED` item; add `IDEA`s for what stays out (web baby playback if not built, full duplex, talking to several baby units). Verify: `just lint-text`.
- [ ] 6.2 `quiet-link-no-rtp/design.md` §4: correct the echo-cancellation reason (half duplex instead of "talk-back is deferred"). Verify: `openspec validate --all --strict`.

## 7. Checks

- [ ] 7.1 refactor-scout cleanup pass in diff mode against `origin/main`; accepted findings implemented. Verify: re-run affected tests.
- [ ] 7.2 `just lint`, `just test`, `just verify-android`. Verify: all green; SKIPPED tasks reported.

## Workflow follow-up

- Archive after `quiet-link-no-rtp` (both modify "Audio only on demand"; rebase the MODIFIED block on its text at archive time).
