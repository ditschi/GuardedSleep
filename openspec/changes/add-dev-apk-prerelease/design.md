# Design

## 1. Which APK

**Option A — re-sign the APK the gate tested (recommended).** The `gate` job already uploads `app-debug.apk`; the `dev-apk` job downloads that artifact and re-signs it with `apksigner` (build-tools 36.0.0 from `./.github/actions/setup-android`).

- Pros: the published bytes are exactly what passed AC-1 ... AC-4 apart from the signature; no second build (~20 min saved); the signing secret never reaches the long gate job, which also runs on pull requests.
- Cons: `apksigner` must replace the v1/v2/v3 debug signature cleanly (it does: signing an already signed APK rewrites the signature block); a debug-signed artifact passes through one more job.

**Option B — sign in the gate job through a Gradle `signingConfig` read from environment variables.**

- Pros: one job; local builds could use the same key.
- Cons: the secret is exposed to the 2-hour gate job, and pull-request runs need a code path without it; changes the build script for a CI concern.

Recommendation: A.

## 2. Where it is published

A single pre-release with the tag `android-dev`, updated in place:

1. Move the tag to the tested commit: `gh api -X PATCH repos/{owner}/{repo}/git/refs/tags/android-dev -f sha=$GITHUB_SHA -F force=true` (create the ref and the release on the first run).
2. `gh release edit android-dev --prerelease --title "Dev build <short sha>" --notes-file notes.md`.
3. `gh release upload android-dev guardedsleep-alpha-dev.apk guardedsleep-alpha-dev.apk.sha256 --clobber`.

The tag pattern `android-v*` stays reserved for releases; `android-dev` matches no release trigger. `main` branch protection does not cover tags.

## 3. Key and secrets

Environment `dev-apk`, deployment branches: `main` only. Secrets:

| Secret                     | Content                                  |
| -------------------------- | ---------------------------------------- |
| `GS_DEV_KEYSTORE_B64`      | base64 of a PKCS12 keystore with one key |
| `GS_DEV_KEYSTORE_PASSWORD` | keystore and key password (PKCS12: one)  |
| `GS_DEV_KEY_ALIAS`         | key alias                                |

The keystore is decoded into `$RUNNER_TEMP` and deleted in an `always()` step. The owner creates it offline (`keytool -genkeypair -storetype PKCS12 -keyalg EC -groupname secp256r1 -validity 3650 ...`). If a secret is missing the job fails with a message naming it: a silent skip would leave a stale APK behind the stable link.

## 4. Safety of a public debug build

The debug APK is `debuggable=true` and contains the gate hooks (`DebugLaunchActivity`, `DebugInjectReceiver`). Both are protected by `android.permission.DUMP`, so only the adb shell can reach them; no other app on the device can inject audio. `debuggable` lets someone with USB debugging and physical access run code as the app; acceptable for a test build and stated in the release notes. The pairing key stays in the Android Keystore (AGENTS.md §7.2).

## 5. Job shape

`needs: gate`, `if: github.ref == 'refs/heads/main' && (github.event_name == 'push' || github.event_name == 'workflow_dispatch')`, `environment: dev-apk`, `permissions: { contents: write }`, `concurrency: { group: dev-apk, cancel-in-progress: false }` so two pushes publish in order. Steps: download artifact → setup-android → decode key → `apksigner sign` + `apksigner verify --print-certs` → write notes and SHA-256 → publish (§2) → remove key.

## Discarded alternatives

- **GitHub Packages:** no APK package type.
- **One release per commit:** clutters Releases and the tag list; the owner wants "the latest".
- **Committed dev keystore:** violates AGENTS.md §7.7.
- **Separate app id (`org.guardedsleep.app.dev`):** would let the dev and Play builds coexist, but needs a rebuild instead of re-signing the tested APK (§1) and changes the gate's package name; revisit in `add-android-release`.
