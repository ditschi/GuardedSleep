# Proposal

## Why

The owner tests on real phones, but the only APK a `main` push produces is the `guardedsleep-alpha-debug-apk` workflow artifact of `android-native`: it needs a GitHub login, comes zipped, has no stable link and expires after 14 days. Every CI run also signs it with a fresh throwaway debug key, so a new build cannot be installed over the previous one; uninstalling deletes the pairing. GitHub Packages cannot hold an APK. Tooling only (no product behaviour, no spec change); it does not change architecture, scope or the security model. Tier T0.

## What Changes

- A new `dev-apk` job in `android-native.yml` runs after the emulator gate passes on a push to `main` (and on a manual dispatch from `main`). It takes the APK the gate tested, re-signs it with a fixed **dev key**, and replaces the asset of one rolling GitHub pre-release with the tag `android-dev`, which it moves to the tested commit.
- The dev key lives only in the GitHub environment `dev-apk` (deployments limited to `main`). The owner generates it offline; it is never committed and is separate from the alpha release key of `add-android-release`.
- The pre-release notes name the commit, the date, that it is an unreviewed debuggable test build, and the SHA-256 of the APK.
- `docs/ci-gates.md` gets a row for the job; `docs/contributing/` says where the dev APK is and how to install it.
- Out of scope: release signing, Play upload, version bumps, Release Please (all `add-android-release` or T1), a web dev build, nightly or PR publishing.

Persona impact (`docs/concept/personas.md`): serves the owner and alpha testers who install builds on their own devices; no end-user persona is affected, because the pre-release is marked as a test build and is not a release channel.

## Capabilities

### New Capabilities

### Modified Capabilities

## Impact

`.github/workflows/android-native.yml` (new job, `contents: write` on that job only), `docs/ci-gates.md`, `docs/contributing/development.md` (or the Android section that already describes local installs), repository settings: environment `dev-apk` with three secrets (owner task). Coordinate with `add-android-release`: the dev APK and a later Play or release APK share `org.guardedsleep.app.alpha` but not the signing key, so they cannot be installed over each other.
