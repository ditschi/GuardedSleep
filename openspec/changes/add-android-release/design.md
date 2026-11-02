# Design

## Context

Sources: ADR-020 §10 (T0 releases and signing), §11 (release-APK assertions), ADR-021 §3 (channels, Play account, developer verification, store wording), ADR-005 (naming, store ASO), ADR-018 §1 (identity).

## Decisions

- **Manual tag at alpha, Release Please at T1** (ADR-020 §10).
- **The direct-distribution key never enters CI**: it cannot be rotated for existing installs, so it is generated offline now and a v3 rotation-lineage plan is written down.
- **Play account** (owner, D3): an organisation account if a D-U-N-S number is easy; otherwise the 14-day closed test with at least 15 testers is the beta. Developer verification is registered now (backlog `PLANNED`, owner task).
- **Discarded alternatives:** cocogitto or semantic-release (Release Please fits the monorepo components); storing the direct key in CI (unrecoverable if leaked); F-Droid main now (prebuilt libwebrtc and JNA, `DEFERRED`).

## Risks / Trade-offs

- Losing the offline key locks out direct-APK users from updates; the rotation-lineage plan and an offline backup procedure address it.
