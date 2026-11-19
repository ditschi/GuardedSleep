# Proposal

## Why

Testers get the gate-tested dev build from the rolling `android-dev` pre-release (`add-dev-apk-prerelease`), but they have to download and install every new build by hand. F-Droid clients such as Neo Store can update apps automatically, but they read F-Droid repositories, not GitHub releases, and decide on an update by `versionCode`. Every dev build today carries the same `versionCode` (100101, derived from `versionName=0.1.0-alpha.1`). The dev build also shares `org.guardedsleep.app.alpha` with the coming alpha release (`add-android-release`), which is signed with a different key: a tester with both would get a signature conflict and lose the pairing on reinstall.

This changes the product identity (a third application id) and reverses the discarded alternative "separate app id" of `add-dev-apk-prerelease`, so the Critical Reviewer role applies. It does not change the security model or the architecture. Tier T0.

## What Changes

- **Dev application id:** debug builds use `org.guardedsleep.app.dev`, with their own launcher label ("GuardedSleep Dev") and a visibly marked launcher icon (other background colour, and a badge that also shows in themed icons). They install next to alpha, Play and later release builds and never update over them.
- **Commit-derived dev version:** debug builds take `versionName = <base>-dev.<YYYYMMDDHHMMSS>+<sha8>` (or `<base>.dev.…` when the base already has a pre-release suffix, e.g. `0.1.0-alpha.1.dev.20261119143012+abc12345`) and `versionCode` = seconds from 2026-01-01T00:00:00Z to the **committer** time of `HEAD`. Every later `main` commit therefore yields a higher `versionCode`. Release builds keep the existing scheme from `version.properties`.
- **Dev F-Droid repository on GitHub Pages:** after publishing the pre-release, a separate `dev-repo` job builds a signed F-Droid repository with `fdroidserver` at `https://ditschi.github.io/GuardedSleep/fdroid/dev/repo`, keeps the last 3 dev APKs, and deploys it with `actions/deploy-pages` (no `gh-pages` branch). The index is signed with a **separate dev repo key** in its own `dev-repo` environment; the job that runs fdroidserver never holds the APK key or a write token.
- **Pre-release title** carries the full dev `versionName`, so testers see which build it holds. Obtainium is documented with the F-Droid repository as its source.
- **Docs:** the `fdroidrepos://` link with fingerprint, a QR code, setup steps for Neo Store and Obtainium, owner steps for the repo key and Pages, and the extended `dev-apk` row in `docs/ci-gates.md`.
- **No release collision:** separate application id, repository URL (`/fdroid/dev/`), repository key, APK signing key and tag (`android-dev` vs `android-v*`). A later stable repository (IzzyOnDroid at beta, or an own `/fdroid/repo`) uses other keys.
- **Out of scope:** a stable F-Droid repository, IzzyOnDroid, fastlane store metadata, reproducibility checks (T1); a different id for local debug builds; changing the restored brand icon (a separate change).

Persona impact (`docs/concept/personas.md`): serves the owner and alpha testers, who get updates without manual downloads. No end-user persona is affected: the dev channel is labelled as a test build, and release builds keep their identity and version scheme.

## Capabilities

### New Capabilities

### Modified Capabilities

- `distribution`: the product identity gains the dev application id; the derived version code applies to release builds, while dev builds derive theirs from the commit; a new requirement for the dev F-Droid repository.

## Impact

- `apps/android/build-logic` (`AndroidVersion.kt` and tests, `Conventions.kt`), `apps/android/app/build.gradle.kts`, `apps/android/app/src/debug/` (label and icon override), `locales/en.json` (dev launcher label).
- `scripts/gradle.sh` (passes the committer epoch), `scripts/poc/gate.sh`, `scripts/android-install.sh` (package id, `-d` for downgrades), `apps/android/maestro/*.yaml`, the debug receivers' KDoc and the docs that name the old package (`docs/contributing/{development,alpha-hardware-test,turn-server-vps}.md`).
- The release asset is renamed from `guardedsleep-alpha-dev.apk` to `guardedsleep-dev.apk`.
- `.github/workflows/android-native.yml`: `dev-apk` deletes the APK key right after signing and hands the APK on; new `dev-repo` job (environment `dev-repo`, `contents: read`, builds the repository) and `dev-pages` job (environment `github-pages`, `pages: write` and `id-token: write` on that job only).
- `tools/fdroid/requirements.in` and a hash-locked `requirements.txt` for fdroidserver (Renovate `pip-compile`).
- Docs: `docs/contributing/development.md`, `docs/ci-gates.md`, AGENTS.md §1 (names).
- Owner, **before the merge**: create the repository key and the `dev-repo` environment (`scripts/dev-repo-key.sh`), enable GitHub Pages (source: GitHub Actions) and allow `main` in the `github-pages` environment.
- New CI dependency: `fdroidserver` 2.4.5 with hash-locked dependencies, only in the `dev-repo` job, which holds neither the APK key nor a write token.
- Testers with the current `.alpha` dev build must install the new `.dev` app once and pair again.
