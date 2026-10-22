# Proposal

## Why

Self-hosters need to know which images exist and how they are tagged, and the product needs one identity across stores and images. ADR-005, ADR-006, ADR-008, ADR-020 §10 and ADR-021 §3 decide channels and release tooling, but only the container images and the app identity are built today. This change records the built behaviour as the `distribution` spec (tier T0, docs only).

## What Changes

- New capability spec `distribution`: product identity, the two GHCR images with their build and tag rules, and the Compose stacks for self-hosting.
- Decided but not built (manual `android-v*` tag with derived versionCode, alpha signing, GitHub Releases APK, Play testing tracks) becomes the prepared change `add-android-release`. T1 items (Release Please with `app`/`server`/`core`, IzzyOnDroid, fastlane metadata, SBOM, provenance, cosign, reproducibility check) are `PLANNED` in the backlog; F-Droid main stays `DEFERRED`.
- ADR-005, ADR-006 and ADR-008 are archived; ADR-021 is archived once `add-offline-lan` is prepared.

## Capabilities

### New Capabilities

- `distribution`: how GuardedSleep is named, packaged and published, today as container images for self-hosting.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/distribution/`, ADR-005, ADR-006 and ADR-008 (archived), notes in ADR-020 and ADR-021, `docs/backlog.md`.
