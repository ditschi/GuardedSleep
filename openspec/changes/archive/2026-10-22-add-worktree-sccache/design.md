# Design

## Context

Local Rust builds run in the `guardedsleep-android-native-build` image (`scripts/cargo.sh`, `scripts/core-build.sh`, `scripts/gradle.sh`), each worktree mounted at `/workspace` with its own `target/`. `build-image.sh` already shares the Cargo home and Gradle home across worktrees through `GS_DOCKER_CACHE_ARGS`. CI runs inside the image with `CORE_BUILD_NATIVE=1` and never goes through `GS_DOCKER_CACHE_ARGS`.

Two facts shape the options: the repo disk is ext4 (no reflinks) and at 98 % use, with 18 agent worktrees holding 88 GB; and every worktree compiles at the same container path, `/workspace`.

## Decisions

### 1. How worktrees share compiled artefacts

- [ ] **A. sccache, local disk backend, shared directory (recommended)**
  - Pros: content-addressed, so it works across branches and is never stale; identical container paths make cross-worktree hits work without path remapping; bounded size (LRU); one env block in `build-image.sh` covers cargo, core builds and cargo-ndk.
  - Cons: a new tool in the image (one pin to maintain); does not cache incrementally compiled workspace crates or final links, so workspace crates still rebuild (dependencies are the bulk of a cold build).
- [ ] **B. Seed a new worktree's `target/` from the main checkout**
  - Pros: no new tool.
  - Cons: ext4 has no reflinks, so each seed copies ~10 GB on a disk with 36 GB free; artefacts of another branch; cargo fingerprints by mtime and rebuilds much of it anyway.
- [ ] **C. One shared `CARGO_TARGET_DIR` for all worktrees**
  - Pros: no new tool, best hit rate on one branch.
  - Cons: cargo's build-directory lock serialises parallel agents; branches thrash each other's artefacts; breaks the "each worktree keeps its own `target/`" rule.

Recommendation: A.

### 2. How the binary gets into the image

- [ ] **A. Prebuilt musl release from `mozilla/sccache`, version as a Dockerfile `ARG`, verified against the release's `.sha256` checksum pinned next to it (recommended)**
  - Pros: seconds to install, no compile in the image build; Renovate regex manager updates version (checksum updated in the same PR by hand, like the other pinned downloads).
  - Cons: two values to bump together.
- [ ] **B. `cargo install --locked sccache` in the image**
  - Pros: built from source.
  - Cons: several minutes of image build, more deps in the image build graph.

Recommendation: A, pinned to v0.18.0 (latest release at proposal time, 2026-09-14), x86_64 and aarch64 by `TARGETARCH`.

### 3. Where it is turned on

`GS_DOCKER_CACHE_ARGS` gains, unless `GS_SCCACHE=0` or `CI` is set:

```
-v "$cache/sccache:/cache/sccache"
-e RUSTC_WRAPPER=sccache -e SCCACHE_DIR=/cache/sccache -e SCCACHE_CACHE_SIZE="${GS_SCCACHE_SIZE:-10G}"
```

Host cargo (when a contributor has one) is untouched: paths differ per worktree there, so hits would be rare, and changing a contributor's host environment is out of scope.

**Off for** `scripts/core-build.sh determinism` (the generate-twice check must compile twice for real) and for release builds (`build-android` release variants and anything a release tag builds): they pass `-e RUSTC_WRAPPER=` after the cache args.

## Critical review

Findings from the Critical Reviewer role, each answered:

- **CRITICAL – a cache hit masks non-determinism.** The generate-twice check would compare a build with its own cached copy and always pass. Answer: sccache off for the determinism path (§3); a task asserts it (`sccache` stats show zero requests in that run, or `RUSTC_WRAPPER` is empty in the container).
- **IMPORTANT – concurrent sccache servers on one local directory.** Each `docker run` starts its own server, and they share `SCCACHE_DIR`. Cache entries are written atomically (temp file + rename), so readers never see partial entries; the LRU size accounting is per process and can overshoot the cap. Answer: accepted; a verification task runs two parallel cold builds in two worktrees against one cache and checks both succeed; the cap is set below free disk with margin. Fallback if it misbehaves: one cache directory per medium slot.
- **IMPORTANT – disk pressure.** 36 GB free. Answer: 10 GB default cap, `GS_SCCACHE_SIZE` to lower it; the real lever is pruning stale agent worktrees (88 GB), which this change documents but does not do.
- **IMPORTANT – revises ADR-020's T2 placement.** Answer: scoped to local builds; CI caching and its T2 entry unchanged; the AGENTS.md §9 row becomes "sccache: local builds T0, CI T2".
- **MINOR – maintenance budget.** One more pin; Renovate tracks it. Owner and runbook in `docs/ci-gates.md` "Notes" (it is a cache, not a gate).
- **MINOR – cache poisoning.** The cache is local, written only by the user's own builds, and never feeds a release (§3).

## Guardrails and layers

Tooling only. No product layer; no network backend, so nothing leaves the machine. Releases are built without the cache.

## Discarded alternatives

- **Seeding `target/` (1B)** and **shared `CARGO_TARGET_DIR` (1C)**: see §1.
- **`cargo install sccache` (2B)**: image build time.
- **Remote backends (S3, GHA cache, Redis)**: third-party storage and secrets for a local problem; CI caching stays T2.
