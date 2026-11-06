# Decision archive

Everything in this folder is history. The current behaviour is in [`openspec/specs/`](../../specs/), work in progress in [`openspec/changes/`](../), and ideas, deferred and rejected items in [`docs/backlog.md`](../../../docs/backlog.md).

Each archived change has a `proposal.md` (why and what) and, where the work was designed, a `design.md` with the options considered and the rejected alternatives. New decisions are made in OpenSpec changes (`/opsx:propose`) and land here through `/opsx:archive`.

## Architecture Decision Records (ADR-001 … ADR-021)

Until 2026-10-22 decisions were ADRs in `docs/decisions/`. Each ADR was moved here verbatim as `<decision date>-adr-NNN-<slug>/design.md`, so references such as "ADR-017 §6" in code comments still resolve. Its outcome was migrated into the specs, the prepared changes or the backlog; the `proposal.md` next to it says where.

| ADR                                                                                     | Title                                                                                       | Date       |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------- |
| [ADR-001](2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)           | MVP scope, PoC gate, no in-app App-Lock (OS lock)                                           | 2026-04-15 |
| [ADR-002](2026-04-15-adr-002-client-platform-and-repository-structure/design.md)        | Client platform, UI stack, repo layout, signaling server stack                              | 2026-04-15 |
| [ADR-003](2026-04-15-adr-003-connectivity-webrtc-signaling-and-qr-pairing/design.md)    | WebRTC, three paths, signaling, QR pairing, heartbeat                                       | 2026-04-15 |
| [ADR-004](2026-04-15-adr-004-encryption-transport-and-key-storage/design.md)            | DTLS-SRTP, fingerprint binding, keystore, Layer 3 deferred                                  | 2026-04-15 |
| [ADR-005](2026-04-15-adr-005-parent-alerts-product-naming-and-distribution/design.md)   | Parent alerts (child audio), GuardedSleep naming, stores                                    | 2026-04-15 |
| [ADR-006](2026-05-13-adr-006-release-and-changelog-automation/design.md)                | Release Please for SemVer + CHANGELOG automation                                            | 2026-05-13 |
| [ADR-007](2026-05-13-adr-007-browser-e2e-test-strategy/design.md)                       | Browser E2E tests (Playwright, dev-only hooks, CI job)                                      | 2026-05-13 |
| [ADR-008](2026-05-13-adr-008-container-image-build-and-distribution/design.md)          | GHCR images: multi-arch, `:latest` / `:dev` tag strategy                                    | 2026-05-13 |
| [ADR-009](2026-10-21-adr-009-personas-and-failure-model/design.md)                      | Personas, use cases, parent-side failure model (no baby-side alarm)                         | 2026-05-13 |
| [ADR-010](2026-10-21-adr-010-baby-unit-hardware-requirements/design.md)                 | Baby-unit self-tests instead of hardware minimums, battery safety                           | 2026-05-13 |
| [ADR-011](2026-10-21-adr-011-audio-routing-parent-side/design.md)                       | Parent-side alert audio routing + call interruption                                         | 2026-05-13 |
| [ADR-012](2026-10-21-adr-012-connection-model-offline-lan-and-handoff/design.md)        | Connection model: offline-LAN signaling, path selection, network handoff (precises ADR-003) | 2026-05-13 |
| [ADR-013](2026-10-21-adr-013-connection-status-observability-and-copy/design.md)        | Connection-status ambient bar, escalation timing, symmetric copy on both devices            | 2026-05-13 |
| [ADR-014](2026-07-07-adr-014-open-source-license-agpl-and-trademark/design.md)          | Open-source license: AGPL-3.0-or-later + trademark policy                                   | 2026-07-07 |
| [ADR-015](2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md)            | Native clients (Compose / SwiftUI / Svelte) with shared Rust core; PoC gate incl. Maestro   | 2026-10-19 |
| [ADR-016](2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md)            | Test strategy: local → virtual → physical, device-agnostic runner, optional cloud farm      | 2026-10-19 |
| [ADR-017](2026-10-20-adr-017-link-recovery-and-connection-supervision/design.md)        | Link recovery: core `Link` supervisor, new attempt per recovery, server liveness            | 2026-10-20 |
| [ADR-018](2026-10-20-adr-018-alpha-milestone-scope-and-trust-features/design.md)        | Alpha milestone scope, trust features, child-side silence (DND) guarantee                   | 2026-10-20 |
| [ADR-019](2026-10-20-adr-019-language-repository-and-client-policy/design.md)           | Language by layer, Rust protocol source, polyglot monorepo, permanent full web client       | 2026-10-20 |
| [ADR-020](2026-10-20-adr-020-toolchain-and-development-setup/design.md)                 | Toolchain and development setup: just, mise, lefthook, Rust server, Vite web, CI tiers      | 2026-10-20 |
| [ADR-021](2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) | Offline LAN topologies, locale policy (about 20 AI-translated), distribution channels       | 2026-10-20 |

## Migration changes

The ADR → OpenSpec migration, one change per capability:

- [`2026-10-22-migrate-audio-monitoring`](2026-10-22-migrate-audio-monitoring/proposal.md)
- [`2026-10-22-migrate-connection-status`](2026-10-22-migrate-connection-status/proposal.md)
- [`2026-10-22-migrate-dev-toolchain`](2026-10-22-migrate-dev-toolchain/proposal.md)
- [`2026-10-22-migrate-device-setup`](2026-10-22-migrate-device-setup/proposal.md)
- [`2026-10-22-migrate-distribution`](2026-10-22-migrate-distribution/proposal.md)
- [`2026-10-22-migrate-i18n`](2026-10-22-migrate-i18n/proposal.md)
- [`2026-10-22-migrate-pairing`](2026-10-22-migrate-pairing/proposal.md)
- [`2026-10-22-migrate-parent-alerts`](2026-10-22-migrate-parent-alerts/proposal.md)
- [`2026-10-22-migrate-release-checks`](2026-10-22-migrate-release-checks/proposal.md)
- [`2026-10-22-migrate-signaling-relay`](2026-10-22-migrate-signaling-relay/proposal.md)

## Context notes

- **ADR-018 (2026-10-23):** the alpha tester setup changes with `alpha-connection-model` and `add-offline-lan`. Revised 2026-11-02: internet signaling goes through the project server (`gs.mydata.rocks`) and own relays in parallel, with the project TURN on the same host and a disclosed off switch; the public Nostr relays of the first revision (2026-10-23) were dropped, and offline LAN stays in the alpha. Mobile-data rounds therefore run through the project server and its TURN; a pair fails visibly with the `server_unreachable` or `media_blocked` troubleshooter only when the server or every path is unreachable.

## Withdrawn changes

Prepared but never implemented; kept for their options and discarded alternatives.

- [`2026-10-23-add-stun-server-choice`](2026-10-23-add-stun-server-choice/proposal.md): superseded by `alpha-connection-model` (automatic, disclosed EU STUN).
