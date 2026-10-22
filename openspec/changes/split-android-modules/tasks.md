# Tasks

Refined 2026-10-22 from the wave-2 plan (group 2, after group 1). Repository content filters already landed with the `swap-webrtc-sdk` review fixes (`apps/android/settings.gradle.kts`).

## 1. Build structure

- [x] 1.1 `apps/android/build-logic` included build with convention plugins (android-application, android-library, compose, kotlin); configuration cache and build cache on in `gradle.properties`; wrapper `distributionSha256Sum`
- [x] 1.2 Modules `app` (UI + navigation), `ui-contract` (state models from `UiContract.kt`, no Android UI deps), `platform` (service, media, signaling, `media/libwebrtc`), `core-bindings` (renamed from `:core`: UniFFI output + JNI libs); only `platform` depends on `core-bindings`; manual constructor DI wiring in `app`
- [x] 1.3 Prove the boundary: a temporary import of a `core-bindings` type in `app` fails to compile (record the result, revert); the `MediaEngineSeamTest` and `ConformanceTest` still run in their new modules

## 2. Formatting, lint, release

- [x] 2.1 One reformat commit with the pinned ktfmt over all Kotlin; `just fmt` / `just fmt-check` include Kotlin
- [x] 2.2 Android Lint `warningsAsErrors` in every module, no baseline file; fix or justify (with `@Suppress` and a reason) every finding
- [ ] 2.3 R8 on for release with keep rules for UniFFI/JNA and libwebrtc; release-check and size-check still pass
- [ ] 2.4 `version.properties` holds `versionName`; Gradle derives `versionCode` deterministically (documented formula with room for pre-release numbers); test for ordering of alpha/beta/release names

## 3. Close

- [ ] 3.1 Update scripts, CI recipes and docs that name `:core`; `docs/ci-gates.md`
- [ ] 3.2 `just lint`, `just test`, `just verify-android` and the emulator gate green
