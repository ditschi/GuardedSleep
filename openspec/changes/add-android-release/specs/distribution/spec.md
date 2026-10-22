# Spec Delta

## ADDED Requirements

### Requirement: Android release tags

At alpha, an Android release SHALL be made by pushing a manual `android-v<version>` tag, which builds, signs and publishes that version from the protected release environment.

#### Scenario: Alpha release

- **WHEN** the owner pushes `android-v0.1.0-alpha.2`
- **THEN** the release workflow builds and signs that version and publishes it to the alpha channels

### Requirement: Release signing keys

The alpha signing key SHALL live only in the protected release environment. The key for directly distributed APKs SHALL be generated offline, SHALL never be stored in CI, and SHALL have a written rotation-lineage plan.

#### Scenario: CI compromise

- **WHEN** an attacker gains access to CI secrets
- **THEN** they cannot sign a direct-distribution APK

### Requirement: Alpha channels

Alpha builds SHALL be published to a Google Play testing track under `org.guardedsleep.app.alpha` and as an APK on GitHub Releases. Store texts SHALL describe F-Droid availability only as "F-Droid clients via IzzyOnDroid; F-Droid main repository later".

#### Scenario: Tester without Play

- **WHEN** a tester cannot use Google Play
- **THEN** they can install the signed APK from GitHub Releases

### Requirement: Release APK assertions

The release-APK inspection SHALL also fail on tracker classes, permissions outside the expected set, a missing backup exclusion, and `debuggable=true`.

#### Scenario: New dependency adds a permission

- **WHEN** a dependency adds a permission that is not in the expected set
- **THEN** the release-APK inspection fails
