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

| ADR                                                                 | Title                                                                                       | Status   | Date       |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | -------- | ---------- |
| [ADR-001](ADR-001-mvp-scope-poc-gate-and-authentication.md)         | MVP scope, PoC gate, no in-app App-Lock (OS lock)                                           | DECIDED  | 2026-04-15 |
| [ADR-002](ADR-002-client-platform-and-repository-structure.md)      | Client platform, UI stack, repo layout, signaling server stack                              | DECIDED  | 2026-04-15 |
| [ADR-003](ADR-003-connectivity-webrtc-signaling-and-qr-pairing.md)  | WebRTC, three paths, signaling, QR pairing, heartbeat                                       | DECIDED  | 2026-04-15 |
| [ADR-004](ADR-004-encryption-transport-and-key-storage.md)          | DTLS-SRTP, fingerprint binding, keystore, Layer 3 deferred                                  | DECIDED  | 2026-04-15 |
| [ADR-005](ADR-005-parent-alerts-product-naming-and-distribution.md) | Parent alerts (child audio), GuardedSleep naming, stores                                    | DECIDED  | 2026-04-15 |
| [ADR-006](ADR-006-release-and-changelog-automation.md)              | Release Please for SemVer + CHANGELOG automation                                            | DECIDED  | 2026-05-13 |
| [ADR-007](ADR-007-browser-e2e-test-strategy.md)                     | Browser E2E tests (Playwright, dev-only hooks, CI job)                                      | DECIDED  | 2026-05-13 |
| [ADR-008](ADR-008-container-image-build-and-distribution.md)        | GHCR images: multi-arch, `:latest` / `:dev` tag strategy                                    | DECIDED  | 2026-05-13 |
| [ADR-009](ADR-009-personas-and-failure-model.md)                    | Personas, use cases, parent-side failure model (no baby-side alarm)                         | PROPOSED | 2026-05-13 |
| [ADR-010](ADR-010-baby-unit-hardware-requirements.md)               | Baby-unit minimum hardware, self-test, battery safety                                       | PROPOSED | 2026-05-13 |
| [ADR-011](ADR-011-audio-routing-parent-side.md)                     | Parent-side alert audio routing + call interruption                                         | PROPOSED | 2026-05-13 |
| [ADR-012](ADR-012-connection-model-offline-lan-and-handoff.md)      | Connection model: offline-LAN signaling, path selection, network handoff (precises ADR-003) | PROPOSED | 2026-05-13 |
| [ADR-013](ADR-013-connection-status-observability-and-copy.md)      | Connection-status ambient bar, escalation timing, symmetric copy on both devices            | PROPOSED | 2026-05-13 |

## Discarded Options Tracking

Every ADR preserves rejected options in its "Options Considered" section. This is intentional and important:

- Rejected options include full Pros/Cons analysis
- The reason for rejection is documented in the "Decision" section
- If context changes later, we can revisit rejected options without re-analyzing from scratch
- Searching across ADRs gives a full history of what was considered and why
