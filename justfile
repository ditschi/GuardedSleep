# Single entry point for tests, lint, builds and gates (ADR-019). Install: https://github.com/casey/just
# Host needs Docker + Node/corepack (pnpm); cargo is optional (scripts/cargo.sh falls back to the
# guardedsleep-android-native-build image). `just` itself is in the Dev Container.

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Install workspace dependencies (idempotent).
install:
    corepack enable 2>/dev/null || true
    pnpm install --frozen-lockfile

# Everything that must stay green: Rust, WASM, web/node, Android JVM unit tests.
test: test-rust test-wasm test-web test-android

# Cargo workspace tests (core incl. golden vectors, sim, i18n-gen, uniffi-bindgen).
test-rust:
    scripts/cargo.sh test --workspace

# WASM build + node conformance against the golden vectors (AC-5).
test-wasm:
    scripts/core-build.sh wasm
    node --test crates/guardedsleep-core/js/*.test.mjs

# pnpm workspaces: protocol, core-wasm, signaling server, web client.
test-web: install
    pnpm test

# Android :core and :app JVM unit tests in the build image (JNI libs + bindings first).
test-android:
    scripts/core-build.sh android
    scripts/gradle.sh :core:testDebugUnitTest :app:testDebugUnitTest

# Fast, repo-wide static checks plus fmt/clippy and the i18n/UI-literal gates.
lint: install
    pre-commit run --all-files
    scripts/core-build.sh test
    scripts/lint-ui-literals.sh
    pnpm run i18n:check

# Web build, Android debug + release APKs, release-surface and core-size checks.
build: install
    pnpm --filter @guardedsleep/mobile build
    scripts/core-build.sh android
    scripts/gradle.sh :app:assembleDebug :app:assembleRelease
    scripts/poc/release-check.sh
    scripts/poc/size-check.sh

# Emulator gate AC-1..AC-4 (needs KVM; DOZE_MINUTES=2 just gate for a quick run). Needs `just build` first.
gate:
    scripts/poc/local-gate.sh

# Browser E2E (Playwright, local dev stack).
e2e: install
    pnpm run e2e
