# Proposal

## Why

Today UI code can import binding types, Kotlin is not formatted by `just fmt` (72 files differ), and `versionCode` is hand-edited. ADR-020 §4 decides convention plugins, module boundaries that make layer violations a compile error, ktfmt, strict Lint and a derived versionCode. Tier T0, wave 2 (group 2, after group 1).

## What Changes

- `build-logic` included build with convention plugins; configuration and build cache on.
- Modules `app`, `ui-contract`, `platform`, `core-bindings`; only `platform` depends on `core-bindings`.
- ktfmt (pinned jar) in `just fmt`/`fmt-check`; Android Lint with `warningsAsErrors`; R8 on for release.
- `versionCode` derived from `versionName` in `version.properties`.
- Gradle wrapper `distributionSha256Sum`; repository content filters (`google()` for AndroidX/`com.android`, everything else `mavenCentral()`, no JitPack).

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: Android module boundaries, Kotlin formatting and Lint.
- `distribution`: derived Android version code.

## Impact

`apps/android` (all modules, Gradle files), `justfile`, `docs/ci-gates.md`.
