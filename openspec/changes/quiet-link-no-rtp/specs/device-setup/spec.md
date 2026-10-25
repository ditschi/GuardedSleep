# Spec Delta

## ADDED Requirements

### Requirement: Audio quality setting

The parent's Basic settings SHALL offer the audio quality Data saver (16 kbit/s), Standard (24 kbit/s, default) and Clear (32 kbit/s), with an info sheet that shows the expected data per hour of audio and per typical night for each level, and that the level applies from the next connection setup.

#### Scenario: Choosing Data saver

- **WHEN** the parent opens the info sheet and chooses Data saver
- **THEN** the sheet shows about 9 MB per hour of audio, and the next connection negotiates 16 kbit/s
