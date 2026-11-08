# Spec Delta

## ADDED Requirements

### Requirement: Browser parent keeps the sound alert style

A browser parent unit SHALL NOT offer the discreet alert style until the browser can vibrate: it SHALL always play the child's audio with a noise alert, as the "Sound" style does. It SHALL ignore the core's noise escalation event.

#### Scenario: Settings in the browser

- **WHEN** a user opens the settings of a browser parent unit
- **THEN** there is no alert style choice and no discreet indicator
