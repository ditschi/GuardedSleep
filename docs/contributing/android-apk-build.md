# Android APK (Capacitor)

The same web app in `apps/mobile` is wrapped with **Capacitor** for Android. Distribution for end users is an **APK** or **AAB** (Play Store), built with the Android Gradle project under `apps/mobile/android/`.

## Debug vs release (what is a “real” APK?)

|                     | **Debug** (`assembleDebug`)                                        | **Release** (`assembleRelease` / `bundleRelease`)                                                                                                                |
| ------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Signing**         | Automatic **debug** keystore (created by the Android tooling).     | **Your** release keystore (or Play App Signing after upload).                                                                                                    |
| **Optimisation**    | Larger, debuggable; intended for development and sideload testing. | Shrunk / minified (per your Gradle settings); intended for end users and stores.                                                                                 |
| **CI in this repo** | Yes — no secrets required; proves the native project still builds. | Not in GitHub Actions — would require storing signing material in **GitHub Secrets** and wiring `signingConfigs` in Gradle (out of scope until you choose that). |

A **debug APK is still a real, installable APK** you can copy to a phone (`adb install` or “Open with package installer”). It is **not** “fake”; it is **not** suitable as a Play Store listing without going through the release/signing flow.

**“Normal” / production build** in Android terms means the **`release` build type**, which you get with `./gradlew assembleRelease` (APK) or `./gradlew bundleRelease` (AAB for Play). This project does **not** yet define `signingConfigs` in Gradle — you must add them (or use Android Studio **Build → Generate Signed App Bundle / APK**) before `assembleRelease` will produce a signed artifact. See [Release build](#release-build) below.

## Where to build locally (two setups)

### 1. Dev Container (recommended — one image)

The **Dev Container** (`.devcontainer/`) uses the **TypeScript/Node dev image** as base and adds **Eclipse Temurin JDK 21** plus a minimal **Android SDK** (platform 35, build-tools 35.0.0). Same container runs `pnpm`, `cap sync`, and `./gradlew`.

- Config: [`.devcontainer/devcontainer.json`](../../.devcontainer/devcontainer.json), [`.devcontainer/Dockerfile`](../../.devcontainer/Dockerfile)
- Also described in [development.md](development.md) (Capacitor / Android section)

You **do not** need `docker/android-build` unless you prefer a slimmer JDK-only image (e.g. broken Java on the host).

### 2. `docker/android-build` (optional — different base image)

[`docker/android-build/`](../../docker/android-build/) is a **separate** image based on **`eclipse-temurin:21-jdk-jammy`** with the Android SDK. It contains **no Node/pnpm** — you run `pnpm … build` and `cap sync` **on the host** or inside the Dev Container, then run Gradle inside this Compose service.

Use it when the host has only a JRE, wrong Java version, or no SDK — see [docker/android-build/README.md](../../docker/android-build/README.md).

## Prerequisites

- **JDK** (this project targets **Java 21** for Capacitor/Android modules — use a **full JDK**, not a headless JRE; on Debian/Ubuntu e.g. `openjdk-21-jdk` where available, or Temurin 21).
- **Android SDK** and **`ANDROID_HOME`** (or `ANDROID_SDK_ROOT`) set.
- Optional: **Android Studio** for GUI builds and emulators (`pnpm --filter @guardedsleep/mobile exec cap open android`).

If Gradle still fails inside the Dev Container, check **`JAVA_HOME`** on ARM64 (see [development.md](development.md)).

## Build steps (local)

From the **repository root** (with `pnpm` and dependencies installed):

1. **Web bundle + sync into Android**

   ```bash
   pnpm install --frozen-lockfile
   pnpm --filter @guardedsleep/mobile build
   pnpm --filter @guardedsleep/mobile exec cap sync
   ```

2. **Debug APK** (always works once SDK + JDK are OK)

   ```bash
   cd apps/mobile/android
   ./gradlew assembleDebug
   ```

   Output (typical): `apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`

3. **Release APK / AAB** (after you configure signing)

   ```bash
   cd apps/mobile/android
   ./gradlew assembleRelease    # APK
   # or
   ./gradlew bundleRelease      # AAB for Play Console
   ```

   Until `signingConfigs` (or Android Studio’s signing UI) is set up, Gradle may fail with a signing error — that is expected.

## APK build with Docker (`docker/android-build`)

If `./gradlew` fails with **no `javac`** / **invalid source release** (often a JRE-only package or wrong Java version), use the image under `docker/android-build/` (Temurin **JDK 21** + Android SDK 35). You still run **`cap sync` where Node exists** (host or Dev Container).

From the **repository root**:

```bash
pnpm install --frozen-lockfile
pnpm --filter @guardedsleep/mobile build
pnpm --filter @guardedsleep/mobile exec cap sync

docker compose -f docker/android-build/compose.yaml build
docker compose -f docker/android-build/compose.yaml run --rm apk
```

That Compose service runs **`assembleDebug`** only. For a release build inside the same image, override the command (after signing exists), e.g.:

```bash
docker compose -f docker/android-build/compose.yaml run --rm apk ./gradlew assembleRelease --no-daemon
```

Details: [docker/android-build/README.md](../../docker/android-build/README.md).

## Release build

Requires **your** signing configuration in `android/app` (keystore file + Gradle `signingConfigs`, or Android Studio’s signed-build flow). Official overview: [Sign your app](https://developer.android.com/studio/publish/app-signing) (Android Developers).

From `apps/mobile/android` after signing is configured:

```bash
./gradlew assembleRelease
```

## Signaling URL in the APK

`VITE_SIGNALING_URL` is fixed at **`pnpm build`** time. For a self-hosted server, either:

- Build with `VITE_SIGNALING_URL=wss://your.signal.host pnpm --filter @guardedsleep/mobile build` (and re-run `cap sync`), or
- Rely on the in-app **Signaling server** field (saved per device) so one APK can target multiple hosts.

## CI (GitHub Actions)

The workflow [`.github/workflows/android.yml`](../../.github/workflows/android.yml) builds a **debug APK** only and uploads it as a workflow artifact:

- **Pull requests** — when the PR touches `apps/mobile/**`, `packages/protocol/**`, the root `pnpm-lock.yaml`, or the workflow file itself.
- **`main` / `master` / `develop`** — same path filters on push.
- **Tags** `mobile-v*` — Release Please tags for the `mobile` component; use these runs to download an APK that matches a published app version.

Steps mirror local builds: `pnpm install`, `pnpm --filter @guardedsleep/mobile build`, `cap sync`, then `./gradlew assembleDebug` with **Temurin JDK 21** and **Android SDK 35** (`android-actions/setup-android@v4`). The artifact is the standard Capacitor output: `app-debug.apk` under `app/build/outputs/apk/debug/`.

**Not covered in CI:** signed **release** / Play **AAB** builds (keystore secrets and Gradle wiring). The root `ci.yml` job still builds the static web bundle only; the Android workflow is additive.

Download: GitHub → **Actions** → select a run → **Artifacts** → `guardedsleep-android-debug-<run_id>.zip`.
