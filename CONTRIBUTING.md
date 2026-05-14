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
- Installing dependencies (`pnpm`, Python `pre-commit`)
- Running the same checks as CI (`pnpm run lint` — includes typos + CSpell for locale JSON, `pnpm run format:check`)
- Git hooks (Husky: do **not** run `pre-commit install`; it conflicts with Husky)

## Commits and pull requests

- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced locally via Husky + **commitlint**, and on GitHub via PR title check for squash merges).
- **Pull request titles** should use the same convention when the merge strategy is **squash** (the squashed commit message is usually derived from the PR title).
- For user-visible changes, update **[CHANGELOG.md](CHANGELOG.md)** under `## [Unreleased]` (see [Release notes and changelog](docs/contributing/release-and-changelog.md)).

## Changelog and releases

Long-term automation options are described in **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.
