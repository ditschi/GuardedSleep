# Spec Delta

## ADDED Requirements

### Requirement: Derived Android version code

The Android `versionCode` SHALL be derived from `versionName` in `version.properties` and SHALL NOT be edited by hand.

#### Scenario: New alpha build

- **WHEN** `versionName` changes from `0.1.0-alpha.1` to `0.1.0-alpha.2`
- **THEN** the build computes a higher `versionCode` without a manual edit
