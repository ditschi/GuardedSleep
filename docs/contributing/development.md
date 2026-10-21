# Development environment

Setup follows [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §1, §2, §5 and §8: **mise** pins every tool except Rust, **rustup** reads `rust-toolchain.toml`, **just** is the single entry point, **lefthook** runs the hooks and **pnpm 11** (from mise, no corepack) manages JavaScript. Host prerequisites: the version control client, [mise](https://mise.jdx.dev/getting-started.html) and, for host cargo, [rustup](https://rustup.rs). Docker is needed for the build image and the emulator gate, KVM for the gate.

## Quick start

```bash
mise install     # node 22, pnpm 11, java 21, just, lefthook, typos, actionlint, zizmor, reuse, cargo-deny, nextest, taplo, ktfmt, betterleaks (versions in mise.toml, checksums in mise.lock)
just setup       # mise install, rustup toolchain, pnpm install --frozen-lockfile, lefthook install
just --list      # setup fmt lint test gen build e2e gate ci (+ sub-recipes such as lint-text, build-android, dev)
just ci          # lint + test + build + e2e: the same verdict as CI (minus the PR title check and the emulator gate)
just dev         # signaling server (legacy Node) + Vite dev server for apps/mobile
```

Activate mise in your shell (`eval "$(mise activate bash)"`, see the [mise docs](https://mise.jdx.dev/getting-started.html#activate-mise)) so your editor sees the pinned tools. Recipes use the pinned tools through `mise exec`; without mise they fall back to the tools on `PATH` and print a hint.

### SKIPPED and `GS_STRICT`

A task that needs Docker, KVM, the Android SDK or Playwright browsers and does not find it prints `SKIPPED: <task> needs <x>` and exits 0 (`scripts/lib/require.sh`, `scripts/lib/e2e.sh`). **A SKIPPED is not a pass**: say so in reports. With `GS_STRICT=1` the same case prints `FAILED: ...` (or, for e2e, installs the browser) and exits 1. CI sets `GS_STRICT=1`, so nothing is silently skipped there.

### What each task covers

| Task                  | Content                                                                                                                                                                                                                        |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `just lint`           | `lint-text` (Prettier, typos, hygiene, REUSE, secret scan) + `lint-workflows` (actionlint, zizmor) + `lint-code` (cargo fmt, taplo, CSpell, clippy `--all-features`, iOS check, cargo-deny, i18n, UI literals, `svelte-check`) |
| `just test`           | Cargo workspace, WASM + node conformance, pnpm tests, Android JVM tests against the **host** build of the core (`scripts/core-build.sh host`): no ABI cross-compile                                                            |
| `just build`          | Web build plus `build-android`: cargo-ndk ABI libs, debug + release APKs, release-surface and core-size checks                                                                                                                 |
| `just verify-android` | `build-android` plus the JVM tests and Roborazzi screenshot verification in one Gradle run (the CI `android` job)                                                                                                              |
| `just e2e`            | Playwright against `just dev`; SKIPPED when Chromium is not installed (`pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium`)                                                                         |
| `just gate`           | Emulator gate (needs KVM and Docker); run `just build-android` first                                                                                                                                                           |

### Hooks

`just setup` runs `lefthook install`. A clone that ran the old Husky setup still has `core.hooksPath` pointing at `.husky/_`; `just setup` removes that setting (and says so) before installing lefthook. Without the cleanup the stale Husky hooks do nothing and no pre-commit check runs.

`lefthook.yml` runs on staged files only (target: under 5 s): Prettier, `cargo fmt` (when `.rs` files are staged and a host cargo exists), typos, whitespace/JSON hygiene and a Betterleaks secret scan. There is no commit-msg hook: Conventional Commits are checked on the PR title in CI. Hooks are a convenience; `just lint` and CI re-run everything. `lefthook run pre-commit --all-files` runs them on the whole tree.

The hook binary comes from mise. Git clients that do not load your shell profile (GUI clients, IDE commit buttons) only find it when the mise shims are on `PATH`: add `eval "$(mise activate --shims)"` to your login profile, otherwise the hook silently does not run there.

### Supply-chain and licence checks

`just deny` runs cargo-deny (`deny.toml`, plus `deny.core.toml` for the sans-IO bans on `guardedsleep-core` and later `guardedsleep-relay`) and the dependency-count guard (`.core-dep-count`; `scripts/lib/core-dep-count.sh --bless` after a reviewed change). Both look at the default-feature graph of the core; wave 2 moves uniffi into `guardedsleep-ffi` and the core graph is then checked with all features. `reuse lint` checks licence annotations (`REUSE.toml`). `just secrets` scans the full history with Betterleaks (`.betterleaks.toml`, live validation off).

## Dev Container (recommended)

The environment is defined by **Docker Compose** (`.devcontainer/docker-compose.yml`) and **Dockerfile** (`.devcontainer/Dockerfile`). The workspace is mounted at **`/workspace`** — the same layout is used in the IDE and on the CLI.

1. Open the repository in **VS Code** or **Cursor** with the Dev Containers extension.
2. **Reopen in Container** — on first start the image is built, then `.devcontainer/post-create.sh` runs once: `mise install`, `just setup`, `pnpm install --frozen-lockfile`.

The image carries the native toolchain (ADR-015, ADR-019), mirroring `docker/android-native-build/Dockerfile`: Eclipse Temurin JDK 21 (copied into `/opt/java/openjdk`), Android SDK (platforms 35 and 37.0, build-tools 35.0.0 and 36.0.0), NDK `30.0.16248370`, Rust from `rust-toolchain.toml` (including the Android, WASM and iOS check targets), `cargo-ndk` 4.1.2 and [mise](https://mise.jdx.dev/), which installs the rest (node, pnpm 11, just, lefthook, …) from `mise.toml`. `CORE_BUILD_NATIVE=1` is preset, so `scripts/core-build.sh` and `scripts/gradle.sh` run in place instead of starting the build image (there is no nested Docker).

- **Trade-off:** the image is larger and the first `docker compose … build` for `.devcontainer/` takes longer (SDK download).
- **ARM64:** the Temurin stage is amd64-oriented; on Apple Silicon, use **Docker Rosetta** / amd64 platform for this image, or maintain an arm64 JDK variant if needed.
- **Shell `PATH`:** the Node dev image loads **nvm** in login shells; the Dockerfile appends the mise shims, `JAVA_HOME` and `ANDROID_HOME` entries to `/etc/profile.d` and `~/.bashrc` so `just`, `java` and `./gradlew` work in integrated terminals.

```bash
just test    # cargo workspace, WASM + node conformance, pnpm tests, Android :core/:app unit tests
just lint    # text, workflows and code checks
just build   # web build, Android debug + release APKs, release/size checks
just gate    # emulator gate (needs KVM and Docker, so run it on the host)
```

On a host without the Dev Container only Docker is needed for the Rust and Android tasks: `scripts/cargo.sh`, `scripts/core-build.sh` and `scripts/gradle.sh` fall back to the `guardedsleep-android-native-build:local` image when `cargo` or `CORE_BUILD_NATIVE=1` is missing. `scripts/lib/build-image.sh` builds it on first use and rebuilds it when `rust-toolchain.toml` or the Dockerfile changes. `scripts/gradle.sh` serialises Gradle runs with `flock target/.gradle.lock`.

### Docker Compose without opening the IDE in Dev Container mode

If you have **Docker** and **Docker Compose** but do not want to attach the editor, run one-off commands in the same environment:

```bash
# From repository root — build the image once (or after Dockerfile changes)
docker compose --project-directory .devcontainer -f .devcontainer/docker-compose.yml build

# Install tools and dependencies, then run the checks
./scripts/devcontainer-run.sh bash -lc './.devcontainer/post-create.sh && just lint'

# Later: only what you need
./scripts/devcontainer-run.sh just test
```

Interactive shell with the repo at `/workspace`:

```bash
./scripts/devcontainer-run.sh bash
```

### Playwright browser E2E in the Dev Container

`just e2e` needs a Chromium install inside the container (not part of `post-create.sh`, which keeps first-time setup fast). In CI the recipe installs it itself; locally:

```bash
pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium
just e2e
```

If `--with-deps` complains about `apt`/permissions, run the install once as root and re-run as `node`:

```bash
docker compose --project-directory .devcontainer -f .devcontainer/docker-compose.yml run --rm -u root dev bash -lc \
  'cd /workspace && mise install && pnpm --filter @guardedsleep/e2e exec playwright install-deps chromium'
```

### Rootless Docker — `EACCES` on `/workspace/_tmp_*`

If `docker info` shows `rootless: true` and `pnpm install` fails with `EACCES: permission denied, open '/workspace/_tmp_…'`, the host user (typically uid 1000) is mapped to **container-uid 0** by the user namespace. The default container user `node` (also uid 1000 inside the container) then maps to a sub-uid on the host that has no write access to your repo files.

Run as container-root — rootless maps it back to your host user, so files on disk stay owned by you, not by real `root`:

```bash
./scripts/devcontainer-run.sh pnpm install
```

The wrapper pins Compose’s **project directory** to `.devcontainer/` (so `.devcontainer/.env` is always read for interpolation) and exports `DEV_UID` / `DEV_GID` when the daemon is rootless.

Manual equivalent:

```bash
DEV_UID=0 DEV_GID=0 docker compose --project-directory .devcontainer -f .devcontainer/docker-compose.yml run --rm dev pnpm install
```

To make rootless mapping permanent for **Cursor / VS Code Dev Containers** (the IDE does not run the shell wrapper), copy `.devcontainer/.env.example` to `.devcontainer/.env` and uncomment `DEV_UID=0` / `DEV_GID=0`. Rootful Docker users with uid 1000 leave both unset.

If you invoke `docker compose` yourself, always pass `--project-directory .devcontainer` (or `run` from inside `.devcontainer/`) so the same `.env` file is used; otherwise Compose may fall back to `user: 1000:1000` and you will see `EACCES` again.

If the version control client or `pnpm` still cannot write under `/workspace` (for example an `index.lock` permission denied) even when using `./scripts/devcontainer-run.sh`, your Docker engine probably does not advertise rootless in a way the script recognizes. Force the same uid mapping the rootless case uses:

```bash
GUARDSLEEP_DEVCONTAINER_UID=0 GUARDSLEEP_DEVCONTAINER_GID=0 ./scripts/devcontainer-run.sh bash
```

Only use `0` when you understand the trade-off from [AGENTS.md](../../AGENTS.md) (rootless: maps to your host user; classic rootful Docker: container root can create root-owned files on the bind mount).

### Host without Node/pnpm/mise (CLI only)

If you do **not** want to install Node, pnpm or mise on the host, use Docker only: tools and `node_modules` live in the mounted repo (or in the container’s view of `/workspace`), not as a global host install.

```bash
./scripts/dev-shell.sh
```

This builds the Dev Container image if needed and opens a shell with **`--service-ports`** so **Vite (5173)** and **signaling (8787)** reach your browser on the host. Inside it run `just setup` once, then `just dev`. Open e.g. `http://localhost:5173/`. The mobile dev server binds `0.0.0.0` so port publishing from Docker works.

## Local setup (without Dev Container or Docker)

Install [mise](https://mise.jdx.dev/getting-started.html) and, for host cargo, [rustup](https://rustup.rs), then run `just setup` (see Quick start). mise provides Node (≥ 22.18, matches `engines`), pnpm 11 and the other pinned tools; there is no Python or Husky setup.

## Local stack (signaling + web UI)

From the repository root, after `just setup`:

```bash
just dev
```

This builds `@guardedsleep/protocol` once, then runs the signaling server (`apps/signaling-server`, default port **8787**, `PORT` overrides it) and the SvelteKit dev server (`apps/mobile`, Vite default **5173**) in parallel; Ctrl-C stops both (`scripts/dev.sh`). Wave 2 swaps in the Rust server. Open e.g. `/baby?room=demo` and `/parent?room=demo` in two browser contexts to exercise pairing.

**PWA:** `vite dev` does not enable the service worker (`devOptions.enabled: false`). To test installability and precaching, build and preview the mobile app:

```bash
pnpm --filter @guardedsleep/mobile build
pnpm --filter @guardedsleep/mobile preview
```

**Docker (no Node on host):** build and run the **production-like** static stack (nginx + signaling) from the current revision — see [docker/stack/README.md](../../docker/stack/README.md) and [mobile-web-docker.md](mobile-web-docker.md):

```bash
# Phone/tablet on same Wi-Fi: auto LAN IP + stack
just stack-lan

# Single-machine / localhost only:
docker compose -f docker/stack/compose.yaml up --build
```

For a manual LAN IP: `bash scripts/stack-lan-env.sh` writes `docker/stack/.env`; then `just stack-up`.

### Signaling server (Docker)

For a container image and Compose (local build vs. GHCR), see [signaling-server-docker.md](signaling-server-docker.md). The images install pnpm at the version pinned in `mise.toml`.

### Signaling server smoke test

End-to-end WebSocket hello + relay (builds the server, then starts `dist/index.js` in a child process; child stdio is discarded so `console.log` in the server cannot block on a full pipe):

```bash
pnpm --filter @guardedsleep/signaling-server smoke
```

This legacy Node server and its smoke test are replaced by Rust server tests and the simulation in wave 2 ([ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §6).

## Commits

Conventional Commits are checked on the **PR title** in CI (squash merges); there is no local commit-msg hook. Example: `feat(docs): describe pairing flow in connectivity doc`.

## Building and testing apps

Application packages under `apps/` each document their `pnpm` scripts (build, test). **Browser E2E** for the mobile web client lives in `apps/e2e` ([ADR-007](../decisions/ADR-007-browser-e2e-test-strategy.md), `just e2e`). Other checks that have no recipe of their own: `pnpm run audit` (dependency advisories, `--audit-level=high`), `pnpm --filter @guardedsleep/mobile check` (`svelte-check`).
