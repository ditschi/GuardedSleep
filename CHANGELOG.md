# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Android docs:** [android-apk-build.md](docs/contributing/android-apk-build.md) now explains debug vs release signing, why CI builds debug only, building both variants locally, and Dev Container vs `docker/android-build`; [docker/android-build/README.md](docker/android-build/README.md) clarifies when to use which image.
- **Branding / technical IDs:** Renamed the npm workspace scope to `@guardedsleep/*`, the root package to `guardedsleep`, Android/Capacitor `appId` to `org.guardedsleep.app` (Java package `org.guardedsleep.app`), the browser `localStorage` signaling key to `guardedsleep:signaling-url`, and Docker image name examples in docs to the existing `guardedsleep-*` prefix. Existing installs must re-pair or re-enter signaling settings; Android is a new application id on device.
- **commitlint:** explicit `type-enum` added (mirrors `@commitlint/config-conventional` plus a project-specific `security` type used by Release Please for the `Security` changelog section).

### Added

- **GitHub Actions — Android debug APK ([`.github/workflows/android.yml`](.github/workflows/android.yml)):** on path-filtered PRs and pushes to `main` / `master` / `develop`, on tags `mobile-v*`, and via `workflow_dispatch`. Installs JDK 21 + Android SDK 35, runs `pnpm --filter @guardedsleep/mobile build`, `cap sync`, `./gradlew assembleDebug`, uploads `app-debug.apk` as workflow artifact. Documented in [docs/contributing/android-apk-build.md](docs/contributing/android-apk-build.md#ci-github-actions); `AGENTS.md` command table updated.
- **Browser E2E ([ADR-007](openspec/changes/archive/2026-05-13-adr-007-browser-e2e-test-strategy/design.md)):** `apps/e2e` Playwright workspace (Chromium, two contexts), CI job `e2e-browser`, root script `pnpm run e2e`; dev-only `window.__guardedsleep_test` hooks in mobile routes for `getStats` / pairing probes.
- Repository tooling: Dev Container, pre-commit (hooks + CI), Husky + commitlint, Prettier, typos.
- `AGENTS.md` — tool-agnostic agent briefing covering stack, conventions, role workflow, privacy guardrails, and not-yet-implemented features so any AI agent (Cursor, Claude Code, Codex CLI, Cline, Aider, Continue, …) shares the same context.
- **Release automation ([ADR-006](openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md)):** Release Please wired in monorepo manifest mode for `@guardedsleep/protocol`, `@guardedsleep/signaling-server`, `@guardedsleep/mobile`. Adds `release-please-config.json`, `.release-please-manifest.json`, `.github/workflows/release-please.yml` (push on `main` + `workflow_dispatch`), and the `release:lint` / `release:preview` root scripts. CI gains a `commitlint (PR commits)` job to catch non-Conventional commits introduced via rebase/amend. Full contributor workflow: [docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md). No release is produced until the maintainer sets `bootstrap-sha` after activation. **Versioning posture:** `@guardedsleep/mobile`'s version is the user-facing product version; `protocol` and `signaling-server` evolve independently. The `node-workspace` Release Please plugin auto-propagates patch bumps through the workspace dependency graph (so `protocol` bumps trigger explicit `Dependencies` entries on `mobile` and `signaling-server` in the same release PR). Project milestones (PoC, MVP, Phase 2, …) receive manual annotated meta-tags such as `mvp-v0.1.0` whose message lists the exact package triplet — see release-and-changelog.md §16–§19. Tag format is `<component>-v<semver>` with hyphen separator (Release Please default).
- Docker Compose (`.devcontainer/docker-compose.yml`) + Dockerfile: same environment for IDE Dev Containers and CLI (`docker compose run` / `scripts/devcontainer-run.sh`).
- MVP scaffold: monorepo (`pnpm-workspace.yaml`), `@guardedsleep/protocol`, `apps/signaling-server` (WebSocket relay), `apps/mobile` (SvelteKit + Tailwind + Vitest), CI job `test-and-build`.
- Capacitor Android project under `apps/mobile/android` (tracked), scripts `cap:sync` / `cap:open:android`; signaling-server `smoke` script and CI step for WebSocket relay smoke test.
- `@guardedsleep/protocol`: `tsc` emit to `dist/` (`tsconfig.build.json`); package `exports` point at compiled JS so `node dist/index.js` and smoke tests resolve the package without `tsx`.
- Signaling server: multi-stage `Dockerfile` under `docker/signaling-server/`, root `.dockerignore`, Compose files for local build (`compose.build.yaml`) and GHCR (`compose.ghcr.yaml`); docs in `docs/contributing/signaling-server-docker.md`; optional CI workflow `docker-signaling.yml` to build/push `ghcr.io/<owner>/guardedsleep-signaling`.
- **Container image build & distribution ([ADR-008](openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md)):** new `.github/workflows/docker-web.yml` publishes the PWA host image to GHCR; both `docker-signaling.yml` and `docker-web.yml` were refactored to use `docker/metadata-action@v5`, build **multi-arch** (`linux/amd64` + `linux/arm64`), and emit a richer tag set: `:latest` only on Release Please tags (`signaling-server-v*` / `mobile-v*`), `:vX.Y.Z` / `:vX.Y` / `:vX` (post-1.0), `:dev` (alias `:edge`) on every `main` push, `:sha-<short>` for traceability. Replaces the previous "every push → `:latest`" behaviour (no users affected — no release has shipped yet). New consumer-facing drop-in [`docker/stack/compose.ghcr.yaml`](docker/stack/compose.ghcr.yaml) + [`.env.ghcr.example`](docker/stack/.env.ghcr.example) lets self-hosters run the full stack (web + signaling) from GHCR without cloning the repo. Updated docs: [docker/stack/README.md](docker/stack/README.md) (now documents Mode A build-from-source vs Mode B pull-from-GHCR), [docs/contributing/signaling-server-docker.md](docs/contributing/signaling-server-docker.md), [docs/contributing/mobile-web-docker.md](docs/contributing/mobile-web-docker.md).
- Prettier: `prettier-plugin-svelte` for `.svelte` files; repo-wide formatting pass; CI job `test-and-build` runs `pnpm run format:check`; pre-commit Prettier hook extended to TS/JS/Svelte/CSS/HTML (file-name pattern).
- Security: `pnpm.overrides` for `cookie` (GHSA-pxg6-pf52-xh8x); CI runs `pnpm audit --audit-level=high`; signaling server hardening (HTTP security headers, WebSocket `maxPayload` / no `perMessageDeflate`, optional `SIGNALING_ALLOWED_ORIGINS`, `SIGNALING_HELLO_TIMEOUT_MS`, reject duplicate `hello`); WebRTC client validates signaling payloads before use.
- WebRTC default STUN: EU public servers (sipgate.de, Linphone) instead of Google; feature doc `STATUS: IDEA` for signaling rate limits; docs: Traefik TLS/domain/labels (`signaling-traefik.md` (since folded into `docker/public-server/README.md`)), example [compose.traefik.example.yaml](docker/signaling-server/compose.traefik.example.yaml); rolling role review checklist [review-signaling-mobile-docker-ci.md](docs/reviews/review-signaling-mobile-docker-ci.md).

<!-- Example sections:
### Changed
### Deprecated
### Removed
### Fixed
### Security
-->
