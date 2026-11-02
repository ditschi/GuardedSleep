# Tasks

Each task is a brief for the `implementer` sub-agent. Work in one branch off `origin/main` and commit after each task (Conventional subject, for example `chore(tooling): …`). Layer rules: tooling only. Do not touch `crates/*/src`, `apps/web/src` or the app's Kotlin sources. Generated files stay uncommitted. Every version-specific setting keeps its doc-link comment. Weight: medium (builds); run checks through `just`, with `GS_RUN_TIMEOUT` (AGENTS.md §10.2).

## 1. Swift smoke removal (D3)

- [x] 1.1 Delete `.github/workflows/swift-bindings.yml`. In `scripts/core-build.sh`, remove `run_swift`, the `swift` case and its usage line. Keep `just check-ios` unchanged. In `docs/backlog.md`, drop the weekly bindings smoke from the iOS row. Acceptance: `git grep -n 'swift-bindings\|core-build.sh swift\|run_swift\|bindings smoke' -- ':!openspec/'` finds nothing; `just lint-workflows` is green.

## 2. Orchestration slimming (D4)

- [x] 2.1 Files: `scripts/lib/slot.sh`, `justfile`, `scripts/lib/reap.sh`; delete `scripts/lib/timings.sh` and `scripts/lib/machine-profile.sh`.
  - `slot.sh`: remove `record_timing`, `t_start`, `t_acquired`, `nesting` and the timings header comment. Replace the profile sourcing with `GS_MEDIUM_SLOTS="${GS_MEDIUM_SLOTS:-2}"` and `GS_HEAVY_SLOTS="${GS_HEAVY_SLOTS:-1}"`, validated as positive integers (otherwise exit 2 with a message). Keep the `GS_SLOT_WAIT`/exit 75 and `GS_RUN_TIMEOUT`/exit 124 behaviour and the re-entrancy rules unchanged.
  - `justfile`: remove the `machine-profile` and `timings` recipes and the `machine-profile.sh` call in `setup`.
  - `reap.sh`: remove `--prune-worktrees` and the prune branch; keep `--worktrees` listing and `--stop`; update the usage line and the header comment.

  Acceptance:
  - `GS_MEDIUM_SLOTS=1 scripts/lib/slot.sh medium -- true` exits 0;
  - `GS_MEDIUM_SLOTS=x scripts/lib/slot.sh medium -- true` exits 2;
  - `GS_RUN_TIMEOUT=1 scripts/lib/slot.sh medium -- sleep 5` exits 124;
  - `just reap --worktrees` lists, `just reap --prune-worktrees` prints usage and exits 2;
  - `git grep -n 'timings\.sh\|timings\.tsv\|machine-profile\|prune-worktrees' -- ':!openspec/' ':!docs/handoff-wave-3.md' ':!AGENTS.md' ':!docs/contributing/'` finds nothing (AGENTS.md and the docs follow in tasks 5 and 6).

## 3. One Rust execution mode (D5)

- [ ] 3.1 Files: `scripts/core-build.sh`, `scripts/gradle.sh`, `scripts/lib/build-image.sh`, `scripts/lib/require.sh` (comments and the `android-sdk` check only if needed), `apps/android/core-bindings/build.gradle.kts`, `apps/android/platform/build.gradle.kts`, `.devcontainer/Dockerfile` (comment).
  - **Host detection:** `rustup` on `PATH` (not just `cargo`); the same check in `scripts/cargo.sh` and `require.sh`.
  - **Mode selection** in `core-build.sh`, decided per subcommand:
    - `test` and `wasm` run natively with host rustup, otherwise in the image with `target/` (like `cargo.sh`, whose fallback keeps `target/`; say why in its header);
    - `host`, `android` and `determinism` run in the image unless `CORE_BUILD_NATIVE=1`.

    A mixed invocation (`core-build.sh test android`) runs each subcommand in its own mode. `gradle.sh` keeps its current mode rule.

  - **Image runs of `host`, `android` and `determinism`** set `CARGO_TARGET_DIR=/workspace/target/container`, and `gradle.sh` passes the same value into its container. In `core-build.sh` only cargo outputs (`debug/…`, `wasm32-unknown-unknown/release/…`) become `${CARGO_TARGET_DIR:-target}/…`. `target/wasm-pkg` and `target/determinism` stay fixed: `packages/core-wasm/scripts/copy-pkg.mjs`, `crates/guardedsleep-core/js/conformance.test.mjs` and `scripts/lib/api-snapshot.sh` read them.
  - **Gradle library path:** both `build.gradle.kts` files take `jna.library.path` from `System.getenv("CARGO_TARGET_DIR")`, defaulting to `<repo>/target`, plus `/debug`. Keep the existing comment and update it.
  - **Native runs:** keep `CARGO_HOME` unless `CARGO_HOME_OVERRIDE` is set. Apply the same rule to the `CARGO_HOME` default in `apps/android/build-logic` (`GenerateI18nTask.kt`, `GeneratePublicListsTask.kt`, `ShippedLocalesTask.kt`).
  - **wasm-bindgen:** use it from `PATH` when its version matches the pin (CI caches it in `~/.cargo/bin`). Only on a miss, install it with `cargo install --locked --root target/tools` and put `target/tools/bin` first on `PATH`.
  - **No cache:** `determinism` keeps running with `RUSTC_WRAPPER` empty.
  - **Comments:** update the header comments, and the comment in `build-image.sh` that says host cargo shares `target/`.

  Acceptance, on a machine with host rustup and Docker:
  1. `just test` is green.
  2. `just build-android` is green.
  3. A second `just test` shows no `Compiling guardedsleep-` or `Compiling i18n-gen` line at all (host and image steps).
  4. `ls target/container/debug/libguardedsleep_ffi.so` exists after step 2.
  5. With `CORE_BUILD_NATIVE=1` (the CI path), `scripts/core-build.sh host` writes `target/debug/libguardedsleep_ffi.so` as before, and `scripts/core-build.sh wasm` writes `target/wasm-pkg/{node,web}`.
  6. `just api-check` and `just check-determinism` are green.

  If Docker or the SDK is missing, report it as SKIPPED, not as a pass.

## 4. Gate register (D2)

- [ ] 4.1 Rewrite `docs/ci-gates.md`.
  - **Keep** the budget rule (12 h/month; six months without a catch → remove or demote), with "The maintainer owns every gate during alpha" stated once.
  - **Table:** columns Gate | Protects | Runbook | Last catch. One row each for `ci-ok` (mentions the `changes` path filter), `pr-title`, `text-lint`, `lint`, `test`, `build`, `android`, `e2e`, emulator gate (`android-native.yml`), images (`docker-signaling.yml` + `docker-web.yml`), `audit` and the relay check (`relays-weekly.yml`).
    - Fold every former per-check row into the Protects cell of its job, and its runbook into the job's Runbook cell, without losing a check name. Use `grep -c` before and after to check the names.
    - The `e2e` row carries `2026-10-24 (text-overflow check)`.
  - **Below the table:** a short "Other safeguards" list for Renovate and the libwebrtc CVE watch (keep the `docs/backlog.md` log link).
  - **Notes:** keep the sans-IO and API-snapshot notes. Move the sccache note to `development.md`. Drop the lefthook row: the hooks are described in `development.md`.

  Acceptance: `just lint-text` is green; at most 14 table rows; no Owner column; every workflow file name and every job of `ci.yml` except `changes` appears in the table.

## 5. Contributor docs (D1, D4, D5)

- [ ] 5.1 `docs/contributing/development.md` becomes the home of the mechanisms the spec no longer names. Sections:
  - **Hooks:** keep the Husky repair text.
  - **Load slots:** defaults 2 and 1 (set `GS_MEDIUM_SLOTS=1` below 8 cores or 12 GB); the `GS_MEDIUM_SLOTS`, `GS_HEAVY_SLOTS`, `GS_SLOT_WAIT` and `GS_RUN_TIMEOUT` variables; exit 75 vs. 124 vs. the test verdict; re-entrancy in one sentence.
  - **Delta tests:** drop the timings paragraph and the prune sentence; keep the `just reap --worktrees` listing and `git worktree remove` as the removal step.
  - **Rust execution mode** (new section): what runs where, `target/` vs. `target/container`, `CORE_BUILD_NATIVE=1` for CI and the devcontainer, the glibc reason for the Android chain.
  - **Compiler cache:** now covers container builds only; includes the moved sccache note.

  Acceptance: `just lint-text` is green; `git grep -n 'exit 75\|exit 124\|status 75' -- ':!openspec/' ':!AGENTS.md'` shows the numbers only in `development.md` and `slot.sh`.

## 6. AGENTS.md (§3, §5.2, §8, §10.1-10.3)

- [ ] 6.1 Update AGENTS.md.
  - **§4:** the command table row "Delta tests, timings" becomes "Delta tests" without `just timings`.
  - **§3:** the `docs/` line shows `docs/ci-gates.md` (one row per CI job, runbooks, last-catch dates).
  - **§5.2:** the tooling budget names one row per job.
  - **§8:**
    - branch protection is configured: required `ci-ok`, linear history, merge commits disabled;
    - drop "not enforced while the repository is private";
    - agents still check `ci-ok` before they ask to merge;
    - rebase and merge for a clean Conventional history, otherwise squash.
  - **§9:** the iOS row drops "and a weekly bindings smoke test".
  - **§10.1:**
    - drop `just machine-profile` and the profile file;
    - state the defaults (`GS_MEDIUM_SLOTS=2`, `GS_HEAVY_SLOTS=1`) and the environment overrides;
    - drop the `just timings` mention;
    - shared caches: say that host cargo builds into `target/` and container builds into `target/container`.
  - **§10.2:** the slot-deadline bullet points to `development.md` for the exit statuses instead of naming them; `just reap` lists leftovers, `just reap --worktrees` lists finished worktrees; remove `--prune-worktrees`.
  - **§10.3:** remove `docs/ci-gates.md` from the merge hotspots.
  - Keep the file under about 300 lines.

  Acceptance: `just lint-text` is green; `git grep -n 'machine-profile\|prune-worktrees\|just timings\|bindings smoke' AGENTS.md` finds nothing.

## 7. GitHub settings (owner, manual)

- [x] 7.1 (done by the planner via the API: ruleset 26403725, merge commits disabled, 2026-11-02) Owner: in the repository settings, turn off "Allow merge commits" and keep "Allow squash merging" and "Allow rebase merging". Add a ruleset for `main`: require status check `ci-ok`, require linear history, block force pushes and deletions; no required reviews, no signed commits. Verify with `gh api repos/ditschi/GuardedSleep/rulesets` and `gh repo view --json mergeCommitAllowed`.

## 8. Verification

- [ ] 8.1 Planner: run `refactor-scout` in diff mode against `origin/main`, hand accepted findings to the `implementer`, and re-run the checks.
- [ ] 8.2 Run `just lint` and `just test` (both green). Run `just build-android` once for task 3. Report every SKIPPED. Then run `just reap` and confirm no leftovers.
