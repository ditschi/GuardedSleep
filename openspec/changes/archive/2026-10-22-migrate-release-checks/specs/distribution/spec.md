# Spec Delta

## ADDED Requirements

### Requirement: No test hooks in release builds

Release APKs SHALL NOT contain the debug launch activity, the audio-injection receiver or its intent action, and debug-only test hooks (autostart and configuration extras, PoC logs) SHALL exist only in debug builds. An inspection of the release APK SHALL fail the build otherwise.

#### Scenario: Injection receiver left in a release build

- **WHEN** the release APK's manifest or dex still contains the audio-injection receiver
- **THEN** the release-surface check fails

### Requirement: Core size budget

The stripped Rust core library SHALL add at most 15 MB to the APK, summed over every packaged ABI.

#### Scenario: Core grows past the budget

- **WHEN** a change makes the packaged core libraries larger than 15 MB in total
- **THEN** the size check fails
