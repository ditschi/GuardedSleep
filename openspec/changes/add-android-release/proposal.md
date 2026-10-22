# Proposal

## Why

The alpha has no way to ship: no release tag, no release signing, no GitHub or Play release, and the release-APK inspection only checks test hooks. ADR-020 §10, §11 and ADR-021 §3 decide the T0 release path. Tier T0.

## What Changes

- A manual `android-v*` tag builds, signs and publishes the release (versionCode from `split-android-modules`).
- Alpha signing key in the protected `release` environment; the direct-distribution key is generated offline, used from beta, never stored in CI, with a written v3 rotation-lineage plan.
- Channels at alpha: Google Play testing tracks (`org.guardedsleep.app.alpha`) and a GitHub Releases APK.
- Release-APK assertions extended: no tracker classes, the expected permission set, backup exclusion, `debuggable=false`.
- Store texts: shortened title where needed, searchable terms, "F-Droid clients via IzzyOnDroid; F-Droid main repository later".
- Out of scope (T1/beta): Release Please, fastlane metadata, SBOM, provenance, cosign, reproducibility check, IzzyOnDroid.

## Capabilities

### New Capabilities

### Modified Capabilities

- `distribution`: release tags, signing, channels, release-APK assertions, store wording.

## Impact

`.github/workflows` (release workflow, protected environment), `scripts/poc/release-check.sh`, store listing texts, `docs/contributing/release-and-changelog.md`, `docs/ci-gates.md`.
