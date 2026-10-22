# Contributing

Thank you for helping improve **GuardedSleep**. This repository combines product documentation, ADRs, and (soon) application code in one place.

## Where to start

- Read the [Vision](docs/concept/vision.md) and [Feature overview](docs/features/README.md).
- Architecture decisions live under [docs/decisions/](docs/decisions/README.md).

## Development setup

See **[docs/contributing/development.md](docs/contributing/development.md)** for:

- Dev Container (recommended) and local tooling
- Docker / Compose: signaling **[docs/contributing/signaling-server-docker.md](docs/contributing/signaling-server-docker.md)**; static PWA + full stack **[docs/contributing/mobile-web-docker.md](docs/contributing/mobile-web-docker.md)** and **[docker/stack/README.md](docker/stack/README.md)** (includes **GHCR** pull mode `compose.ghcr.yaml`, [ADR-008](openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md)); Traefik / TLS: **[docs/contributing/signaling-traefik.md](docs/contributing/signaling-traefik.md)**; Android build and emulator gate in CI: [`.github/workflows/ci.yml`](.github/workflows/ci.yml) (`android` job) and [`.github/workflows/android-native.yml`](.github/workflows/android-native.yml)
- Role-based review templates: **[docs/reviews/](docs/reviews/README.md)**
- Installing tools: [mise](https://mise.jdx.dev/getting-started.html) plus rustup, then `just setup` (pins in `mise.toml`, `rust-toolchain.toml`)
- Running the same checks as CI: `just ci` (lint + test + build + e2e); single tasks are `just lint`, `just test`, `just build`, `just e2e` (`just --list`)
- Git hooks: lefthook (installed by `just setup`) runs Prettier, cargo fmt, typos, hygiene and a secret scan on staged files; CI re-runs everything
- Tasks needing Docker, KVM or the Android SDK print `SKIPPED` locally (a skip is not a pass); CI runs with `GS_STRICT=1`, which fails on skips
- Tool pins, Renovate, and the gate list: [ADR-020](docs/decisions/ADR-020-toolchain-and-development-setup.md)

## Commits and pull requests

- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced on the PR title in CI; with squash merges only the title reaches `main`; there is no local commit-msg hook).
- **Pull request titles** must use the same convention; `main` allows squash merges only, requires the `ci-ok` check and linear history, and requires no reviews.
- For user-visible changes, update **[CHANGELOG.md](CHANGELOG.md)** under `## [Unreleased]` (see [Release notes and changelog](docs/contributing/release-and-changelog.md)).

## Changelog and releases

Long-term automation options are described in **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.
