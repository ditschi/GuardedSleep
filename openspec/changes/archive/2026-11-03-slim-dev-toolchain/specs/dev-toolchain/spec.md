## ADDED Requirements

### Requirement: Rust execution mode

Rust commands SHALL run on the host rustup toolchain from `rust-toolchain.toml` by default, and in the pinned build image only for the Android chain (ABI libraries, the core library the JVM tests load, Gradle), the local determinism check and machines without host rustup. Those image builds SHALL NOT write into the host build output directory. The determinism check and release builds SHALL NOT use a compiler cache.

#### Scenario: Switching between a host test and an ABI build

- **WHEN** a contributor runs `just test`, then `just build-android`, then `just test` again without changing sources
- **THEN** the second `just test` reuses the host build output and recompiles no workspace crate

#### Scenario: No host cargo

- **WHEN** a machine has Docker but no host rustup
- **THEN** the Rust recipes run in the build image instead of failing

#### Scenario: Determinism check with a warm cache

- **WHEN** the generate-twice check runs on a machine with a warm compiler cache
- **THEN** both builds compile without the cache, so non-deterministic output still fails the check

### Requirement: Pre-commit hooks

`just setup` SHALL install lefthook pre-commit hooks that format staged files and run typos, whitespace and JSON hygiene and a secret scan of staged changes. There SHALL be no commit-msg hook; hooks are fast feedback and CI re-runs everything. Outside CI, `just lint` SHALL warn, without failing, when the hooks are not active.

#### Scenario: Unformatted Markdown

- **WHEN** a contributor commits an unformatted Markdown file
- **THEN** the hook formats it and stages the result

#### Scenario: Hooks not active

- **WHEN** the lefthook pre-commit hook is not installed and a contributor runs `just lint` locally
- **THEN** `just lint` prints a warning that names `just setup`, and its verdict is unchanged

## MODIFIED Requirements

### Requirement: CI and branch protection

The only required status check on `main` SHALL be `ci-ok`, which aggregates every job of the CI workflow, PR title check included. Actions SHALL be pinned by SHA with default `permissions: {}`. `main` SHALL require `ci-ok` and linear history, without required reviews or signed commits, and SHALL refuse merge commits. The PR title SHALL follow Conventional Commits. The maintainer SHALL rebase-merge only a branch whose every commit is Conventional and green, otherwise squash.

#### Scenario: PR title without a type

- **WHEN** a pull request is titled "update docs"
- **THEN** the PR title check fails

#### Scenario: Merge commit attempted

- **WHEN** someone tries to merge a pull request into `main` with a merge commit
- **THEN** GitHub refuses it because linear history is required

#### Scenario: Red required check

- **WHEN** `ci-ok` fails on a pull request
- **THEN** the pull request cannot be merged into `main`

### Requirement: Rust rules

Rust crates SHALL use edition 2024 and inherit the workspace lints (checked by a lint), pass `cargo fmt` and `clippy -D warnings`, forbid `unsafe` outside generated FFI glue, and deny `unwrap`, `expect`, `panic` and unchecked indexing in parser modules. The core, `guardedsleep-relay` and `guardedsleep-ffi` SHALL stay buildable for iOS (`cargo check --target aarch64-apple-ios` in `just lint`).

#### Scenario: New crate without workspace lints

- **WHEN** a new crate does not inherit the workspace lints
- **THEN** the lint check fails

#### Scenario: Core stops compiling for iOS

- **WHEN** a change makes `guardedsleep-ffi` fail to compile for `aarch64-apple-ios`
- **THEN** `just lint` fails

### Requirement: Gate register and budget

Every workflow, and every job of the CI workflow, SHALL have one row in `docs/ci-gates.md` with what it protects, a one-line runbook and a last-catch date; checks inside a job are named in its row. The maintainer owns every gate during alpha. No such job or workflow SHALL be added without a row. Tooling maintenance SHALL stay within 12 hours per month; a gate without a catch in six months SHALL be removed or demoted to nightly.

#### Scenario: New CI check

- **WHEN** a change adds a new CI job
- **THEN** the same change adds its row to `docs/ci-gates.md`

#### Scenario: New check inside an existing job

- **WHEN** a change adds a check to the `lint` job
- **THEN** the same change names it in the `lint` row instead of adding a row

### Requirement: Load slots

Medium and heavy recipes SHALL run through a machine-wide slot semaphore shared by all worktrees, with slot counts from documented defaults that the environment can override. The wait for a slot SHALL be bounded, and an optional run deadline SHALL start only once the slot is held; both failures SHALL be distinguishable from a test failure.

#### Scenario: Queued job with a run deadline

- **WHEN** a recipe with a 10-minute run deadline waits 20 minutes for a medium slot and then runs for 5 minutes
- **THEN** it completes with the test verdict and is not cut off by the deadline

#### Scenario: Run exceeds the deadline

- **WHEN** a slot-held run takes longer than its run deadline
- **THEN** it is terminated with a message that names the deadline, not reported as a test failure

### Requirement: Delta tests

`just test-affected [base]` SHALL run the tests of the areas touched since the merge base with `base` (default `origin/main`), including uncommitted and untracked files, using the same path map as CI. A global or unmapped path SHALL run the full `just test`. It SHALL name the heavy checks the change needs in the full sweep.

#### Scenario: Web-only change

- **WHEN** only a file under `apps/web/src/` changed
- **THEN** `just test-affected` runs the affected web tests, skips the Rust and Android tests, and names `just e2e` for the full sweep

#### Scenario: Unmapped path

- **WHEN** a file in a new top-level directory changed
- **THEN** `just test-affected` runs the full `just test` and says why

## REMOVED Requirements

### Requirement: Local compiler cache

**Reason**: Cache layout, size cap and environment switches are implementation details. After "Rust execution mode" the cache serves only container builds.

**Migration**: Documented in `docs/contributing/development.md` ("Compiler cache"). The binding part, no cache for the determinism check and release builds, moves to "Rust execution mode".

### Requirement: Run timings

**Reason**: Nobody has read the timings file since wave 3 introduced it, and it adds code to every slot-held run. The tooling budget does not cover it.

**Migration**: `scripts/lib/timings.sh` and `just timings` are deleted, and `slot.sh` stops recording. An existing `~/.cache/guardedsleep/timings.tsv` can be deleted by hand.

### Requirement: Worktree cleanup

**Reason**: Automatic pruning is rarely used, and `git worktree remove` already covers it. The listing is a convenience, not a guarantee.

**Migration**: `just reap --worktrees` keeps listing removable and kept worktrees (documented in `docs/contributing/development.md`). `--prune-worktrees` is removed; remove a listed worktree with `git worktree remove <path>`.

### Requirement: Git hooks

**Reason**: Replaced by "Pre-commit hooks", which keeps the behaviour and drops the repair detail for clones of the former Husky setup.

**Migration**: `just setup` still removes a stale Husky `core.hooksPath` (documented in `docs/contributing/development.md`, "Hooks"); the warning in `just lint` now covers any inactive hook.
