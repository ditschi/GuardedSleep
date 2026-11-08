# Spec Delta

## ADDED Requirements

### Requirement: Browser parent alert style

A browser parent unit SHALL offer the alert style "Sound" (default) or "Discreet" and SHALL show the persistent indicator while "Discreet" is active. In "Discreet" a noise alert SHALL show the in-page alert and the level meter without playing the child's audio or a test tone until the parent starts Listen. The web client SHALL NOT vibrate, so a discreet browser alert is visual only; the setting SHALL live in the page's non-secret settings state and never in pairing material.

#### Scenario: Discreet noise in the browser

- **WHEN** the browser parent unit is in "Discreet" and the core raises a noise alert
- **THEN** the in-page alert appears with the level meter and no audio plays until the parent taps Listen
