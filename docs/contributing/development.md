# Development environment

This repo uses **Node.js** (see `package.json` `engines`), **pnpm**, **Python** (for [pre-commit](https://pre-commit.com)), and optional **GitHub CLI** in the Dev Container.

## Dev Container (recommended)

The environment is defined by **Docker Compose** (`.devcontainer/docker-compose.yml`) and **Dockerfile** (`.devcontainer/Dockerfile`). The workspace is mounted at **`/workspace`** — the same layout is used in the IDE and on the CLI.

1. Open the repository in **VS Code** or **Cursor** with the Dev Containers extension.
2. **Reopen in Container** — on first start the image is built, then `.devcontainer/post-create.sh` runs once.

After creation you should have:

- `node` / `pnpm` at the versions pinned in the devcontainer
- `pre-commit` on `PATH` (`~/.local/bin`)
- Husky hooks under `.husky/` (`core.hooksPath` set via `pnpm run prepare`)

The dev image includes **Eclipse Temurin JDK 21** (copied into `/opt/java/openjdk`) and a minimal **Android SDK** (platform 35, build-tools 35.0.0) so you can run **`pnpm … build`**, **`cap sync`**, and **`./gradlew assembleDebug`** in the **same** container — no second Compose stack required.

- **Trade-off:** the image is larger and the first `docker compose … build` for `.devcontainer/` takes longer (SDK download).
- **ARM64:** the Temurin stage is amd64-oriented; on Apple Silicon, use **Docker Rosetta** / amd64 platform for this image, or maintain an arm64 JDK variant if needed.
- **Shell `PATH`:** the Node dev image loads **nvm** in login shells; the Dockerfile appends `JAVA_HOME` / `ANDROID_HOME` entries to `/etc/profile.d` and `~/.bashrc` so `java` and `./gradlew` work in integrated terminals.
- **Optional:** [docker/android-build](../../docker/android-build/README.md) remains useful for **CI** or host-only Gradle without the full dev image.

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

After `pnpm --filter @guardedsleep/mobile build`, sync native projects and open Android Studio:

```bash
cd apps/mobile
pnpm run cap:sync
pnpm run cap:open:android
```

The `android/` directory is tracked in git (run `pnpm install` at the repo root so `@capacitor/android` is available).

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
docker compose -f docker/stack/compose.yaml up --build
```

(Optional: `docker/stack/.env` + `--env-file docker/stack/.env` for LAN or non-default ports — see [docker/stack/README.md](../../docker/stack/README.md).)

### Signaling server smoke test

End-to-end WebSocket hello + relay (builds the server, then starts `dist/index.js` in a child process; child stdio is discarded so `console.log` in the server cannot block on a full pipe):

```bash
pnpm --filter @guardedsleep/signaling-server smoke
```

Commit messages are validated by **commitlint** (`.husky/commit-msg`). The hook runs `./node_modules/.bin/commitlint` from the repo root — it needs **`node` on `PATH`** and a completed **`pnpm install`**; it does **not** invoke `pnpm` (so GUI Git clients with a minimal `PATH` still work if `node` is available, e.g. under `/usr/bin`).

Example:

`feat(docs): describe pairing flow in connectivity doc`

## Python path

If `pre-commit` is not found, add the user script directory to `PATH`, for example:

`export PATH="$HOME/.local/bin:$PATH"`

## Building and testing apps

Application packages under `apps/` each document `pnpm` scripts (build, test). **Browser E2E** for the mobile web client lives in `apps/e2e` ([ADR-007](../decisions/ADR-007-browser-e2e-test-strategy.md), `pnpm run e2e`). Until additional per-app harnesses exist, quality checks remain documentation + tooling as above for packages without their own E2E.
