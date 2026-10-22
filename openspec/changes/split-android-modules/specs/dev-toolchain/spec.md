# Spec Delta

## ADDED Requirements

### Requirement: Android module boundaries

The Android app SHALL consist of the modules `app`, `ui-contract`, `platform` and `core-bindings`, and only `platform` SHALL depend on `core-bindings`, so UI code that imports a binding type does not compile.

#### Scenario: Compose screen imports an FFI type

- **WHEN** a screen in `app` imports a type from `core-bindings`
- **THEN** the build fails

### Requirement: Kotlin formatting and lint

`just fmt` and `just fmt-check` SHALL format and check Kotlin with the pinned ktfmt, and Android Lint SHALL treat warnings as errors.

#### Scenario: Unformatted Kotlin file

- **WHEN** a Kotlin file is not ktfmt-formatted
- **THEN** `just fmt-check` fails
