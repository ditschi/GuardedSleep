# Spec Delta

## ADDED Requirements

### Requirement: Android module boundaries

The Android app SHALL consist of the modules `app`, `ui-contract`, `platform` and `core-bindings`, and only `platform` SHALL depend on `core-bindings`, so UI code that imports a binding type does not compile.

#### Scenario: Compose screen imports an FFI type

- **WHEN** a screen in `app` imports a type from `core-bindings`
- **THEN** the build fails

### Requirement: Kotlin formatting and lint

`just fmt` and `just fmt-check` SHALL format and check Kotlin with the pinned ktfmt, and Android Lint SHALL treat warnings as errors in every module with no baseline file.

#### Scenario: Unformatted Kotlin file

- **WHEN** a Kotlin file is not ktfmt-formatted
- **THEN** `just fmt-check` fails

#### Scenario: Lint warning

- **WHEN** Android Lint reports a warning in any module
- **THEN** the build fails, and no baseline file hides it

### Requirement: Gradle wrapper checksum

The Gradle wrapper SHALL pin `distributionSha256Sum`.

#### Scenario: Tampered distribution

- **WHEN** the downloaded Gradle distribution does not match the pinned checksum
- **THEN** the wrapper refuses to run it

### Requirement: Binding seam guard

A unit test SHALL fail if a source file outside `platform` and `core-bindings` imports `uniffi.`, or if it finds no sources.

#### Scenario: UI imports a binding type

- **WHEN** a file in `app` imports `uniffi.`
- **THEN** the test fails
