# Spec Delta

## ADDED Requirements

### Requirement: Derived Android version code

The Android `versionCode` SHALL be derived from `versionName` in `version.properties` and SHALL NOT be edited by hand.

#### Scenario: New alpha build

- **WHEN** `versionName` changes from `0.1.0-alpha.1` to `0.1.0-alpha.2`
- **THEN** the build computes a higher `versionCode` without a manual edit

#### Scenario: Version name outside the supported form

- **WHEN** `versionName` is `1.0.0-snapshot`
- **THEN** the build fails

### Requirement: Version code format

The `versionCode` SHALL be `major*10_000_000 + minor*100_000 + patch*1_000 + stage*100 + n` with stage alpha=1, beta=2, rc=3 and release=9, from a `versionName` of `MAJOR.MINOR.PATCH[-(alpha|beta|rc).N]` with minor, patch and N at most 99 and major at most 209.

#### Scenario: Pre-release orders below release

- **WHEN** `0.1.0-rc.99` and `0.1.0` are compared
- **THEN** the release has the higher `versionCode`

### Requirement: Shrunk release build

The Android release build SHALL be minified and resource-shrunk with R8, and the keep rules a library needs SHALL live in that module's `consumer-rules.pro`.

#### Scenario: Release APK built

- **WHEN** `:app:assembleRelease` runs
- **THEN** R8 shrinks the dex and resources, and the release-surface and core-size checks still pass

### Requirement: Release permission allow-list

The release-surface check SHALL compare the permissions of the release APK with an allow-list file and fail on any permission not listed.

#### Scenario: Unexpected permission in release build

- **WHEN** a dependency adds a permission that is not in the allow-list
- **THEN** the release-surface check fails
