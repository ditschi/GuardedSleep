# Contributing

Thank you for helping improve **GuardedSleep**. This repository combines product documentation, ADRs, and (soon) application code in one place.

## Where to start

- Read the [Vision](docs/concept/vision.md) and [Feature overview](docs/features/README.md).
- Architecture decisions live under [docs/decisions/](docs/decisions/README.md).

## Development setup

See **[docs/contributing/development.md](docs/contributing/development.md)** for:

- Dev Container (recommended) and local tooling
- Docker / Compose: signaling **[docs/contributing/signaling-server-docker.md](docs/contributing/signaling-server-docker.md)**; static PWA + full stack **[docs/contributing/mobile-web-docker.md](docs/contributing/mobile-web-docker.md)** and **[docker/stack/README.md](docker/stack/README.md)** (includes **GHCR** pull mode `compose.ghcr.yaml`, [ADR-008](docs/decisions/ADR-008-container-image-build-and-distribution.md)); Traefik / TLS: **[docs/contributing/signaling-traefik.md](docs/contributing/signaling-traefik.md)**; Android debug APK in CI: **[docs/contributing/android-apk-build.md](docs/contributing/android-apk-build.md#ci-github-actions)** (workflow [`.github/workflows/android.yml`](.github/workflows/android.yml))
- Role-based review templates: **[docs/reviews/](docs/reviews/README.md)**
- Installing tools: **in transition (ADR-020)** to mise + rustup, with `just setup`; until wave 1 lands use `pnpm install --frozen-lockfile`
- Running the same checks as CI: target `just ci`; today `pnpm run lint` (typos + CSpell for locale JSON), `pnpm run format:check`, `scripts/core-build.sh test`
- Git hooks: today Husky (do **not** run `pre-commit install`); lefthook replaces Husky, pre-commit and commitlint in wave 1
- After wave 1: tasks needing Docker or KVM print `SKIPPED` locally; CI runs with `GS_STRICT=1`, which fails on skips
- Tool pins, Renovate, and the gate list: [ADR-020](docs/decisions/ADR-020-toolchain-and-development-setup.md)

## Commits and pull requests

- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced on the PR title in CI; with squash merges only the title reaches `main`; local commitlint is removed in wave 1).
- **Pull request titles** must use the same convention; `main` allows squash merges only, requires the `ci-ok` check and linear history, and requires no reviews.
- For user-visible changes, update **[CHANGELOG.md](CHANGELOG.md)** under `## [Unreleased]` (see [Release notes and changelog](docs/contributing/release-and-changelog.md)).

## Changelog and releases

Long-term automation options are described in **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.
