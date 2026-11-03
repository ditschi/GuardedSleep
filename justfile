# The single entry point for tasks (ADR-020 §1). Fixed vocabulary:
#   setup fmt lint test gen build e2e gate ci   (+ sub-recipes such as fmt-check, build-android)
# Install: `mise install` (pins in mise.toml). Recipes run with the mise-pinned tools and fall back to
# plain bash with a hint when mise is missing (scripts/lib/shell.sh).
#
# Tasks that need Docker, KVM or the Android SDK print "SKIPPED: <task> needs <x>" and exit 0.
# CI sets GS_STRICT=1, which turns every skip into a failure (scripts/lib/require.sh).
# just manual: https://just.systems/man/en/

set shell := ["./scripts/lib/shell.sh", "-c"]

[private]
default:
    @just --list

# One-time setup: pinned tools, JS dependencies, git hooks.
[script("bash")]
setup:
    set -euo pipefail
    if ! command -v mise >/dev/null 2>&1; then
      echo "mise is required: https://mise.jdx.dev/getting-started.html" >&2; exit 1
    fi
    mise install
    # rustup reads rust-toolchain.toml and installs the pinned toolchain, components and targets.
    if command -v rustup >/dev/null 2>&1; then rustup show >/dev/null; else echo "hint: install rustup (https://rustup.rs) for host cargo; Docker is the fallback."; fi
    mise exec -- pnpm install --frozen-lockfile
    # Clones that ran the old Husky setup still point core.hooksPath at .husky/_; lefthook refuses to
    # install over it, and the stale Husky hooks do nothing once .husky is gone.
    # The setting can live in the repository config (--local) or, in a linked worktree, in its own
    # config.worktree (--worktree); check and clear both scopes.
    for scope in local worktree; do
      hooks_path="$(git config "--$scope" --get core.hooksPath 2>/dev/null || true)"
      if [ "${hooks_path#*.husky}" != "$hooks_path" ]; then
        git config "--$scope" --unset-all core.hooksPath
        echo "Removed the stale Husky core.hooksPath ($hooks_path, $scope scope); lefthook replaces it."
      fi
    done
    mise exec -- lefthook install
# Lists the prerequisites of the recipes (mise tools, rustup, Docker, uv, Playwright browser, KVM, Android SDK, git hooks) with a fix for each missing one. Informational, always exits 0.
doctor:
    scripts/doctor.sh

# Format everything that is formatted automatically (Prettier, rustfmt, taplo, ktfmt).
fmt:
    pnpm exec prettier --write .
    scripts/cargo.sh fmt --all
    taplo fmt
    scripts/lib/ktfmt.sh

# Same checks without writing; also the whitespace/JSON hygiene the old pre-commit config had.
fmt-check:
    pnpm exec prettier --check .
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/lib/ktfmt.sh --dry-run --set-exit-if-changed
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    git ls-files -z 'scripts/*.sh' | xargs -0 shellcheck -x -P SCRIPTDIR

# ktfmt over apps/android (also part of fmt and fmt-check).
fmt-kotlin *args:
    scripts/lib/ktfmt.sh {{ args }}

# Everything static: text, workflows, then the code checks. CI runs the three parts as separate jobs.
lint: lint-text lint-workflows lint-code

# Text and repository hygiene for every file, docs included: Prettier, spelling, whitespace/JSON hygiene,
# REUSE, OpenSpec spec/change validation and the secret scan over the full history (CI checks out
# with fetch-depth 0).
lint-text: _js-deps
    scripts/lib/hooks-check.sh
    pnpm exec prettier --check .
    openspec validate --all --strict --no-interactive
    typos --force-exclude
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    scripts/lib/no-google-stun.sh
    reuse lint
    scripts/lib/secrets.sh

# GitHub workflow linters.
lint-workflows:
    actionlint
    zizmor --offline --min-severity medium .github

# Code: Rust format and clippy (all features), iOS check, supply chain, API snapshots, generate-twice, i18n, curated public lists (minimums, unlisted STUN hosts), UI literals, types.
lint-code:
    scripts/lib/slot.sh medium -- just _lint-code

[private]
_lint-code: _js-deps _js-gen check-ios check-determinism
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/lib/ktfmt.sh --dry-run --set-exit-if-changed
    scripts/cspell-i18n.sh
    scripts/cargo.sh clippy --workspace --all-targets --all-features -- -D warnings
    just deny
    just api-check
    pnpm run i18n:check
    pnpm run lists:check
    pnpm run lists:stun
    just lint-message-keys
    scripts/lint-ui-literals.sh
    pnpm -r --if-present run check

# Every `message_keys()` entry of the core exists in locales/en.json as `core.<camelCase>` (openspec i18n spec).
lint-message-keys:
    scripts/lib/slot.sh medium -- scripts/cargo.sh test -p guardedsleep-core --test message_keys_catalogue

# The core, the relay (shared with the Android LAN listener) and the UniFFI facade must stay buildable for iOS (ADR-020 §3); rust-toolchain.toml installs the target.
check-ios:
    scripts/cargo.sh check -p guardedsleep-core -p guardedsleep-relay -p guardedsleep-ffi --target aarch64-apple-ios

# Generate-twice check: the Kotlin bindings (from the host ffi library) and the wasm package are generated
# twice (the second time after a clean of the three workspace crates) and must hash identically. SKIPPED without Docker.
[script("bash")]
check-determinism:
    set -euo pipefail
    source scripts/lib/require.sh
    require check-determinism docker
    scripts/core-build.sh determinism

# API-surface snapshots of the binding facades (cargo public-api on the pinned nightly in tools/nightly, plus
# the generated .d.ts). Fails on a diff; `just api-snapshot` regenerates. SKIPPED without Docker.
api-check:
    scripts/lib/api-snapshot.sh check

# Regenerate crates/guardedsleep-{ffi,wasm}/api.txt and packages/core-wasm/api/core.d.ts after an intended change.
api-snapshot:
    scripts/lib/api-snapshot.sh update

# cargo-deny (advisories, licences, sources, sans-IO bans) plus the core dependency-count guard.
deny:
    scripts/lib/deny.sh

# Secret scan of the full history (Betterleaks, validation off).
secrets:
    scripts/lib/secrets.sh

# Host Rust, WASM + node conformance, pnpm tests, Android JVM tests. Builds no Android ABI: the JVM tests
# load the host build of the core.
test:
    scripts/lib/slot.sh medium -- just _test

[private]
_test: _js-deps _js-gen test-rust test-wasm test-web test-android test-tooling

# Unit tests of the repository scripts (delta-test planner and the like).
test-tooling:
    node --test scripts/lib/*.test.mjs scripts/spike/*.test.mjs

# Runs only the tests the changed paths need (committed since the merge base with <base>, uncommitted and
# untracked files), mapped by .github/path-filters.yml; global or unmapped paths run the full `just test`.
# The plan itself runs `_js-deps`/`_js-gen` where Vitest or WASM tests need them. `--dry-run` prints the
# plan only: `node scripts/lib/affected.mjs --dry-run [base]`.
test-affected base="origin/main":
    scripts/lib/slot.sh medium -- node scripts/lib/affected.mjs {{ base }}

# Cargo workspace tests; nextest when installed, doctests through cargo test.
test-rust:
    if command -v cargo-nextest >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then cargo nextest run --workspace && cargo test --workspace --doc; else scripts/cargo.sh test --workspace; fi

# `_js-gen` has already built (or found up to date) the WASM core.
test-wasm: _js-gen
    node --test crates/guardedsleep-core/js/*.test.mjs

test-web:
    pnpm test

# Android JVM unit tests (core + app) against the host build of the core, no ABI cross-compile.
[script("bash")]
test-android:
    set -euo pipefail
    source scripts/lib/require.sh
    require test-android android-sdk
    scripts/core-build.sh host
    scripts/gradle.sh buildLogicTest :core-bindings:testDebugUnitTest :platform:testDebugUnitTest :app:testDebugUnitTest

# Generate the i18n output (Android resources, web JSON) and the curated public lists (web module, CSP origins). Nothing generated is committed.
gen:
    pnpm run i18n:gen
    pnpm --filter @guardedsleep/web run lists:gen

# Everything: web, Android debug + release APKs, release-surface and core-size checks.
build:
    scripts/lib/slot.sh medium -- just _build

[private]
_build: _js-deps build-web build-android

build-web:
    pnpm run build

# Core for Android (cross-compiled ABIs), debug and release APKs and their checks. Extra Gradle tasks
# join the single Gradle run, e.g. `just build-android :app:verifyRoborazziDebug`.
build-android *gradle_tasks:
    scripts/lib/slot.sh heavy -- just _build-android {{ gradle_tasks }}

[private]
[script("bash")]
_build-android *gradle_tasks:
    set -euo pipefail
    source scripts/lib/require.sh
    require build-android android-sdk
    scripts/core-build.sh android
    scripts/gradle.sh {{ gradle_tasks }} :app:assembleDebug :app:assembleRelease
    scripts/poc/release-check.sh
    scripts/poc/size-check.sh

# What the CI android job runs: build-android plus Android Lint (warnings are errors), the JVM tests and Roborazzi screenshot verification.
verify-android:
    just build-android lintDebug buildLogicTest :core-bindings:testDebugUnitTest :platform:testDebugUnitTest :app:testDebugUnitTest :app:verifyRoborazziDebug

# Browser E2E (Playwright, local dev stack). Prints SKIPPED without Playwright browsers unless GS_STRICT=1.
e2e:
    scripts/lib/slot.sh heavy -- just _e2e

[private]
_e2e: _js-deps _js-gen
    scripts/lib/e2e.sh

# Emulator gate AC-1..AC-4 (needs KVM; DOZE_MINUTES=2 just gate for a quick run). Run build-android first.
gate:
    scripts/lib/slot.sh heavy -- just _gate

[private]
[script("bash")]
_gate:
    set -euo pipefail
    source scripts/lib/require.sh
    require gate kvm docker
    scripts/poc/local-gate.sh

# The same verdict as CI (the PR title check and the emulator gate are CI-only): lint, test, build, e2e in order,
# then a list of every SKIPPED line of the run (a skip is not a pass), also when a step fails.
ci:
    scripts/lib/ci.sh {{ just_executable() }}

# Rust signaling server plus the web client, for local development.
dev:
    scripts/dev.sh

# Runs one cargo-fuzz target (fuzz/Cargo.toml) on the pinned nightly of tools/nightly; extra args go to libFuzzer,
# e.g. `just fuzz envelope_open -- -max_total_time=60`. SKIPPED without cargo-fuzz and rustup.
fuzz target="envelope_open" *args:
    scripts/lib/fuzz.sh {{target}} {{args}}

# Signaling server smoke: starts the built guardedsleep-server binary and relays hello + signal over a WebSocket.
smoke:
    scripts/cargo.sh test -p guardedsleep-server --test smoke

# Two real Links over three relays picked from data/nostr-relays.json for N hours (default 8), a forced link
# loss every 10 minutes, a verdict per design section 9 of alpha-connection-model. Extra options go to gs-soak
# (`--relays`, `--tag-only`, `--loss-every-min`, `--trace`, see scripts/spike/soak --help). The report lands in
# .cache/relay-soak/. Runbook: docs/contributing/relay-soak.md.

# Go/no-go soak of the public Nostr mailboxes (task 4.2); needs network, exits 0 also for a no-go or inconclusive verdict.
soak hours="8" *args:
    scripts/lib/slot.sh heavy -- just _soak {{ hours }} {{ args }}

[private]
[script("bash")]
_soak hours *args:
    set -euo pipefail
    source scripts/lib/require.sh
    require soak cargo network
    rc=0
    scripts/spike/soak --hours {{ hours }} {{ args }} || rc=$?
    # gs-soak exits 0 for go, 3 for no-go, 4 for inconclusive: verdicts, not tool failures.
    case "$rc" in
      0 | 3 | 4) echo "soak verdict exit code: $rc (0 go, 3 no-go, 4 inconclusive)"; exit 0 ;;
      *) exit "$rc" ;;
    esac

# Measures the active relays of data/nostr-relays.json and the STUN servers of data/stun-servers.json (NIP-11,
# connect, delivery, 2-16 KiB events, retention, created_at tolerance, burst). Report in .cache/relays-check/.
# Weekly in CI (relays-weekly.yml), not part of `just ci`.

# Measure the listed Nostr relays and STUN servers; needs network, exits 1 when an active entry fails.
relays-check *args:
    scripts/lib/slot.sh medium -- just _relays-check {{ args }}

[private]
[script("bash")]
_relays-check *args:
    set -euo pipefail
    source scripts/lib/require.sh
    require relays-check cargo network
    node scripts/spike/relays-check.mjs {{ args }}

# Docker stack with the host LAN IP, for phone tests.
stack-lan:
    scripts/lib/slot.sh heavy -- just _stack-lan

[private]
[script("bash")]
_stack-lan:
    set -euo pipefail
    source scripts/lib/require.sh
    require stack-lan docker
    bash scripts/stack-lan-env.sh
    docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

stack-up:
    scripts/lib/slot.sh heavy -- docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

diagnose-lan:
    bash scripts/diagnose-lan-connect.sh

check-kvm:
    bash scripts/check-kvm.sh

audit:
    pnpm run audit

# Preview the Release Please output (maintainer; needs GH_TOKEN and GITHUB_REPOSITORY).
release-preview:
    npx --yes release-please release-pr --token=$GH_TOKEN --repo-url=$GITHUB_REPOSITORY --config-file=release-please-config.json --manifest-file=.release-please-manifest.json --dry-run --debug

# List leftover dev servers, stuck agent polls and old project containers; --stop ends them; --worktrees lists finished agent worktrees (AGENTS.md section 10.2).
reap *args:
    scripts/lib/reap.sh {{ args }}

# Generated, uncommitted packages that svelte-check and Vitest import (the WASM core); rebuilt only when its
# Rust inputs changed (scripts/lib/core-wasm-fresh.mjs). `pnpm --filter @guardedsleep/core-wasm build` forces it.
[private]
_js-gen:
    node scripts/lib/core-wasm-fresh.mjs ensure

[private]
_js-deps:
    pnpm install --frozen-lockfile
