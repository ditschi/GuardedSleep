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
    hooks_path="$(git config --local --get core.hooksPath || true)"
    if [ "${hooks_path#*.husky}" != "$hooks_path" ]; then
      git config --local --unset-all core.hooksPath
      echo "Removed the stale Husky core.hooksPath ($hooks_path); lefthook replaces it."
    fi
    mise exec -- lefthook install

# Format everything that is formatted automatically (Kotlin: see fmt-kotlin).
fmt:
    pnpm exec prettier --write .
    scripts/cargo.sh fmt --all
    taplo fmt

# Same checks without writing; also the whitespace/JSON hygiene the old pre-commit config had.
fmt-check:
    pnpm exec prettier --check .
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    git ls-files -z 'scripts/*.sh' | xargs -0 shellcheck -x -P SCRIPTDIR

# ktfmt over apps/android. Not part of fmt/fmt-check yet: 72 files differ today (fold in during wave 2).
fmt-kotlin *args:
    scripts/lib/ktfmt.sh {{ args }}

# Everything static: text, workflows, then the code checks. CI runs the three parts as separate jobs.
lint: lint-text lint-workflows lint-code

# Text and repository hygiene for every file, docs included: Prettier, spelling, whitespace/JSON hygiene,
# REUSE, OpenSpec spec/change validation and the secret scan over the full history (CI checks out
# with fetch-depth 0).
lint-text: _js-deps
    pnpm exec prettier --check .
    openspec validate --all --strict --no-interactive
    typos --force-exclude
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    reuse lint
    scripts/lib/secrets.sh

# GitHub workflow linters.
lint-workflows:
    actionlint
    zizmor --offline --min-severity medium .github

# Code: Rust format and clippy (all features), iOS check, supply chain, i18n, UI literals, types.
lint-code: _js-deps _js-gen check-ios
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/cspell-i18n.sh
    scripts/cargo.sh clippy --workspace --all-targets --all-features -- -D warnings
    just deny
    pnpm run i18n:check
    scripts/lint-ui-literals.sh
    pnpm -r --if-present run check

# The core must stay buildable for iOS (ADR-020 §3); rust-toolchain.toml installs the target.
check-ios:
    scripts/cargo.sh check -p guardedsleep-core --target aarch64-apple-ios

# cargo-deny (advisories, licences, sources, sans-IO bans) plus the core dependency-count guard.
deny:
    scripts/lib/deny.sh

# Secret scan of the full history (Betterleaks, validation off).
secrets:
    scripts/lib/secrets.sh

# Host Rust, WASM + node conformance, pnpm tests, Android JVM tests. Builds no Android ABI: the JVM tests
# load the host build of the core.
test: _js-deps _js-gen test-rust test-wasm test-web test-android

# Cargo workspace tests; nextest when installed, doctests through cargo test.
test-rust:
    if command -v cargo-nextest >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then cargo nextest run --workspace && cargo test --workspace --doc; else scripts/cargo.sh test --workspace; fi

# `_js-gen` has already built the WASM core (the core-wasm package build runs core-build.sh wasm).
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
    scripts/gradle.sh :core:testDebugUnitTest :app:testDebugUnitTest

# Generate the i18n output (Android resources, web JSON). Nothing generated is committed.
gen:
    pnpm run i18n:gen

# Everything: web, Android debug + release APKs, release-surface and core-size checks.
build: _js-deps build-web build-android

build-web:
    pnpm run build

# Core for Android (cross-compiled ABIs), debug and release APKs and their checks. Extra Gradle tasks
# join the single Gradle run, e.g. `just build-android :app:verifyRoborazziDebug`.
[script("bash")]
build-android *gradle_tasks:
    set -euo pipefail
    source scripts/lib/require.sh
    require build-android android-sdk
    scripts/core-build.sh android
    scripts/gradle.sh {{ gradle_tasks }} :app:assembleDebug :app:assembleRelease
    scripts/poc/release-check.sh
    scripts/poc/size-check.sh

# What the CI android job runs: build-android plus the JVM tests and Roborazzi screenshot verification.
verify-android:
    just build-android :core:testDebugUnitTest :app:testDebugUnitTest :app:verifyRoborazziDebug

# Browser E2E (Playwright, local dev stack). Prints SKIPPED without Playwright browsers unless GS_STRICT=1.
e2e: _js-deps _js-gen
    scripts/lib/e2e.sh

# Emulator gate AC-1..AC-4 (needs KVM; DOZE_MINUTES=2 just gate for a quick run). Run build-android first.
[script("bash")]
gate:
    set -euo pipefail
    source scripts/lib/require.sh
    require gate kvm docker
    scripts/poc/local-gate.sh

# The same verdict as CI (the PR title check and the emulator gate are CI-only).
ci: lint test build e2e

# Signaling server (legacy Node, wave 2 swaps in the Rust server) plus the web client, for local development.
dev:
    scripts/dev.sh

# Docker stack with the host LAN IP, for phone tests.
[script("bash")]
stack-lan:
    set -euo pipefail
    source scripts/lib/require.sh
    require stack-lan docker
    bash scripts/stack-lan-env.sh
    docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

stack-up:
    docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

diagnose-lan:
    bash scripts/diagnose-lan-connect.sh

check-kvm:
    bash scripts/check-kvm.sh

e2e-lan: _js-deps
    pnpm run e2e:lan

e2e-docker *args:
    bash scripts/e2e-docker-run.sh {{ args }}

audit:
    pnpm run audit

# Preview the Release Please output (maintainer; needs GH_TOKEN and GITHUB_REPOSITORY).
release-preview:
    npx --yes release-please release-pr --token=$GH_TOKEN --repo-url=$GITHUB_REPOSITORY --config-file=release-please-config.json --manifest-file=.release-please-manifest.json --dry-run --debug

# Generated, uncommitted packages that svelte-check and Vitest import (WASM core, protocol types).
[private]
_js-gen:
    pnpm --filter @guardedsleep/protocol build
    pnpm --filter @guardedsleep/core-wasm build

[private]
_js-deps:
    pnpm install --frozen-lockfile
