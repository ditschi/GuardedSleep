# Contributing

Thank you for helping improve **GuardedSleep**. This repository combines the product specification (OpenSpec), documentation and application code in one place.

## Where to start

- Read the [Vision](docs/concept/vision.md), the current behaviour in [`openspec/specs/`](openspec/specs/) and the [backlog](docs/backlog.md).
- Changes to behaviour, scope or architecture start as an OpenSpec change (`/opsx:propose`, see [AGENTS.md §6](AGENTS.md)); past decisions, including the former ADRs, are in the [decision archive](openspec/changes/archive/README.md).

## Development setup

See **[docs/contributing/development.md](docs/contributing/development.md)** for:

- Dev Container (recommended) and local tooling
- Docker / Compose: signaling **[docs/contributing/signaling-server-docker.md](docs/contributing/signaling-server-docker.md)**; static PWA + full stack **[docs/contributing/web-docker.md](docs/contributing/web-docker.md)** and **[docker/stack/README.md](docker/stack/README.md)** (includes **GHCR** pull mode `compose.ghcr.yaml`, [ADR-008](openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md)); Traefik / TLS: **[docs/contributing/signaling-traefik.md](docs/contributing/signaling-traefik.md)**; Android build and emulator gate in CI: [`.github/workflows/ci.yml`](.github/workflows/ci.yml) (`android` job) and [`.github/workflows/android-native.yml`](.github/workflows/android-native.yml)
- Role-based review templates: **[docs/reviews/](docs/reviews/README.md)**
- Installing tools: [mise](https://mise.jdx.dev/getting-started.html) plus rustup, then `just setup` (pins in `mise.toml`, `rust-toolchain.toml`)
- Running the same checks as CI: `just ci` (lint + test + build + e2e); single tasks are `just lint`, `just test`, `just build`, `just e2e` (`just --list`)
- Git hooks: lefthook (installed by `just setup`) runs Prettier, cargo fmt, typos, hygiene and a secret scan on staged files; CI re-runs everything
- Tasks needing Docker, KVM or the Android SDK print `SKIPPED` locally (a skip is not a pass); CI runs with `GS_STRICT=1`, which fails on skips
- Tool pins, Renovate, and the gate list: [ADR-020](openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md)

## Commits and pull requests

- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced on the PR title in CI; there is no local commit-msg hook). Commits that reach `main` by rebase follow it too.
- **Pull request titles** must use the same convention; a PR reaches `main` by **Rebase and merge** when its history is clean (linear, no merge commits, every commit Conventional and self-contained), otherwise by **Squash and merge**. `ci-ok` must be green; no reviews are required (AGENTS.md §8).
- For user-visible changes, update **[CHANGELOG.md](CHANGELOG.md)** under `## [Unreleased]` (see [Releases and changelog](docs/contributing/release-and-changelog.md)).

## Changelog and releases

How releases and the changelog work: **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.
