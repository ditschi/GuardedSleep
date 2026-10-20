# GuardedSleep

**Product name:** **GuardedSleep – Privacy Baby Monitor**

An open-source, privacy-first baby monitor application that turns your existing devices into a secure monitoring system.

## Project Status: Concept Phase

We are currently defining requirements, evaluating technical approaches, and making architectural decisions. Application code is planned next; the repo already includes **documentation** and **developer tooling** (formatting, spelling, commit conventions, CI).

## Core Principles

- **Privacy First** -- End-to-end encryption, no cloud dependency, your data stays yours
- **Cross-Platform** -- Works on Android, iOS, tablets; optionally desktop (Windows, Linux, macOS)
- **Open Source** -- Community-driven development, transparent and auditable
- **User-Friendly** -- Usable by non-technical people, easy setup and daily operation

## How It Works

One device acts as the **Baby Unit** (placed near the child), another as the **Parent Unit** (carried by the parent). The baby unit monitors audio, detects noise events, and alerts the parent unit when thresholds are exceeded. Data transmission is minimized to save bandwidth -- audio streams only when needed.

## Documentation

- [Architecture Decision Records (ADRs)](docs/decisions/README.md) -- authoritative MVP decisions
- [Vision and Goals](docs/concept/vision.md)
- [Initial Ideas and Brainstorm](docs/concept/initial-ideas.md)
- [Feature Overview](docs/features/README.md)
- [Technical Analysis](docs/technical/analysis.md)
- [Architecture Decisions](docs/decisions/README.md)
- [Development Roles](docs/roles/README.md)
- [Contributing](CONTRIBUTING.md) — setup, hooks, CI, changelog expectations
- **Self-hosted stack (Docker):** [docker/stack/README.md](docker/stack/README.md) — local build (`compose.yaml`) or pull from GHCR (`compose.ghcr.yaml`); image tags and CI: [ADR-008](docs/decisions/ADR-008-container-image-build-and-distribution.md)

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)** and **[docs/contributing/development.md](docs/contributing/development.md)** for Dev Container usage, local `pnpm`/`pre-commit` setup, and how to run the same checks as GitHub Actions. For release/changelog automation plans, see **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.

If you want to contribute ideas or review proposals, start with the [Vision](docs/concept/vision.md) and the [Feature Overview](docs/features/README.md).

## License

**GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later).** See [`LICENSE`](LICENSE). Product name and branding: [`docs/legal/trademark.md`](docs/legal/trademark.md). Decision: [ADR-014](docs/decisions/ADR-014-open-source-license-agpl-and-trademark.md).
