# Development environment

Setup follows [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §1, §2, §5 and §8: **mise** pins every tool except Rust, **rustup** reads `rust-toolchain.toml`, **just** is the single entry point, **lefthook** runs the hooks and **pnpm 11** (from mise, no corepack) manages JavaScript. Host prerequisites: the version control client, [mise](https://mise.jdx.dev/getting-started.html) and, for host cargo, [rustup](https://rustup.rs). Docker is needed for the build image and the emulator gate, KVM for the gate.

## Quick start

```bash
mise install     # node 22, pnpm 11, just, lefthook, typos, actionlint, zizmor, reuse, cargo-deny, nextest, taplo, ktfmt, betterleaks (versions in mise.toml, checksums in mise.lock)
just setup       # mise install, rustup toolchain, pnpm install --frozen-lockfile, lefthook install
just --list      # setup fmt fmt-check lint test gen build build-android e2e gate ci (+ helpers)
just ci          # lint + test + build: the same verdict as CI
```

Activate mise in your shell (`eval "$(mise activate bash)"`, see the [mise docs](https://mise.jdx.dev/getting-started.html#activate-mise)) so the hooks and your editor see the pinned tools. Recipes use the pinned tools through `mise exec`; without mise they fall back to the tools on `PATH` and print a hint.

### SKIPPED and `GS_STRICT`

A task that needs Docker, KVM or the Android SDK and does not find it prints `SKIPPED: <task> needs <x>` and exits 0 (`scripts/lib/require.sh`). **A SKIPPED is not a pass**: say so in reports. With `GS_STRICT=1` the same case prints `FAILED: ...` and exits 1. CI sets `GS_STRICT=1`, so nothing is silently skipped there.

### Hooks

`lefthook.yml` runs on staged files only (target: under 5 s): Prettier, `cargo fmt` (when `.rs` files are staged and a host cargo exists), typos, whitespace/JSON hygiene and a Betterleaks secret scan. There is no commit-msg hook: Conventional Commits are checked on the PR title in CI. Hooks are a convenience; `just lint` and CI re-run everything. `lefthook run pre-commit --all-files` runs them on the whole tree.

### Supply-chain and licence checks

`just deny` runs cargo-deny (`deny.toml`, plus `deny.core.toml` for the sans-IO bans on `guardedsleep-core` and later `guardedsleep-relay`) and the dependency-count guard (`.core-dep-count`; `scripts/lib/core-dep-count.sh --bless` after a reviewed change). `reuse lint` checks licence annotations (`REUSE.toml`). `just secrets` scans the full history with Betterleaks (`.betterleaks.toml`, live validation off).

> The Dev Container and Docker sections below predate ADR-020 and stay valid until the build image and Dev Container are regenerated (wave 2). Where they mention Husky, `pre-commit`, corepack or pnpm 9.15.4, use the procedure above.

## Dev Container (recommended)

The environment is defined by **Docker Compose** (`.devcontainer/docker-compose.yml`) and **Dockerfile** (`.devcontainer/Dockerfile`). The workspace is mounted at **`/workspace`** — the same layout is used in the IDE and on the CLI.

1. Open the repository in **VS Code** or **Cursor** with the Dev Containers extension.
2. **Reopen in Container** — on first start the image is built, then `.devcontainer/post-create.sh` runs once.

After creation you should have:

- `node` / `pnpm` at the versions pinned in the devcontainer
- `pre-commit` on `PATH` (`~/.local/bin`)
- Husky hooks under `.husky/` (`core.hooksPath` set via `pnpm run prepare`)

The dev image includes **Eclipse Temurin JDK 21** (copied into `/opt/java/openjdk`) and a minimal **Android SDK** (platform 35, build-tools 35.0.0) so you can run **`pnpm … build`** and **`./gradlew assembleDebug`** in the **same** container — no second Compose stack required.

- **Trade-off:** the image is larger and the first `docker compose … build` for `.devcontainer/` takes longer (SDK download).
- **ARM64:** the Temurin stage is amd64-oriented; on Apple Silicon, use **Docker Rosetta** / amd64 platform for this image, or maintain an arm64 JDK variant if needed.
- **Shell `PATH`:** the Node dev image loads **nvm** in login shells; the Dockerfile appends `JAVA_HOME` / `ANDROID_HOME` entries to `/etc/profile.d` and `~/.bashrc` so `java` and `./gradlew` work in integrated terminals.

### Native and core development

The Dev Container also carries the native toolchain (ADR-015, ADR-019), mirroring `docker/android-native-build/Dockerfile`: Rust 1.99.1 (`wasm32-unknown-unknown`, `aarch64-linux-android`, `x86_64-linux-android`, clippy, rustfmt), `cargo-ndk` 4.1.2, Android NDK `30.0.16248370`, platform `android-37.0` with build-tools 36.0.0, and [`just`](https://github.com/casey/just). `CORE_BUILD_NATIVE=1` is preset, so `scripts/core-build.sh` and `scripts/gradle.sh` run in place instead of starting the build image (there is no nested Docker).

```bash
just test    # cargo workspace, WASM + node conformance, pnpm tests, Android :core/:app unit tests
just lint    # pre-commit, cargo fmt + clippy, UI-literal lint, i18n check
just build   # web build, Android debug + release APKs, release/size checks
just gate    # emulator gate (needs KVM and Docker, so run it on the host)
```

On a host without the Dev Container only Docker and Node/corepack are needed: `scripts/cargo.sh`, `scripts/core-build.sh` and `scripts/gradle.sh` fall back to the `guardedsleep-android-native-build:local` image (built on first use) when `cargo` or `CORE_BUILD_NATIVE=1` is missing. Install `just` from your package manager or `cargo install --locked just`. `scripts/gradle.sh` serialises Gradle runs with `flock target/.gradle.lock`.

### Docker Compose without opening the IDE in Dev Container mode

If you have **Docker** and **Docker Compose** but do not want to attach the editor (or you want a one-off build/lint in the same environment as CI/Dev Container):

```bash
# From repository root — build the image once (or after Dockerfile changes)
docker compose -f .devcontainer/docker-compose.yml build

# Install deps and run checks (first run: post-create installs pip/pre-commit tools)
docker compose -f .devcontainer/docker-compose.yml run --rm dev bash -lc './.devcontainer/post-create.sh && pnpm run lint'

# Later: only what you need (node_modules may already exist on the host from a previous run)
docker compose -f .devcontainer/docker-compose.yml run --rm dev pnpm run format:check
```

Shorter alias (same Compose project):

```bash
chmod +x scripts/devcontainer-run.sh
./scripts/devcontainer-run.sh pnpm run lint
```

### Running tests in the Dev Container

The Dev Container matches **pinned Node 22 + pnpm 9.15.4** (same family as CI) and includes **Python / pre-commit**, **JDK 21**, and a minimal **Android SDK** — so `pnpm test`, `pnpm run lint`, and `pnpm run build` behave like in automation.

**Vitest (and anything that does not open a browser):** from `/workspace` after `pnpm install` (post-create does this on first open):

```bash
pnpm test
pnpm --filter @guardedsleep/mobile check
```

**Playwright browser E2E** (`pnpm run e2e`) needs a **Chromium install** inside the container (not part of `post-create.sh` today — keeps first-time image setup faster). Mirror CI (`.github/workflows/ci.yml`):

```bash
pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium
pnpm run e2e
```

If `--with-deps` complains about `apt`/permissions, run it once as root and re-run as `node`:

```bash
docker compose -f .devcontainer/docker-compose.yml run --rm -u root dev bash -lc \
  'cd /workspace && corepack enable && corepack prepare pnpm@9.15.4 --activate && pnpm --filter @guardedsleep/e2e exec playwright install-deps chromium'
```

Then `pnpm --filter @guardedsleep/e2e exec playwright install chromium` as usual.

**One-shot from the host** (no IDE), same Compose service:

```bash
./scripts/devcontainer-run.sh bash -lc 'pnpm test'
./scripts/devcontainer-run.sh bash -lc 'pnpm run e2e'   # after Chromium install, see above
```

Interactive shell with the repo at `/workspace`:

```bash
docker compose -f .devcontainer/docker-compose.yml run --rm -it dev bash
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

If `git` or `pnpm` still cannot write under `/workspace` (for example `index.lock` permission denied) even when using `./scripts/devcontainer-run.sh`, your Docker engine probably does not advertise rootless in a way the script recognizes. Force the same uid mapping the rootless case uses:

```bash
GUARDSLEEP_DEVCONTAINER_UID=0 GUARDSLEEP_DEVCONTAINER_GID=0 ./scripts/devcontainer-run.sh git commit -m "…"
```

Only use `0` when you understand the trade-off from [AGENTS.md](../../AGENTS.md) (rootless: maps to your host user; classic rootful Docker: container root can create root-owned files on the bind mount).

### Host without Node/pnpm (CLI only)

If you do **not** want to install Node or pnpm on the host, use Docker only: tools and `node_modules` live in the mounted repo (or in the container’s view of `/workspace`), not as a global host install.

From the **repository root**:

```bash
chmod +x scripts/dev-shell.sh   # once, if needed
./scripts/dev-shell.sh
```

This builds the Dev Container image if needed, enables **Corepack + pnpm 9.15.4** inside the container, and opens a shell with **`--service-ports`** so **Vite (5173)** and **signaling (8787)** reach your browser on the host. Then:

```bash
pnpm install
pnpm run dev:local
```

Open e.g. `http://localhost:5173/`. The mobile dev server binds `0.0.0.0` so port publishing from Docker works.

**Cursor / VS Code:** You can instead use **Reopen in Container**; `post-create` already sets up pnpm. Port **5173** and **8787** are declared in Compose for CLI users; the IDE may also auto-forward when you start dev servers.

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §1, §5, §8:** pnpm 9.x via corepack, Python pre-commit and Husky wiring (`pnpm run prepare`). Target: `just setup` with tools pinned in `mise.toml`. Replaced in toolchain realignment wave 1; this procedure stays valid until then.

## Local setup (without Dev Container or Docker)

Requirements:

- **Node.js** ≥ 22.18 (matches CI and `engines`; CSpell in pre-commit enforces this floor)
- **pnpm** 9.x (`corepack enable && corepack prepare pnpm@9.15.4 --activate`, or install pnpm per [pnpm.io](https://pnpm.io/installation))
- **Python** 3.11+ with `pip install --user pre-commit` (or your preferred virtualenv)

Commands from the repository root:

```bash
pnpm install
chmod +x .husky/pre-commit .husky/commit-msg   # if your git checkout drops executable bits
pnpm run prepare    # Husky: wire Git to .husky/
```

Do **not** run `pre-commit install`. File checks are triggered by **Husky** (`.husky/pre-commit` runs `pre-commit run`). Running `pre-commit install` alongside Husky can break hook wiring.

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §1:** `pnpm run lint` (= `pre-commit run --all-files`) and the pnpm-based task table. Target: the fixed `just` vocabulary (`setup fmt lint test gen build e2e gate ci`); `just ci` gives the CI verdict. Replaced in toolchain realignment wave 1; this procedure stays valid until then.

## Checks (same as CI)

| Command                 | Purpose                                                                                                                                               |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm run lint`         | Runs `pre-commit run --all-files` (whitespace, YAML/JSON, **typos** + **CSpell** locale JSON, Prettier — see `.pre-commit-config.yaml`).              |
| `pnpm run format`       | Writes Prettier formatting for the repo (incl. `.svelte` via `prettier-plugin-svelte`).                                                               |
| `pnpm run format:check` | Fails if formatting differs (same scope as `pnpm run format`).                                                                                        |
| `pnpm run audit`        | Reports known dependency issues (`--audit-level=high`; CI runs the same).                                                                             |
| `pnpm test`             | Builds `@guardedsleep/protocol` to `dist/`, then runs Vitest in all workspace packages (`packages/protocol`, `apps/signaling-server`, `apps/mobile`). |
| `pnpm run e2e`          | Playwright browser E2E (`apps/e2e`); starts `dev:local` via Playwright `webServer` (see `apps/e2e/README.md`).                                        |
| `pnpm run build`        | Runs each package `build` script (protocol `tsc`, mobile static site, signaling-server `tsc`).                                                        |

From the repo root, targeted builds:

```bash
pnpm --filter @guardedsleep/mobile build
pnpm --filter @guardedsleep/signaling-server build
```

### Capacitor (Android)

Removed in wave 1 (ADR-020).

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §6:** the Node signaling server image; replaced by the Rust axum server. Replaced in toolchain realignment wave 2; this procedure stays valid until then.

### Signaling server (Docker)

For a container image and Compose (local build vs. GHCR), see [signaling-server-docker.md](signaling-server-docker.md).

### Local stack (signaling + mobile web UI)

From the repository root, after `pnpm install`:

```bash
pnpm run dev:local
```

This builds `@guardedsleep/protocol` once, then runs the signaling server (`apps/signaling-server`, default port **8787**) and the SvelteKit dev server (`apps/mobile`, Vite default **5173**) in parallel. Open e.g. `/baby?room=demo` and `/parent?room=demo` in two browser contexts to exercise pairing.

**PWA:** `vite dev` does not enable the service worker (`devOptions.enabled: false`). To test installability and precaching, build and preview the mobile app:

```bash
pnpm --filter @guardedsleep/mobile build
pnpm --filter @guardedsleep/mobile preview
```

**Docker (no Node on host):** build and run the **production-like** static stack (nginx + signaling) from the current revision — see [docker/stack/README.md](../../docker/stack/README.md) and [mobile-web-docker.md](mobile-web-docker.md):

```bash
# Phone/tablet on same Wi-Fi: auto LAN IP + stack (recommended until community signaling exists)
pnpm run stack:lan

# Single-machine / localhost only:
docker compose -f docker/stack/compose.yaml up --build
```

For manual LAN IP: `pnpm run stack:lan:env` writes `docker/stack/.env`; then `pnpm run stack:up`.

### Android debug APK: self-signed WSS (Docker `wss://…:8443`)

Removed in wave 1 (ADR-020).

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §6:** the Node signaling smoke test; replaced by Rust server tests and the simulation. Replaced in toolchain realignment wave 2; this procedure stays valid until then.

### Signaling server smoke test

End-to-end WebSocket hello + relay (builds the server, then starts `dist/index.js` in a child process; child stdio is discarded so `console.log` in the server cannot block on a full pipe):

```bash
pnpm --filter @guardedsleep/signaling-server smoke
```

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §8:** commitlint and the `commit-msg` hook; Conventional Commits are checked on the PR title only, in CI. Replaced in toolchain realignment wave 1; this procedure stays valid until then.

Commit messages are validated by **commitlint** (`.husky/commit-msg`). The hook runs `./node_modules/.bin/commitlint` from the repo root — it needs **`node` on `PATH`** and a completed **`pnpm install`**; it does **not** invoke `pnpm` (so GUI Git clients with a minimal `PATH` still work if `node` is available, e.g. under `/usr/bin`).

Example:

`feat(docs): describe pairing flow in connectivity doc`

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §8:** Python pre-commit; lefthook replaces it. Replaced in toolchain realignment wave 1; this procedure stays valid until then.

## Python path

If `pre-commit` is not found, add the user script directory to `PATH`, for example:

`export PATH="$HOME/.local/bin:$PATH"`

## Building and testing apps

Application packages under `apps/` each document `pnpm` scripts (build, test). **Browser E2E** for the mobile web client lives in `apps/e2e` ([ADR-007](../decisions/ADR-007-browser-e2e-test-strategy.md), `pnpm run e2e`). Until additional per-app harnesses exist, quality checks remain documentation + tooling as above for packages without their own E2E.
