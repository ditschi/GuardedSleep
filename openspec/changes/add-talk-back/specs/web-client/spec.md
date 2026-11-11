# Spec Delta

## ADDED Requirements

### Requirement: Browser talk-back

A browser parent unit SHALL offer talk-back like the Android parent unit, ask for microphone permission in a separate step before the first talk, and hold the microphone only while the talk button is held. A browser baby unit SHALL play talk-back like the Android baby unit, with the chime, fade-in, volume levels and loudness cap of `talk-back`, and SHALL rely on the playback check to tell the parent when its sound was probably not audible, because a browser cannot read or set the system volume.

#### Scenario: Laptop parent talks to an Android baby unit

- **WHEN** a browser parent unit holds the talk button on a connected link to an Android baby unit that grants the talk
- **THEN** the parent's voice plays on the baby unit, and the browser's microphone is released when the button is released

#### Scenario: Microphone permission refused

- **WHEN** the user refuses microphone access in the permission step
- **THEN** the browser parent unit sends no talk request and says that talk-back needs the microphone

#### Scenario: Tab hidden mid-hold (failure)

- **WHEN** the tab is hidden or loses focus while the talk button is held
- **THEN** the talk ends and the microphone is released

#### Scenario: Laptop as baby unit

- **WHEN** a parent talks to a browser baby unit whose loudspeaker is on
- **THEN** the chime and the voice play on the laptop and the talk is reported as heard

#### Scenario: Audio blocked in the browser baby unit (failure)

- **WHEN** the browser baby unit's audio output is suspended or blocked
- **THEN** it reports talk-back as muted, or the talk as unconfirmed, and the parent sees that the voice may not be audible

### Requirement: Browser parent talk-back limits

Where alert audio is not proven to keep its output and level after a talk (iOS Safari), a browser parent unit SHALL report talk-back as not supported.

#### Scenario: iPhone browser as parent unit

- **WHEN** the web client runs as parent unit in iOS Safari
- **THEN** the talk button says that talk-back is not supported in this browser
