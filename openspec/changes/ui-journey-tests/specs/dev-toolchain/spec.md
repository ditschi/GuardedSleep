# Spec Delta

## MODIFIED Requirements

### Requirement: Test layers

Behaviour SHALL be proven locally first, then on emulators, then on hardware. The layers are: Rust unit, property and snapshot tests with conformance vectors and traces; seeded host simulation scenarios; Android JVM tests with Roborazzi screenshots and UI journeys through the real controller; Playwright browser E2E against the local stack; and the emulator gate on KVM (AC-1 to AC-4 and AC-10: Doze, foreground service, alerts, link recovery, plus UI journeys on two emulators).

#### Scenario: Link recovery change

- **WHEN** a change touches link recovery
- **THEN** the host simulation scenarios pass before the emulator gate runs it on devices

## ADDED Requirements

### Requirement: Android UI journeys through the real controller

`just test` SHALL run Android journey tests that render the real app UI on a real `LinkController` per device, with only signaling, media, audio and system services replaced, and drive the standard user flows by stable test tags, including pairing a baby and a parent device end to end.

#### Scenario: A button that does nothing

- **WHEN** a contract method behind a button returns without effect in the real controller
- **THEN** the journey that taps the button fails on the next PR

### Requirement: Complete UI contract

The Kotlin `AppController` SHALL declare no default method bodies, and a test SHALL fail when a state field of the UI contract has no writer in the platform layer.

#### Scenario: Forgotten override

- **WHEN** a new contract method is added without an implementation in `LinkController`
- **THEN** the build fails

### Requirement: UI journeys on two emulators

The emulator gate SHALL run pinned Maestro UI journeys on two emulators that pair through the real UI (onboarding on both devices, the baby's code pasted into the parent), monitor, alert and stop. Under `GS_STRICT=1` a missing Maestro SHALL fail the gate. It SHALL run nightly, on dispatch and on the release commit, not on every PR.

#### Scenario: Release

- **WHEN** a release tag is prepared
- **THEN** a gate run on that commit, including the UI journeys, is green
