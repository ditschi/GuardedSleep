# Spec Delta

## ADDED Requirements

### Requirement: Browser talk-back

A browser parent unit SHALL offer talk-back like the Android parent unit and SHALL open the microphone only while the talk button is held, asking for microphone permission at the first press with a plain reason. A browser baby unit SHALL report talk-back as not supported and deny every talk request, because a browser cannot guarantee that its sound is audible (system mute, output device, volume).

#### Scenario: Laptop parent talks to an Android baby unit

- **WHEN** a browser parent unit holds the talk button on a connected link to an Android baby unit that grants the talk
- **THEN** the parent's voice plays on the baby unit, and the browser's microphone is released when the button is released

#### Scenario: Microphone permission refused

- **WHEN** the user refuses microphone access at the first press
- **THEN** the browser parent unit sends no talk request and says that talk-back needs the microphone

#### Scenario: Laptop as baby unit

- **WHEN** a parent presses the talk button on a link to a browser baby unit
- **THEN** the parent sees that this baby device cannot play talk-back, and the browser baby unit plays nothing
