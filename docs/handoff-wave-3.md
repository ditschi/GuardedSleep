# Handoff: wave 3 on the OpenSpec workflow

`STATUS: UNDER DISCUSSION`. This is a temporary handoff note written 2026-10-22 at the end of wave 2. Delete it once wave 3 has started and its content lives in changes, the backlog or AGENTS.md.

## 1. Start here

1. **Branch.** Use `chore/toolchain-wave-2`. Waves 1 and 2 are merged on it; nothing is pushed and no PRs are open. Earlier branches build on each other: `chore/agent-model-routing` → `docs/toolchain-realignment` → `chore/toolchain-wave-1` → `docs/openspec-adoption` → `chore/toolchain-wave-2`. Create `chore/toolchain-wave-3` from the tip of wave 2.
2. **Setup** (once per checkout): `just setup`. It installs the mise tools, writes the machine profile, and replaces the stale Husky `core.hooksPath` with lefthook. Until this runs, git hooks are off in the main checkout.
3. **Read.** AGENTS.md is binding. §6 covers OpenSpec, §7 the guardrails, §10.1 machine load and parallel agents, §10.2 commands that must finish, and §10.3 agent worktrees, interruptions and hooks. Then read `openspec/specs/`, the active changes in `openspec/changes/`, and `docs/backlog.md`.
4. **OpenSpec.** Call it only via `mise exec -- openspec …` or `just`, because telemetry and update checks are disabled there. The workflow per change:
   1. Refine the coarse `tasks.md` against the current code.
   2. Delegate implementation to the `implementer` sub-agent, with the tasks section as the brief.
   3. Run role reviews for the lens the change touches.
   4. Fix the findings.
   5. Run the Critical Reviewer gate. It is mandatory for architecture, security-model, scope or core-concept changes.
   6. Run `just ci` with `GS_STRICT=1` on the integrated branch.
   7. Archive with `mise exec -- openspec archive <change> --yes`.
   8. Re-run `openspec validate --all --strict`. Merged requirements can exceed the 500-character warning; split them.

## 2. Wave 2 outcome

All four changes were archived on 2026-10-22 after role reviews, critical gates and a green `just ci` (`GS_STRICT=1`):

- **`add-core-facades`**
  - Binding-free core with `guardedsleep-ffi` (UniFFI) and `guardedsleep-wasm` (wasm-bindgen + tsify).
  - API snapshots, a generate-twice determinism check, vectors and trace replay through every binding, and a weekly Swift smoke run.
- **`add-rust-signaling-server`**
  - Sans-IO `guardedsleep-relay` and an axum `guardedsleep-server`. The Node server and `packages/protocol` are deleted.
  - Relay admission: `Required` on LAN relays, `Open` on the public server.
  - Abuse limits: 16 connections per IP, frame budgets 120 per minute before joining and 600 per minute after.
  - Built-in STUN with bounded amplification: ≤ 44 B and ≤ 2.2× the request, per-IP rate limit, on by default. No Google STUN.
  - Hardened distroless image.
  - Android uses the relay's STUN by default.
- **`swap-webrtc-sdk`**
  - `io.github.webrtc-sdk:android` 150.7871.01 behind `MediaEngine` (`media/libwebrtc`), guarded by a seam test.
  - Maven Central bound with `exclusiveContent`.
  - CVE-based patch policy with a monthly watch.
- **`split-android-modules`**
  - `build-logic`; modules `app`, `ui-contract`, `platform` and `core-bindings`.
  - ktfmt in `just fmt`; Lint warnings are errors, with no baseline.
  - R8 shrinks the release APK by 23%.
  - versionCode derived from versionName: `0.1.0-alpha.1` gives 100101.
  - Permission allow-list in `release-check.sh`; `BindingSeamTest`.

Tooling added during wave 2:

- `just machine-profile`, `scripts/lib/slot.sh` (medium and heavy slots), `just reap`, and shared Cargo and Gradle caches.
- A lint that rejects Google STUN hosts.

## 3. Wave 3: active changes and order

| Change                   | Tier      | Depends on                            | Notes                                                                                                                                                                  |
| ------------------------ | --------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rebuild-web-client`     | T0        | (facades done)                        | New `apps/web` on Vite 8 and Svelte 5, no SvelteKit. Task 1.6: relay STUN by default, drop the sipgate/linphone defaults. Then delete the legacy mobile client (done). |
| `expand-locales`         | T0        | —                                     | About 20 AI-translated locales, `critical` keys, `machineTranslated`/`reviewed` markers. The owner reviews German.                                                     |
| `add-quality-gates`      | T0        | ideally after `rebuild-web-client`    | Tokens with a contrast test, Roborazzi a11y goldens, log canary, 2 fuzz targets, size budgets, `secrecy`/`zeroize`, `SECURITY.md`.                                     |
| `add-android-release`    | T0        | versionCode (done)                    | Manual `android-v*` tag release. Task 1.6 is the release smoke of the R8 APK on the emulator before the first release APK; the R8 build has never run on a device.     |
| `add-stun-server-choice` | T0/T1     | `rebuild-web-client` for the web part | Vetted EU STUN list plus manual entry, with a warning. Research the candidates first (task 1.1).                                                                       |
| `add-offline-lan`        | beta (T1) | `rebuild-web-client`                  | Android listener and local PC mode. The local-mode code already exists in the server (`--serve-web`, `--local`); this change specifies and reviews it.                 |

Suggested grouping on this machine (2 medium slots, 1 heavy slot):

- **First pair:** `rebuild-web-client` and `expand-locales`. They use different files; i18n generation is shared, so coordinate on `locales/`.
- **Then:** `add-quality-gates` and `add-android-release`.
- **Later:** `add-stun-server-choice`, then `add-offline-lan` at beta.

## 4. Deadlines and owner to-dos

- **2026-11-19: libwebrtc CVE decision.** Chrome 156 fixed a critical WebRTC bug (CVE-2026-113303) that webrtc-sdk 150.7871.01 predates. If no patched vendor build exists 30 days after the Chrome fix, propose a change: switch vendor, patch-build, or disable the feature. Seven High WebRTC fixes from M152 to M155 are also missing; that does not trigger the policy. See `docs/backlog.md`, "libwebrtc CVE decision". The monthly watch is in `docs/ci-gates.md`.
- **Owner:**
  - Run `just setup`.
  - Play developer account: organisation account with D-U-N-S if easy, and start developer verification.
  - Generate the offline key for the direct APK distribution.
  - Provide a brand launcher icon (`ic_launcher.xml` is a placeholder).

## 5. Working rules learned (also in AGENTS.md §10.1–§10.3)

- **Machine load.**
  - Load comes from what agents run, not from how many there are.
  - Light agents (reviews, docs, OpenSpec artifacts): about 6 in parallel.
  - Agents that build or test: at most `GS_MEDIUM_SLOTS` at a time.
  - Heavy phases (gate, e2e, image builds) queue on the heavy slot automatically.
- **Timeouts and slots.** A wrapping `timeout` also counts the time spent waiting for a slot. Give heavy runs a generous timeout (2400–3600 s) or rerun after a slot wait. Backlog item: `GS_RUN_TIMEOUT` in `slot.sh`.
- **Commands must finish.** Wrap long commands in `timeout`. No polling loops and no `pgrep -f` waits: these hung shells for 3+ hours in this session. Nothing may be left running; `just reap` finds leftovers.
- **Agent worktrees** start from an old base. Always `git rebase chore/toolchain-wave-3` first, and have agents commit after each step.
- **Usage limits** cut agents off mid-run. Commits survive. Check `git -C <worktree> status`, then resume the agent with SendMessage.
- **Merge conflicts** recur in `docs/ci-gates.md` (one big table), `justfile` and AGENTS.md. Resolve them row by row and keep both sides.
- **Full integrated `just ci`** takes about 15 minutes warm (`GS_STRICT=1`). A cold build in a fresh worktree adds several minutes.

## 6. Open follow-ups (in the backlog)

- **Pre-alpha security audit** candidates: PSK redaction in `Debug`/`toString`, constant-time compare and zeroize of the admission key, `DeterministicEntropy` made test-only.
- **Invite codec** moved from Kotlin (`PairingPayload.kt`) into the core.
- **Tooling:** sccache for agent worktrees, `GS_RUN_TIMEOUT`, and the refactor-scout items that still need a decision.
