# Android APK build (Docker)

**This is not the Dev Container.** It is a **small optional image** (Temurin JDK 21 + Android SDK only) for running `./gradlew` when your **host** has no usable JDK/SDK. The **Dev Container** (`.devcontainer/`) already includes Node + JDK + SDK in one image — see [android-apk-build.md](../contributing/android-apk-build.md#where-to-build-locally-two-setups).

If you use the **Dev Container**, you normally **do not** need this Compose file — run `cap sync` and `./gradlew` there.

## When to use

- You already ran **`cap sync`** (needs Node) so `android/app/src/main/assets/public` is up to date.
- You want `./gradlew assembleDebug` in an environment with **Temurin JDK 21** (matches Capacitor/AGP `sourceCompatibility` / `release`) and **SDK platform 35**.

## Commands

From the **repository root**:

```bash
pnpm install --frozen-lockfile
pnpm --filter @guardedsleep/mobile build
pnpm --filter @guardedsleep/mobile exec cap sync

docker compose -f docker/android-build/compose.yaml build
docker compose -f docker/android-build/compose.yaml run --rm apk
```

Debug APK path:

`apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`

**Release build:** the default service runs `assembleDebug` only. After [signing](https://developer.android.com/studio/publish/app-signing) is configured in the Gradle project, run e.g.:

```bash
docker compose -f docker/android-build/compose.yaml run --rm apk ./gradlew assembleRelease --no-daemon
```

## Notes

- The repo is bind-mounted read-write so Gradle writes outputs into your tree.
- Gradle caches live in the **`gradle-home`** named volume (faster repeat runs).
- For an interactive shell: `docker compose -f docker/android-build/compose.yaml run --rm apk bash`
