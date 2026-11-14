# Spec Delta

## ADDED Requirements

### Requirement: UI journeys on two emulators

The emulator gate SHALL run a pinned Maestro UI journey in which two emulators complete onboarding, point themselves at the gate's own signaling server and TURN through the Settings UI, pair by pasting the baby's code into the parent, and run a test alert and a stop. No pairing code SHALL be written to the gate's published output, and the log canary SHALL check the code. Under `GS_STRICT=1` a missing Maestro SHALL fail the gate. The journey SHALL run wherever the gate runs (nightly, on dispatch, on PRs matching the `gate` path filter) and on the release commit.

#### Scenario: Release

- **WHEN** a release tag is prepared
- **THEN** a strict gate run on that commit, including the UI journey, is green

#### Scenario: Gate without Maestro in CI

- **WHEN** the gate runs with `GS_STRICT=1` and Maestro is not available
- **THEN** the gate fails instead of skipping the UI checks
