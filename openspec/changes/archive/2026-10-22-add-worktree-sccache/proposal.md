# Proposal

## Why

Every new agent worktree compiles the Rust workspace cold (12–24 min on the reference laptop): the shared Cargo home (AGENTS.md §10.1) shares downloads, not compiled artefacts. With several agents per task this is the largest single source of waiting time.

ADR-020 put sccache in **T2** ("Caching: rust-cache and setup-gradle; sccache is T2"). This change **pulls it forward to T0 for local builds only**, which revises a prior decision, so the Critical Reviewer role applies (design.md, "Critical review"). CI keeps rust-cache; the T2 entry for CI caching is unchanged.

Persona impact: none (contributors and agents only). Out of scope: sccache in CI, remote cache backends (S3, GHA, Redis), sharing `target/` between worktrees, Gradle build cache changes.

## What Changes

- The build image (`docker/android-native-build`) ships a pinned, checksum-verified prebuilt `sccache` binary.
- `scripts/lib/build-image.sh` mounts a shared local cache `${GS_SHARED_CACHE}/sccache` and sets `RUSTC_WRAPPER=sccache` for every local `docker run` (cargo, core builds, Gradle/cargo-ndk), capped by `GS_SCCACHE_SIZE` (default 10G). `GS_SCCACHE=0` turns it off.
- The generate-twice check and release builds run without sccache, so a cache hit can never mask non-determinism or put a locally cached artefact into a release.
- Renovate tracks the sccache pin; `docs/ci-gates.md`, AGENTS.md (§9 row, §10.1) and the backlog are updated.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: a new "Local compiler cache" requirement.

## Impact

`docker/android-native-build/Dockerfile`, `scripts/lib/build-image.sh`, `scripts/core-build.sh` (determinism, release paths), `renovate.json`, AGENTS.md, `docs/contributing/development.md`, `docs/backlog.md`. Disk: one bounded cache (default 10 GB) under `~/.cache/guardedsleep/sccache`.
