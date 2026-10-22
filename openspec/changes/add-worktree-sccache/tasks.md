# Tasks

## 1. Image

- [ ] 1.1 Brief (implementer): in `docker/android-native-build/Dockerfile` add `ARG SCCACHE_VERSION=0.18.0` and the per-arch sha256 (read the values from the release's `.sha256` assets with `gh api`; never invent them), download the musl tarball for `TARGETARCH`, verify, install `/usr/local/bin/sccache`. Comment with the release URL. Add a Renovate regex manager for the `ARG` (`github-releases`, `mozilla/sccache`). Acceptance: the image builds; `docker run … sccache --version` prints 0.18.0.

## 2. Wiring

- [ ] 2.1 Brief (implementer): `scripts/lib/build-image.sh` adds the sccache mount and env per design §3 (off when `GS_SCCACHE=0` or `CI` is set); update its header comment. `scripts/core-build.sh determinism` and release build paths pass `-e RUSTC_WRAPPER=` after the cache args. Acceptance: shellcheck clean; a container started by `scripts/cargo.sh` sees `RUSTC_WRAPPER=sccache`; the determinism container sees it empty.

## 3. Verification

- [ ] 3.1 Cold build in worktree A, then in a fresh worktree B: record wall times and `sccache --show-stats` hit rate in the PR.
- [ ] 3.2 Two parallel builds in two worktrees against one cache both succeed (critical review finding 2).
- [ ] 3.3 `just check-determinism` with a warm cache still compiles both times.

## 4. Docs

- [ ] 4.1 AGENTS.md §9 row and §10.1 (shared caches), `docs/contributing/development.md`, `docs/ci-gates.md` notes, delete the backlog `IDEA`.

## 5. Verify

- [ ] 5.1 `just lint` and `just test`.
