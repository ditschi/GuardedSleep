# Spec Delta

## ADDED Requirements

### Requirement: Adaptive launcher icon

The Android app SHALL declare an adaptive launcher icon with background, foreground and monochrome layers as `android:icon` and `android:roundIcon`.

#### Scenario: Icon in the APK

- **WHEN** `aapt2 dump badging` reads a built APK
- **THEN** it names a launcher icon resource for the application

### Requirement: Compressed native libraries

The Android app SHALL package its native libraries compressed, and the published dev APK SHALL contain only the `arm64-v8a` libraries.

#### Scenario: Dev APK published

- **WHEN** the `dev-apk` job publishes the gate-tested APK
- **THEN** the APK holds no `lib/x86_64/` entries and still verifies with `apksigner`
