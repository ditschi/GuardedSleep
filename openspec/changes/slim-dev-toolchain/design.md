# Design: slim the dev toolchain

Roles applied: Coordinator, Technical Architect (this document), Critical Reviewer (findings and answers at the end). Tier T0, tooling only.

## Context

Verified on `origin/main` 501f95bd (2026-11-02):

- `openspec/specs/dev-toolchain/spec.md` has **33** requirements (the request said 32). Lines 36-48 (Git hooks, with the Husky scenario) and 267-325 (Load slots with exit codes 75/124, Local compiler cache, Delta tests, Run timings with a TSV column layout, Worktree cleanup) describe mechanisms rather than behaviour.
- `docs/ci-gates.md` has **30** gate rows. One has a last-catch date (text-overflow check, 2026-10-24). The Owner column says `owner` in 29 rows and `project owner` in 1. Seven workflows exist: `ci.yml` (jobs `changes`, `pr-title`, `text-lint`, `lint`, `test`, `build`, `android`, `e2e`, `ci-ok`), `android-native.yml`, `audit.yml`, `docker-signaling.yml`, `docker-web.yml`, `relays-weekly.yml` and `swift-bindings.yml`.
- `swift-bindings.yml` runs `scripts/core-build.sh swift` weekly. iOS is `DEFERRED` (AGENTS.md §9). `just check-ios` (`cargo check … --target aarch64-apple-ios`) runs inside `just lint-code`.
- `scripts/lib/slot.sh` (130 lines) records timings through an inline `record_timing`. `scripts/lib/timings.sh` summarises them. `scripts/lib/machine-profile.sh` writes `.cache/machine-profile.env`; only `slot.sh` reads it, and only for `GS_MEDIUM_SLOTS` and `GS_HEAVY_SLOTS`. On the maintainer's machine (8 cores, 15 GB) it yields 2 medium and 1 heavy. `scripts/lib/reap.sh` implements `--worktrees` and `--prune-worktrees`.
- `scripts/cargo.sh` uses host cargo first and the build image as a fallback. `scripts/core-build.sh` and `scripts/gradle.sh` default to the build image unless `CORE_BUILD_NATIVE=1`; CI and the devcontainer set it. The image mounts the repository at `/workspace`, writes `target/`, and in native mode uses its own `CARGO_HOME` under `target/.cargo-home`. Host and container builds therefore share `target/` but differ in absolute paths, `CARGO_HOME` and `RUSTC_WRAPPER` (sccache), so each mode invalidates the other's fingerprints. The image is Ubuntu jammy (glibc 2.35); the host has glibc 2.43. The JVM tests load `target/debug/libguardedsleep_ffi.so` (`jna.library.path` in `apps/android/{core-bindings,platform}/build.gradle.kts`), so a host-built library cannot run under a container Gradle.
- The repository is **public** since 2026-11-02. `main` has no branch protection and no ruleset, and merge commits are still allowed in the repository settings (`gh repo view`, `gh api …/branches/main/protection` → 404).

## Goals / Non-Goals

**Goals:** the spec states invariants a tooling tweak does not touch; the gate register is small and lists real jobs; less script code to maintain; one predictable place for Rust build output; branch protection matches the documented merge rule.

**Non-goals:** new gates; changes to the content of CI jobs (apart from deleting the Swift workflow); sccache in CI (T2); deleting `docs/handoff-wave-3.md` (separate cleanup).

## Decisions

### D1. What stays in the spec

- **Option A: invariants in the spec, mechanisms in `docs/contributing/development.md`.** Keep requirements on observable contributor behaviour (one entry point, SKIPPED/`GS_STRICT`, pins, hooks exist and lint warns, `ci-ok` and protection, supply chain, register and budget, slot queueing with a deadline that excludes the queue, delta tests share CI's path map). Drop exit codes, file formats, variable defaults and cache layout.
  - Pros: tweaks (an exit code, a default, a cache path) need no change cycle; the spec still catches regressions that matter.
  - Cons: the docs can drift from the scripts without a validator; a reviewer has to know to look there.
- **Option B: move the whole toolchain spec into docs** and keep only a one-line pointer.
  - Pros: zero spec maintenance.
  - Cons: loses binding rules that protect the product (sans-IO bans, binding-free core, vectors through every binding, libwebrtc policy); AGENTS.md treats specs as the source of truth.

**Recommendation: A.** Requirements that protect product properties stay as they are. Only the eight listed requirements change.

### D2. Gate register shape

- **Option A: one row per CI job or scheduled workflow.** Columns: Gate, Protects, Runbook, Last catch. Checks inside a job are named in the Protects cell. Rows: `ci-ok`, `pr-title`, `text-lint`, `lint`, `test`, `build`, `android`, `e2e`, emulator gate (`android-native.yml`), images (`docker-signaling.yml` + `docker-web.yml`), `audit`, relay check (`relays-weekly.yml`); two non-CI safeguards, Renovate and the libwebrtc CVE watch, go in a short list below the table. The `changes` path-filter job is mentioned under `ci-ok`. lefthook is not a gate and moves to the docs.
  - Pros: about 12 rows match what a red CI run shows; a new check inside a job edits a cell instead of adding a row, so parallel branches conflict less.
  - Cons: the last-catch date becomes coarser (per job, not per check). The 2026-10-24 date moves to the `e2e` row with the check named.
- **Option B: keep per-check rows, drop only the Owner column.**
  - Pros: precise catch dates.
  - Cons: still 30 rows and still the merge hotspot; per-check dates were never filled in (1 of 30).

**Recommendation: A.** The six-month rule then acts on jobs. A check inside a job that is suspected dead is handled in the job's runbook cell.

### D3. Swift bindings smoke

- **Option A: delete the workflow and the `swift` subcommand; keep `just check-ios`.**
  - Pros: one weekly job and about 15 lines of script fewer; `cargo check` still keeps the core and ffi compiling for iOS, which is the property that is expensive to recover later.
  - Cons: a UniFFI change that breaks Swift generation in particular goes unnoticed until the iOS gate. That is acceptable: UniFFI is pinned exactly and updated in its own Renovate PR.
- **Option B: keep the job on `workflow_dispatch` only.**
  - Pros: can be run by hand before a UniFFI bump.
  - Cons: still code and a register row to maintain; unscheduled jobs rot.

**Recommendation: A.** The iOS gate change (Phase 2) brings generation back together with a real client.

### D4. Agent orchestration

- **Option A: keep the `flock` semaphore and the `just reap` listing; drop timings, machine-profile detection and worktree pruning.** Slot counts become `GS_MEDIUM_SLOTS` (default 2) and `GS_HEAVY_SLOTS` (default 1), read from the environment in `slot.sh`. The defaults equal the measured profile of the maintainer's machine and are safe on a 16 GB laptop. Bigger machines set the variables in their shell profile; machines below 8 cores or 12 GB set `GS_MEDIUM_SLOTS=1` (the old formula's result there), documented in `development.md`. `GS_SLOT_WAIT`, `GS_RUN_TIMEOUT` and their exit statuses (75, 124) stay, documented in `development.md`. `just setup` stops running `machine-profile.sh`. A leftover `.cache/machine-profile.env` is ignored (git-ignored, harmless).
  - Pros: removes `timings.sh` (38 lines), `machine-profile.sh` (55 lines), the recording block in `slot.sh` and the prune branch in `reap.sh`; the parts that prevent overload and leftovers stay.
  - Cons: no data on where time goes. `just timings` was never used for a decision; CI job durations in the Actions UI cover the main need.
- **Option B: keep everything and only move it out of the spec.**
  - Pros: no code change.
  - Cons: the maintenance cost stays; the owner asked to slim.

**Recommendation: A.**

### D5. One Rust execution mode

- **Option A: host by default; the image for the Android chain, the determinism check and as a fallback; separate container target directory.**
  - **Host detection** means `rustup` on `PATH` (a distro cargo would ignore `rust-toolchain.toml`).
  - `scripts/cargo.sh`: host first, image fallback as today. The fallback keeps writing `target/`: without host rustup there is no host build to protect, and `scripts/dev.sh`, `scripts/poc/env-up.sh` and the web dev server read `target/debug`.
  - `scripts/core-build.sh test|wasm`: natively with host rustup; otherwise in the image with `target/`, like `cargo.sh`.
  - `scripts/core-build.sh host|android` and `scripts/gradle.sh` (the **Android chain**): in the image unless `CORE_BUILD_NATIVE=1` (CI, devcontainer). The JVM tests load the core library built in the same environment, which avoids the glibc mismatch.
  - `scripts/core-build.sh determinism`: in the image locally (a reproducible environment for the generate-twice check, without sccache); CI keeps running it natively.
  - Image runs of the Android chain and determinism set `CARGO_TARGET_DIR=/workspace/target/container`. In `core-build.sh` only cargo outputs (`debug/`, `wasm32-unknown-unknown/release/`) follow `${CARGO_TARGET_DIR:-target}`; tool outputs read by other files (`target/wasm-pkg`, read by `packages/core-wasm/scripts/copy-pkg.mjs`, the conformance test and `api-snapshot.sh`; `target/determinism`) stay at fixed `$ROOT/target/…`. `jna.library.path` in both `build.gradle.kts` files reads `CARGO_TARGET_DIR` (default `<repo>/target`), and `gradle.sh` passes it into the container. The host keeps `target/` with its normal `CARGO_HOME`. Native runs stop overriding `CARGO_HOME` unless `CARGO_HOME_OVERRIDE` is set (`scripts/lib/build-image.sh` sets it for image runs); the build-logic Gradle tasks that default `CARGO_HOME` to `target/.cargo-home` follow the same rule. `wasm-bindgen` is taken from `PATH` when its version matches the pin (CI caches it in `~/.cargo/bin`); only on a miss is it installed with `cargo install --root target/tools`, and `target/tools/bin` is put first on `PATH`, so a contributor's global copy is never overwritten. The determinism check and release builds run with `RUSTC_WRAPPER` empty (spec: no compiler cache).
  - Pros: host `cargo test`, clippy and wasm builds stay warm across `just build-android`; the Android chain runs in one consistent environment; CI is unchanged (`CORE_BUILD_NATIVE=1`).
  - Cons: the Android chain compiles the core twice on a cold worktree (host target and container target); `target/container` costs disk. sccache in the image softens the first cost.
- **Option B: host for everything, including the Android chain** (needs NDK, SDK and `cargo-ndk` on the host).
  - Pros: one build tree, fastest incremental builds.
  - Cons: contributors must install the Android SDK and NDK at the pinned version; the build image stops being the reproducible reference for Android; contradicts the owner's "keep Docker for Android ABIs".
- **Option C: the image for everything** (as `core-build.sh` does today, and `cargo.sh` changed to match).
  - Pros: fully reproducible.
  - Cons: slow cold starts per worktree; host-installed rust-analyzer still builds in `target/` and fights the container; contributors with host rustup gain nothing.

**Recommendation: A.** The spec states only the property (host default; the image for the Android chain and determinism; no shared output directory; no cache for determinism and release). Paths and variables live in `development.md`.

### D6. Merge rule and branch protection

- **Option A: the spec requires `ci-ok` and linear history on `main`, and states the merge rule.** Land by rebase and merge when every commit is Conventional and green on its own; otherwise squash and merge with the PR title as subject; never a merge commit. The owner configures a branch ruleset (required check `ci-ok`, require linear history, block force pushes and deletion) and turns off "Allow merge commits" in the repository settings. AGENTS.md §8 drops "not enforced while private" and the "agents check `ci-ok` themselves" workaround, but agents still read `ci-ok` before they ask to merge.
  - Pros: the rule that AGENTS.md already states becomes enforced; GitHub refuses the merge-commit button.
  - Cons: a manual settings step that no lint can verify; `ci-ok` must report on every PR (it does: `if: always()`).
- **Option B: squash only.**
  - Pros: simplest rule; the PR title check covers the subject.
  - Cons: throws away a curated history the owner explicitly allows (AGENTS.md §8).

**Recommendation: A.** No required reviews or signed commits (ADR-020 §9 unchanged).

## Privacy guardrails and layers

None of this touches the product layers (UI, State, Platform, Bindings, Core) or the privacy guardrails of AGENTS.md §7. Supply-chain checks (cargo-deny, the dependency-count guard, `pnpm audit`, SHA-pinned actions, `permissions: {}`) are unchanged. Enabling branch protection tightens the supply-chain posture. Removing the Swift job removes one workflow that had `contents: read`; no permission is added anywhere.

## Risks / Trade-offs

- **Docs drift from scripts** → `development.md` is the only home for the mechanisms; each script's header comment links its section. The refactor-scout cleanup pass checks both.
- **Container target directory doubles disk use per worktree** → `target/container` is under `target/`, so `cargo clean` and `just reap` guidance cover it.
- **Hard-coded `target/debug` paths missed** → the implementer greps for `target/` in `scripts/`, `justfile` and `apps/android/**/build.gradle.kts`; acceptance runs `just test`, `just build-android`, `just test` and checks that the last run compiles no workspace crate.
- **Owner forgets the GitHub setting** → a task in `tasks.md` and a line in the PR description; the spec scenario fails visibly if anyone tries a merge commit.

## Migration Plan

1. Merge this change (docs PR) after owner approval.
2. Implement through the `implementer` agent (tasks 1-6), one commit per task, then `just lint`, `just test`, `just build-android`.
3. Owner applies the GitHub settings (task 7).
4. Archive the change: the deltas merge into `openspec/specs/dev-toolchain/spec.md`.

Rollback: revert the implementation commits; the removed scripts come back from git history. The GitHub ruleset can be disabled in one click.

## Open Questions

None blocking. The owner may later drop sccache once the image only builds the Android chain; the decision rests on measured cold-build times and is out of scope here.

## Discarded alternatives

- **D1 B (whole toolchain spec into docs):** loses binding rules that protect product properties.
- **D2 B (per-check rows without the Owner column):** keeps the hotspot; dates were never maintained per check.
- **D3 B (Swift job on dispatch only):** unscheduled jobs rot and still cost a register row.
- **D4 B (keep timings, profile and prune, move them out of the spec):** keeps the maintenance cost the owner asked to cut.
- **D5 B (host for the Android chain):** needs a host SDK/NDK and gives up the reproducible reference image.
- **D5 C (the image for every Rust command):** slow, and fights host tooling over `target/`.
- **D5, sharing `target/` with a path remap** (`--remap-path-prefix`, the same mount path on host and container): fingerprints also depend on `CARGO_HOME`, `RUSTC_WRAPPER` and the glibc of build-script binaries, so the invalidation would remain partial and hard to debug.
- **D6 B (squash only):** discards the clean-history path that AGENTS.md §8 allows.

## Critical review

Independent Critical Reviewer pass (sub-agent, read-only, 2026-11-02): approved with concerns. Every IMPORTANT finding is answered in the artifacts.

| #   | Severity  | Finding                                                                                                        | Answer                                                                                                                                                             |
| --- | --------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | IMPORTANT | Moving every `target/` path under `CARGO_TARGET_DIR` would move `target/wasm-pkg` away from its three readers. | D5: only cargo outputs follow the variable; `wasm-pkg` and `determinism` stay fixed. The no-rustup fallback keeps `target/` anyway. Task 3.1 acceptance checks it. |
| 2   | IMPORTANT | Installing wasm-bindgen into `target/tools` would ignore CI's cached copy and rebuild it in four jobs.         | D5: `PATH` first when the version matches; install only on a miss.                                                                                                 |
| 3   | IMPORTANT | Removing "Local compiler cache" dropped the only binding text that keeps the determinism check able to fail.   | Moved into "Rust execution mode" with a scenario.                                                                                                                  |
| 4   | IMPORTANT | "Delta tests" silently dropped naming the heavy checks.                                                        | Restored, with the scenario clause.                                                                                                                                |
| 5   | IMPORTANT | "One row per job" contradicted the planned merged rows (docker, audit).                                        | Spec now says every workflow, and every job of the CI workflow; task 4 acceptance matches.                                                                         |
| 6   | IMPORTANT | Acceptance greps could not pass before archive (main spec, AGENTS.md §10.2).                                   | Greps exclude `openspec/`; task 6 covers §4 and §10.2.                                                                                                             |
| 7   | MINOR     | Missed references: `docs/backlog.md` iOS row, AGENTS.md §4 timings row, build-logic `CARGO_HOME` default.      | Added to tasks 1.1, 6.1 and 3.1.                                                                                                                                   |
| 8   | MINOR     | `cargo.sh` fallback vs. the new requirement was ambiguous.                                                     | The requirement scopes the separate directory to the Android chain and determinism; D5 states why the fallback keeps `target/`.                                    |
| 9   | MINOR     | `CARGO_HOME_OVERRIDE` is set by `build-image.sh`, not the image.                                               | Corrected.                                                                                                                                                         |
| 10  | MINOR     | Host detection by `command -v cargo` accepts a distro cargo.                                                   | Detection uses `rustup`.                                                                                                                                           |
| 11  | MINOR     | Fixed slot default 2 over-commits small machines.                                                              | Documented override for machines below 8 cores or 12 GB.                                                                                                           |
| 12  | MINOR     | Task 3 acceptance weaker than the spec scenario.                                                               | Aligned: no workspace crate recompiles.                                                                                                                            |
| 13  | MINOR     | The change revises ADR-020 decisions, so the Critical Review Gate is mandatory.                                | Proposal corrected.                                                                                                                                                |
| 14  | MINOR     | `ci-ok` also aggregates `pr-title`, `text-lint` and `android`; the rebase rule has no gate.                    | Spec says "every job of the CI workflow"; the rebase rule is a maintainer rule.                                                                                    |

Also from the planner's own review: the image's glibc (jammy, 2.35) is older than the host's (2.43). That is why D5 keeps the whole Android chain in one environment, which is wider than "Android ABIs only" in the request. OpenSpec does not let a MODIFIED block drop a scenario, so "Git hooks" is replaced by "Pre-commit hooks".
