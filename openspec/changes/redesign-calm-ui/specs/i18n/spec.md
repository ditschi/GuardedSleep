# Spec Delta

## ADDED Requirements

### Requirement: Copy conventions

The English catalogue SHALL call the baby "the child" and use a neutral person; German SHALL use the formal form of address. Names SHALL appear only in a "State · Name" label or as a sentence subject that needs no case ending, never spliced into inflected text. Durations in prose SHALL be words with units ("1 min 48 s"), never `m:ss`, and screen readers SHALL get the spoken form.

#### Scenario: German connection name

- **WHEN** the connection "Kinderzimmer" is reconnecting and the locale is German
- **THEN** the bar reads "Wird neu verbunden · Kinderzimmer", not a sentence with the name inflected

#### Scenario: Duration in a restored notice

- **WHEN** the link recovers after 108 seconds
- **THEN** the notice says "1 min 48 s" and a screen reader says "1 minute 48 seconds"
