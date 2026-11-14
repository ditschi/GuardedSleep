# Spec Delta

## MODIFIED Requirements

### Requirement: Test layers

Behaviour SHALL be proven locally first, then on emulators, then on hardware. The layers are: Rust unit, property and snapshot tests with conformance vectors and traces; seeded host simulation scenarios; Android JVM tests with Roborazzi screenshots and UI journeys through the real controller; Playwright browser E2E against the local stack; and the emulator gate on KVM (AC-1 to AC-4 and AC-10: Doze, foreground service, alerts, link recovery).

#### Scenario: Link recovery change

- **WHEN** a change touches link recovery
- **THEN** the host simulation scenarios pass before the emulator gate runs it on devices

### Requirement: Android module boundaries

The Android app SHALL consist of the modules `app`, `ui-contract`, `platform` and `core-bindings`, plus the test-only module `test-fixtures`. Only `platform` and `test-fixtures` SHALL depend on `core-bindings`, neither SHALL export it, and `app` and its tests SHALL see no binding type, so UI code that imports a binding type does not compile.

#### Scenario: Compose screen imports an FFI type

- **WHEN** a screen in `app` imports a type from `core-bindings`
- **THEN** the build fails

#### Scenario: Journey test imports an FFI type

- **WHEN** a test in `app/src/test` imports a `uniffi` type
- **THEN** the boundary check fails

## ADDED Requirements

### Requirement: Android UI journeys through the real controller

`just test` SHALL run Android journey tests in which two devices, each a real `LinkController` with its real core and only signaling, media, audio, time and system services replaced, are driven through the standard user flows by stable test tags, including pairing a baby and a parent device end to end. The journeys SHALL be deterministic (virtual time, no sleeps) and SHALL finish within 90 seconds together.

#### Scenario: A button that does nothing

- **WHEN** a contract method behind a button returns without effect in the real controller
- **THEN** the journey that taps the button fails on the next PR

### Requirement: Complete UI contract

The Kotlin `AppController` SHALL declare no default method bodies. Every leaf field of the parent, baby and app UI state SHALL reach a non-default value in at least one journey, unless an allowlist entry gives the reason.

#### Scenario: Forgotten override

- **WHEN** a new contract method is added without an implementation in `LinkController`
- **THEN** the build fails

#### Scenario: State nobody writes

- **WHEN** a new UI state field is added that the platform never sets
- **THEN** the journey suite fails until the platform writes it or an allowlist entry explains why
