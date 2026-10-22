# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for the GuardedSleep project. ADRs document significant technical and design decisions along with their context, the options considered (including rejected ones), and the consequences of the chosen approach.

## Why ADRs?

- Decisions are documented transparently for the community
- Rejected alternatives are preserved -- they provide context for why we chose what we chose
- New contributors can understand the reasoning behind the architecture
- Decisions can be revisited when context changes (marked as SUPERSEDED)

## ADR Format

Each ADR follows this template:

```markdown
# ADR-NNN: [Title]

- Status: PROPOSED | DECIDED | SUPERSEDED
- Date: YYYY-MM-DD
- Deciders: [who was involved in the decision]
- Supersedes: ADR-XXX (if applicable)
- Superseded by: ADR-YYY (if applicable)

## Context

Why this decision is needed. What problem are we solving?

## Options Considered

### Option A: [Name]

[Description]

- **Pros**: ...
- **Cons**: ...

### Option B: [Name]

[Description]

- **Pros**: ...
- **Cons**: ...

## Decision

Which option was chosen and the reasoning behind it.

## Consequences

### Positive

- What improvements or benefits follow from this decision

### Negative

- What trade-offs or limitations we accept

### Risks

- What could go wrong, and how we mitigate it
```

## Naming Convention

ADR files are named: `ADR-NNN-short-title.md`

Example: `ADR-001-app-platform-choice.md`

## Index

| ADR                                                                                                             | Title                                                                                       | Status   | Date       |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | -------- | ---------- |
| [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)    | MVP scope, PoC gate, no in-app App-Lock (OS lock)                                           | ARCHIVED | 2026-04-15 |
| [ADR-002](ADR-002-client-platform-and-repository-structure.md)                                                  | Client platform, UI stack, repo layout, signaling server stack                              | DECIDED  | 2026-04-15 |
| [ADR-003](ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md)                                              | WebRTC, three paths, signaling, QR pairing, heartbeat                                       | DECIDED  | 2026-04-15 |
| [ADR-004](../../openspec/changes/archive/2026-04-15-adr-004-encryption-transport-and-key-storage/design.md)     | DTLS-SRTP, fingerprint binding, keystore, Layer 3 deferred                                  | ARCHIVED | 2026-04-15 |
| [ADR-005](ADR-005-parent-alerts-product-naming-and-distribution.md)                                             | Parent alerts (child audio), GuardedSleep naming, stores                                    | DECIDED  | 2026-04-15 |
| [ADR-006](ADR-006-release-and-changelog-automation.md)                                                          | Release Please for SemVer + CHANGELOG automation                                            | DECIDED  | 2026-05-13 |
| [ADR-007](ADR-007-browser-e2e-test-strategy.md)                                                                 | Browser E2E tests (Playwright, dev-only hooks, CI job)                                      | DECIDED  | 2026-05-13 |
| [ADR-008](ADR-008-container-image-build-and-distribution.md)                                                    | GHCR images: multi-arch, `:latest` / `:dev` tag strategy                                    | DECIDED  | 2026-05-13 |
| [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md)               | Personas, use cases, parent-side failure model (no baby-side alarm)                         | ARCHIVED | 2026-05-13 |
| [ADR-010](../../openspec/changes/archive/2026-10-21-adr-010-baby-unit-hardware-requirements/design.md)          | Baby-unit self-tests instead of hardware minimums, battery safety                           | ARCHIVED | 2026-05-13 |
| [ADR-011](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md)                | Parent-side alert audio routing + call interruption                                         | ARCHIVED | 2026-05-13 |
| [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md)                                                  | Connection model: offline-LAN signaling, path selection, network handoff (precises ADR-003) | DECIDED  | 2026-05-13 |
| [ADR-013](../../openspec/changes/archive/2026-10-21-adr-013-connection-status-observability-and-copy/design.md) | Connection-status ambient bar, escalation timing, symmetric copy on both devices            | ARCHIVED | 2026-05-13 |
| [ADR-014](ADR-014-open-source-license-agpl-and-trademark.md)                                                    | Open-source license: AGPL-3.0-or-later + trademark policy                                   | DECIDED  | 2026-07-07 |
| [ADR-015](ADR-015-native-clients-with-shared-rust-core.md)                                                      | Native clients (Compose / SwiftUI / Svelte) with shared Rust core; PoC gate incl. Maestro   | DECIDED  | 2026-10-19 |
| [ADR-016](ADR-016-test-strategy-local-virtual-physical.md)                                                      | Test strategy: local → virtual → physical, device-agnostic runner, optional cloud farm      | DECIDED  | 2026-10-19 |
| [ADR-017](../../openspec/changes/archive/2026-10-20-adr-017-link-recovery-and-connection-supervision/design.md) | Link recovery: core `Link` supervisor, new attempt per recovery, server liveness            | ARCHIVED | 2026-10-20 |
| [ADR-018](ADR-018-alpha-milestone-scope-and-trust-features.md)                                                  | Alpha milestone scope, trust features, child-side silence (DND) guarantee                   | DECIDED  | 2026-10-20 |
| [ADR-019](ADR-019-language-repository-and-client-policy.md)                                                     | Language by layer, Rust protocol source, polyglot monorepo, permanent full web client       | DECIDED  | 2026-10-20 |
| [ADR-020](ADR-020-toolchain-and-development-setup.md)                                                           | Toolchain and development setup: just, mise, lefthook, Rust server, Vite web, CI tiers      | DECIDED  | 2026-10-20 |
| [ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md)                                           | Offline LAN topologies, locale policy (about 20 AI-translated), distribution channels       | DECIDED  | 2026-10-20 |

Partial supersession is recorded as a status note under each ADR's header block and as an inline "(superseded, see ADR-0xx)" mark on the affected sentence or row. The `Status` stays unchanged unless the whole ADR is superseded. ADR-020 and ADR-021 partly supersede ADR-001, 002, 005, 006, 007, 008, 012, 014, 015, 016, 018 and 019 (see their notes); the original text is kept as decision history.

## Discarded Options Tracking

Every ADR preserves rejected options in its "Options Considered" section. This is intentional and important:

- Rejected options include full Pros/Cons analysis
- The reason for rejection is documented in the "Decision" section
- If context changes later, we can revisit rejected options without re-analyzing from scratch
- Searching across ADRs gives a full history of what was considered and why
