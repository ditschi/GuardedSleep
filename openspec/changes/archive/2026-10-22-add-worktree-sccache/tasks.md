# Tasks

## 1. Image

- [x] 1.1 Brief (implementer): in `docker/android-native-build/Dockerfile` add `ARG SCCACHE_VERSION=0.18.0` and the per-arch sha256 (read the values from the release's `.sha256` assets with `gh api`; never invent them), download the musl tarball for `TARGETARCH`, verify, install `/usr/local/bin/sccache`. Comment with the release URL. Add a Renovate regex manager for the `ARG` (`github-releases`, `mozilla/sccache`). Acceptance: the image builds; `docker run … sccache --version` prints 0.18.0.

## 2. Wiring

- [x] 2.1 Brief (implementer): `scripts/lib/build-image.sh` adds the sccache mount and env per design §3 (off when `GS_SCCACHE=0` or `CI` is set); update its header comment. `scripts/core-build.sh determinism` and release build paths pass `-e RUSTC_WRAPPER=` after the cache args. Acceptance: shellcheck clean; a container started by `scripts/cargo.sh` sees `RUSTC_WRAPPER=sccache`; the determinism container sees it empty.

## 3. Verification

- [x] 3.1 Cold build in worktree A, then in a fresh worktree B: record wall times and `sccache --show-stats` hit rate in the PR.
- [x] 3.2 Two parallel builds in two worktrees against one cache both succeed (critical review finding 2).
- [x] 3.3 `just check-determinism` with a warm cache still compiles both times.

## 4. Docs

- [x] 4.1 AGENTS.md §9 row and §10.1 (shared caches), `docs/contributing/development.md`, `docs/ci-gates.md` notes, delete the backlog `IDEA`.

## 5. Verify

- [x] 5.1 `just lint` and `just test`.

## Results

- 3.1: `cargo test --workspace --no-run` plus the wasm release build, each run in a fresh `target/` mounted at `/workspace/target`: no sccache 90 s; cold with sccache 93 s (0/235 hits); warm 28 s (235/235 hits, 87 non-cacheable calls). Cache size about 200 MB.
- 3.2: two parallel cold builds against one empty cache both succeeded.
- 3.3: `just lint` with the cache in place: the generate-twice check compiled twice and reported identical hashes (kotlin, wasm-node, wasm-web).
- A different `CARGO_TARGET_DIR` per run gives 0 hits (the `-L dependency=` paths are part of the key); every worktree uses `/workspace/target`, so this does not affect real use.
