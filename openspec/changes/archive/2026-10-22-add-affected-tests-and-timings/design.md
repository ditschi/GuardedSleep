# Design

## Context

Medium and heavy recipes already run through `scripts/lib/slot.sh` (AGENTS.md §10.1), which is the natural place to measure them. CI decides which jobs run with dorny/paths-filter in `ci.yml` (`code`, `android`); the emulator gate filters on `on.paths` in `android-native.yml`. Gradle already has the build and configuration caches on, with a Gradle home shared across worktrees; Rust compiles through the shared sccache since `add-worktree-sccache`. Vitest has `--changed <ref>` built in. AGENTS.md §10.1 already asks for "targeted checks per step, full checks once", but leaves the choice to each agent.

## Decisions

### 1. Timings

- [ ] **A. `slot.sh` appends a TSV line per slot-held run to `${GS_SHARED_CACHE:-~/.cache/guardedsleep}/timings.tsv` (recommended)**
  - Pros: zero effort for callers; covers every medium and heavy recipe in every worktree; shared across worktrees so data survives worktree removal; plain text, readable with `awk`.
  - Cons: light recipes (lint-text, docs) are not measured; they are not where time goes.
- [ ] **B. Wrap every recipe in the justfile**
  - Cons: touches every recipe; duplicates the slot entry point.

Recommendation: A. Columns: ISO time, worktree basename, class, command (the arguments after `--`, joined), wait seconds, run seconds, exit status, `nested` when `GS_SLOT_HELD` was already set. Written with one `printf >>` (atomic for lines under `PIPE_BUF`). Never fails the run: a write error is ignored. No PII beyond the worktree name (local file, never uploaded). `just timings [days]` (default 7) prints per command: count, median, p90 and max run time, total wait, failures.

### 2. Delta tests

- [ ] **A. `just test-affected [base]`: path map → test recipes, conservative fallbacks (recommended)**
  - Pros: one command for "targeted checks per step"; uses what tools already do well (Vitest `--changed`, Gradle up-to-date/build cache, sccache); fallbacks keep it safe.
  - Cons: a wrong map can hide a failing test until the full sweep; the map must be maintained (shared with CI, so a gap shows up in both places).
- [ ] **B. Rely on build-tool caching only (run `just test` every time)**
  - Pros: nothing to maintain.
  - Cons: Vitest, WASM conformance and Gradle configuration still run; no knowledge of which heavy checks a change needs.
- [ ] **C. Fine-grained test impact analysis (coverage maps per test)**
  - Cons: per-language tooling, far beyond the 12 h/month budget.

Recommendation: A.

Changed files: `git diff --name-only $(git merge-base <base> HEAD)` plus `git diff --name-only HEAD` plus untracked files (`git ls-files --others --exclude-standard`). Base default `origin/main`; if the ref does not exist, fall back to the full run.

Areas (`.github/path-filters.yml`, the single map) and what `test-affected` runs:

| Area      | Paths (summary)                                                                                                                                                                        | Runs                                                                                 |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `rust`    | `crates/**`, `Cargo.toml`, `Cargo.lock`                                                                                                                                                | `just test-rust`, `just test-wasm`; and the full `web` and `android` rows below      |
| `web`     | `apps/mobile/**`, `packages/**`, `locales/**`                                                                                                                                          | Vitest with `--changed <merge-base>` per package; full Vitest if `rust` or `locales` |
| `android` | the existing `android` filter of `ci.yml`                                                                                                                                              | `just test-android` (Gradle skips unchanged modules)                                 |
| `e2e`     | `apps/e2e/**`, `apps/mobile/**`, `crates/guardedsleep-{relay,server}/**`                                                                                                               | nothing; printed as "needs `just e2e` in the sweep"                                  |
| `gate`    | the `on.paths` list of `android-native.yml`                                                                                                                                            | nothing; printed as "needs `just gate` in the sweep"                                 |
| `global`  | `justfile`, `mise.toml`, `mise.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, `package.json`, `pnpm-workspace.yaml`, `scripts/lib/**`, `docker/android-native-build/**`, `.github/**` | full `just test`                                                                     |
| `docs`    | `docs/**`, `**/*.md`, `openspec/**`, `LICENSE`                                                                                                                                         | nothing (lint-text covers them)                                                      |

A changed path that matches no area falls back to the full `just test`. The run goes through one medium slot (like `just test`). Output starts with the list of areas and the reason for each fallback, so a reader can see why something ran.

`ci.yml` reads the `code` and `android` filters from the same file (`filters: .github/path-filters.yml`; dorny/paths-filter accepts a file path). The `gate` area mirrors `android-native.yml`'s `on.paths`, which cannot read a file; a Vitest test of `affected.mjs` asserts the two lists are equal, so drift fails `just test`.

Implementation: `scripts/lib/affected.mjs` (Node is pinned by mise; no new dependency; glob matching with `path.matchesGlob` if it runs without an experimental warning on the pinned Node, otherwise a small `**`/`*` matcher) with `node --test` unit tests; the recipe calls the existing `test-*` recipes.

### 3. Gradle across worktrees

Measure, do not change: in a fresh worktree, run `just test-android` with a warm shared cache and read Gradle's summary line (`N actionable tasks: X executed, Y from cache, Z up-to-date`). Record it in tasks.md. Only a poor hit rate (most compile tasks executed) leads to a follow-up change.

### 4. Worktree cleanup

- [ ] **A. Extend `reap.sh` with `--worktrees` (list) and `--prune-worktrees` (remove) (recommended)**
  - Pros: the existing leftovers command; same safety style (list first, act on request).
  - Cons: `reap.sh` grows.
- [ ] **B. Separate `just worktrees` recipe**
  - Cons: one more command to remember for the same "clean up after agents" job.

Recommendation: A. A worktree is removable only if all hold: no output from `git status --porcelain`; its HEAD is an ancestor of some `origin/*` ref (after `git fetch` is NOT run automatically; the listing says which ref contains it); it is not the main checkout; any lock names a process that no longer runs (a live lock is listed as "in use" and never touched). Pruning unlocks stale locks, runs `git worktree remove` (never `--force`), and deletes the branch with `git branch -d` only for `worktree-agent-*` branches. Ignored files (target/, generated bindings) go with the worktree; that is the point.

## Guardrails and layers

Tooling only. Timings stay on the machine. The path map changes which CI jobs run only by moving the existing filters into a file; the lists themselves are unchanged.

## Discarded alternatives

- **1B wrapping every recipe**, **2B caching only**, **2C coverage-based selection**, **4B separate recipe**: see above.
- **Playwright `--only-changed`** for app changes: it follows the test files' imports, which never include the app under test, so an app-only change would select no E2E test. E2E stays in the full sweep.
- **Long-lived Gradle daemon container**: conflicts with "leave nothing running"; revisit only if timings show Gradle start-up dominates.
