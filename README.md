# GuardedSleep

**Product name:** **GuardedSleep – Privacy Baby Monitor**

An open-source, privacy-first baby monitor application that turns your existing devices into a secure monitoring system.

> [!WARNING]
> **Work in progress: not ready for use.** GuardedSleep is in early alpha development. There is no release, nothing here has been tested on real families, and protocols, data formats and commands change without notice. **Do not rely on it to monitor a child.** Feedback and issues are welcome; pull requests are best discussed in an issue first.

## Project Status: Alpha Build-Out

Alpha build-out: a sans-IO Rust core, a native Android client and a web client on the same core, plus a self-hosted signaling server ([ADR-015](openspec/changes/archive/2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md), [ADR-019](openspec/changes/archive/2026-10-20-adr-019-language-repository-and-client-policy/design.md)). **The toolchain is in transition (ADR-020):** `just` becomes the single entry point (`setup fmt lint test gen build e2e gate ci`), and commands in the docs may change until the three implementation waves land ([ADR-020](openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md), [ADR-021](openspec/changes/archive/2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md)). The web client is `apps/web` (Vite + Svelte 5); the signaling server is the Rust `crates/guardedsleep-server`.

## Core Principles

- **Privacy First** -- End-to-end encryption, no cloud dependency, your data stays yours
- **Cross-Platform** -- Works on Android, iOS, tablets; optionally desktop (Windows, Linux, macOS)
- **Open Source** -- Community-driven development, transparent and auditable
- **User-Friendly** -- Usable by non-technical people, easy setup and daily operation

## How It Works

One device acts as the **Baby Unit** (placed near the child), another as the **Parent Unit** (carried by the parent). The baby unit monitors audio, detects noise events, and alerts the parent unit when thresholds are exceeded. Data transmission is minimized to save bandwidth -- audio streams only when needed.

## Documentation

- [Specs (`openspec/specs/`)](openspec/specs/) -- what GuardedSleep does today, per capability
- [Prepared changes (`openspec/changes/`)](openspec/changes/) and the [backlog](docs/backlog.md) -- decided work and ideas
- [Decision archive](openspec/changes/archive/README.md) -- history, including the former ADRs
- [Vision and Goals](docs/concept/vision.md)
- [Initial Ideas and Brainstorm](docs/concept/initial-ideas.md)
- [Technical Analysis](docs/technical/analysis.md)
- [Development Roles](docs/roles/README.md)
- [Contributing](CONTRIBUTING.md) — setup, hooks, CI, changelog expectations
- **Self-hosted stack (Docker):** [docker/stack/README.md](docker/stack/README.md) — local build (`compose.yaml`) or pull from GHCR (`compose.ghcr.yaml`); image tags and CI: [ADR-008](openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md)

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)** and **[docs/contributing/development.md](docs/contributing/development.md)** for Dev Container usage, local tool setup (mise, `just`; transitioning from `pnpm`/`pre-commit`), and how to run the same checks as GitHub Actions. For release/changelog automation plans, see **[docs/contributing/release-and-changelog.md](docs/contributing/release-and-changelog.md)**.

If you want to contribute ideas or review proposals, start with the [Vision](docs/concept/vision.md) and the [Feature Overview](docs/backlog.md).

## License

**GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later).** See [`LICENSE`](LICENSE). Product name and branding: [`docs/legal/trademark.md`](docs/legal/trademark.md). Decision: [ADR-014](openspec/changes/archive/2026-07-07-adr-014-open-source-license-agpl-and-trademark/design.md).
