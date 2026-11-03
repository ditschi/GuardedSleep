# dev-toolchain Specification

## Purpose

Give every contributor, human or agent, one way to set up, check, test and build the repository with the same verdict as CI, with pinned tools, guarded secrets and licences, and tests that prove device behaviour before hardware. Migrated from ADR-020 (wave 1), ADR-016, ADR-007 and ADR-019, as built.

## Requirements

### Requirement: One entry point

`just` SHALL be the single entry point with the vocabulary `setup`, `fmt`, `lint`, `test`, `gen`, `build`, `e2e`, `gate` and `ci`, plus sub-recipes. Recipes SHALL call the native tools and not re-implement them, and `just ci` SHALL give the same verdict as CI except for the checks that only CI can run (PR title, emulator gate).

#### Scenario: Before opening a pull request

- **WHEN** a contributor runs `just ci` and it passes
- **THEN** CI's required check passes for the same commit, apart from the PR title

### Requirement: Missing prerequisites

A task that needs Docker, KVM or the Android SDK SHALL print `SKIPPED: <task> needs <x>` and exit 0 when the prerequisite is missing. With `GS_STRICT=1`, as set in CI, a missing prerequisite SHALL fail the task instead.

#### Scenario: Laptop without KVM

- **WHEN** `just gate` runs on a machine without KVM
- **THEN** it prints that it was skipped because KVM is missing, and it fails if `GS_STRICT=1` is set

### Requirement: Pinned tools

Every tool version SHALL be pinned in exactly one place: the Rust toolchain in `rust-toolchain.toml` (exact stable, with a separate nightly only for fuzzing and `cargo public-api`), all other tools in `mise.toml` with checksums in `mise.lock`, and Android build versions in the Gradle version catalog and wrapper. CI SHALL refuse tools that are not in `mise.lock`. Every version-specific setting SHALL carry a comment with a documentation link.

#### Scenario: Tool bump without a lockfile update

- **WHEN** `mise.toml` changes a version but `mise.lock` is not updated
- **THEN** CI fails to install the tools

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

### Requirement: Supply chain

`cargo-deny` SHALL check Rust advisories, AGPL-compatible licences, crates.io as the only source and wildcard versions, and SHALL ban tokio, mio, getrandom and socket crates from the core graph. A guard SHALL fail when the core's dependency count changes without a reviewed bless. `pnpm audit` and `cargo deny` SHALL also run daily.

#### Scenario: Async runtime pulled into the core

- **WHEN** a change adds a dependency that brings tokio into the core's graph
- **THEN** `just deny` fails

### Requirement: Dependency updates

Renovate SHALL propose updates weekly with a minimum release age of 3 days. libwebrtc and Gradle plugins SHALL use a 7-day minimum release age, and libwebrtc updates SHALL come in their own PR and never be automerged.

#### Scenario: libwebrtc update

- **WHEN** Renovate proposes a new `io.github.webrtc-sdk:android` version
- **THEN** it is its own PR, at least 7 days old, and needs manual review

### Requirement: Licence compliance

`reuse lint` SHALL keep every file licensed: AGPL-3.0-or-later by default, CC0-1.0 for configuration, lockfiles and test vectors, and the upstream licence for vendored or generated third-party files.

#### Scenario: New file type without a licence

- **WHEN** a contributor adds a file that no REUSE annotation covers
- **THEN** `just lint-text` fails

### Requirement: Secret scanning

Betterleaks SHALL scan the full git history in `just lint` and staged changes in the pre-commit hook, with live validation off. A false positive SHALL be allowlisted with a comment naming the fixture; a real hit means rotating the secret first.

#### Scenario: Test PSK in a fixture

- **WHEN** a committed test vector contains a value that looks like a key
- **THEN** the scan reports it until it is allowlisted with a comment naming the fixture

### Requirement: Nothing generated is committed

Generated bindings, generated TypeScript and i18n output SHALL NOT be committed; `just gen` and the builds produce them. The one exception is the API-surface snapshots (`crates/guardedsleep-ffi/api.txt`, `crates/guardedsleep-wasm/api.txt` and `packages/core-wasm/api/core.d.ts`): they are the only committed generated files, serve as review artefacts, and are never imported by a build. Screenshot baselines SHALL be recorded only in the pinned build image, never edited by hand.

#### Scenario: Stale generated file

- **WHEN** a contributor tries to commit generated i18n output
- **THEN** it is ignored by git and regenerated by the build

#### Scenario: Snapshot is the only generated file in the tree

- **WHEN** a contributor commits generated Kotlin bindings or the generated wasm package
- **THEN** it is ignored by git, while the API-surface snapshots stay tracked and are checked by `just lint`

### Requirement: Test layers

Behaviour SHALL be proven locally first, then on emulators, then on hardware. The layers are: Rust unit, property and snapshot tests with conformance vectors and traces; seeded host simulation scenarios; Android JVM tests with Roborazzi screenshots; Playwright browser E2E against the local stack; and the emulator gate on KVM (AC-1 to AC-4 and AC-10: Doze, foreground service, alerts, link recovery).

#### Scenario: Link recovery change

- **WHEN** a change touches link recovery
- **THEN** the host simulation scenarios pass before the emulator gate runs it on devices

### Requirement: Vectors and flaky tests

A behaviour change SHALL re-bless the affected conformance vectors and traces, and the diff SHALL be reviewed by hand. A test that flakes twice in seven days SHALL be quarantined with an issue instead of retrying the job.

#### Scenario: Changed trace

- **WHEN** a core change alters a recorded trace
- **THEN** the contributor blesses it and the reviewer reads the trace diff

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

### Requirement: Spec validation

`just lint-text` SHALL run `openspec validate --all --strict`, so every spec and active change is well-formed.

#### Scenario: Requirement without a scenario

- **WHEN** a spec requirement has no scenario
- **THEN** `just lint-text` fails

### Requirement: Binding-free core

Binding attributes (UniFFI, wasm-bindgen) SHALL appear only in the façade crates `guardedsleep-ffi` and `guardedsleep-wasm`, never in `guardedsleep-core`, except for the tsify derive allowed by the requirement "tsify exception in the core". The web client's protocol types (`packages/core-wasm`) SHALL be generated from the core.

#### Scenario: UniFFI attribute in the core

- **WHEN** a change adds a UniFFI attribute to `guardedsleep-core`
- **THEN** the build or lint fails

### Requirement: tsify exception in the core

`guardedsleep-core` SHALL carry no binding attribute except `#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]` behind the non-default `tsify` feature, which only `guardedsleep-wasm` enables.

#### Scenario: wasm-bindgen in the core's default graph

- **WHEN** a change makes `wasm-bindgen`, `js-sys` or `web-sys` part of the default-feature graph of `guardedsleep-core`
- **THEN** the binding-free check in `just deny` fails

### Requirement: API-surface snapshots

The public API of `guardedsleep-ffi` and `guardedsleep-wasm` and the generated `.d.ts` SHALL be committed as snapshots, and CI SHALL fail when they differ from the code. CI SHALL build the core, `guardedsleep-ffi` and `guardedsleep-wasm` twice, the second time after cleaning those three crates, generate the bindings from each build, and fail when the hashes differ.

#### Scenario: Unnoticed API change

- **WHEN** a change renames a function exported to Kotlin without updating the snapshot
- **THEN** CI fails and shows the API diff

### Requirement: Vectors through every binding

Conformance vectors, including negative vectors, SHALL be evaluated through the exported entry point of the Rust core, the Kotlin (UniFFI) bindings and the TypeScript (wasm-bindgen) bindings. Signaling and DataChannel messages from the peer SHALL be parsed only in the core; the bindings pass them through as opaque strings.

#### Scenario: Binding diverges from the core

- **WHEN** the UniFFI or wasm build of the core returns a different result than the Rust core for a vector
- **THEN** that binding's conformance test fails

### Requirement: Trace replay through the binding handles

The committed link traces SHALL replay through each binding's `Link` handle: in Kotlin through the generated UniFFI types (`CoreLink`), comparing action and event kinds, attempt numbers, message texts and SDP with the committed expectation; in TypeScript through `WasmLink` (JSON in, JSON out), comparing the full action JSON.

#### Scenario: Lift bug in a binding

- **WHEN** a binding returns a different action kind, attempt number, text or SDP than the committed trace
- **THEN** that binding's replay test fails

### Requirement: libwebrtc patch policy

A critical or exploited CVE in a used WebRTC path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) that the vendor has not patched 30 days after the Chrome fix, or six months without a vendor release, SHALL trigger a change that switches the vendor, builds a patched version or disables the feature.

#### Scenario: Unpatched SRTP CVE

- **WHEN** a critical SRTP CVE is still unpatched by the vendor 30 days after Chrome fixed it
- **THEN** a change is opened to switch vendor, patch-build or disable the affected feature

### Requirement: libwebrtc vendor isolation

Only files in the engine package (`org.guardedsleep.app.media.libwebrtc`) SHALL import `org.webrtc`; the rest of the Android app talks to the `MediaEngine` interface.

#### Scenario: Vendor import outside the engine package

- **WHEN** a file outside `media/libwebrtc` imports `org.webrtc`
- **THEN** the Android unit tests fail

### Requirement: libwebrtc sourcing

The libwebrtc artifact SHALL resolve from Maven Central only (Gradle `exclusiveContent` on `io.github.webrtc-sdk`), and Google Maven SHALL serve only Google and AndroidX groups.

#### Scenario: Same group published on another repository

- **WHEN** another repository serves a component in group `io.github.webrtc-sdk`
- **THEN** Gradle ignores it and resolves from Maven Central

### Requirement: Android module boundaries

The Android app SHALL consist of the modules `app`, `ui-contract`, `platform` and `core-bindings`, and only `platform` SHALL depend on `core-bindings`, so UI code that imports a binding type does not compile.

#### Scenario: Compose screen imports an FFI type

- **WHEN** a screen in `app` imports a type from `core-bindings`
- **THEN** the build fails

### Requirement: Kotlin formatting and lint

`just fmt` and `just fmt-check` SHALL format and check Kotlin with the pinned ktfmt, and Android Lint SHALL treat warnings as errors in every module with no baseline file.

#### Scenario: Unformatted Kotlin file

- **WHEN** a Kotlin file is not ktfmt-formatted
- **THEN** `just fmt-check` fails

#### Scenario: Lint warning

- **WHEN** Android Lint reports a warning in any module
- **THEN** the build fails, and no baseline file hides it

### Requirement: Gradle wrapper checksum

The Gradle wrapper SHALL pin `distributionSha256Sum`.

#### Scenario: Tampered distribution

- **WHEN** the downloaded Gradle distribution does not match the pinned checksum
- **THEN** the wrapper refuses to run it

### Requirement: Binding seam guard

A unit test SHALL fail if a source file outside `platform` and `core-bindings` imports `uniffi.`, or if it finds no sources.

#### Scenario: UI imports a binding type

- **WHEN** a file in `app` imports `uniffi.`
- **THEN** the test fails

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

### Requirement: Web checks

The web client SHALL pass `svelte-check --fail-on-warnings` (a suppression needs an inline reason), Vitest for logic, and Playwright tests with fake media devices, axe and an overflow check. pnpm SHALL run with an install-script allow-list, a minimum release age of 3 days, blocked exotic sub-dependencies and a frozen lockfile in CI.

#### Scenario: Accessibility regression

- **WHEN** a change removes the label of a button on the web parent screen
- **THEN** the Playwright axe check fails

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
