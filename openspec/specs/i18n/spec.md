# i18n Specification

## Purpose

Keep all user-visible copy in one catalogue with English as the master, ship only complete languages, and turn the catalogue into platform resources by a checked generator, so no screen or alarm shows hard-coded or mixed-language text. Migrated from ADR-020 §7 and ADR-021 §2, as built.

## Requirements

### Requirement: One catalogue

All user-visible copy SHALL live only in `locales/*.json`, with English (`en`) as the master. Values SHALL use the ICU plural and select subset with named placeholders. The core SHALL emit message keys, never prose.

#### Scenario: New screen text

- **WHEN** a developer adds text to a screen
- **THEN** the text is added as a key in `locales/en.json` and the UI refers to that key

### Requirement: No literals in UI code

A lint SHALL fail when Android Compose UI code passes a string literal as text, label or content description; empty strings and test tags are allowed.

#### Scenario: Hard-coded button label

- **WHEN** Compose code contains `Text("Start")`
- **THEN** the lint fails

### Requirement: Shipped locales are complete

The app SHALL ship English, German, French and Spanish. Every shipped locale SHALL contain exactly the keys of the English master with the same placeholders and all CLDR plural categories its language needs; otherwise the catalogue check SHALL fail.

#### Scenario: Missing German key

- **WHEN** a key exists in `en.json` but not in `de.json`
- **THEN** the catalogue check fails and names the missing key

### Requirement: Generated resources

A generator SHALL produce Android string and plural resources and a web catalogue (including an accented pseudo-locale for layout testing) from the catalogue. Generated output SHALL NOT be committed.

#### Scenario: Fresh checkout

- **WHEN** someone builds the app from a fresh checkout
- **THEN** the build generates the platform resources from `locales/*.json`

### Requirement: Spellchecking

English text SHALL be spellchecked by `typos`, and non-English catalogues by CSpell with per-language dictionaries.

#### Scenario: Typo in German copy

- **WHEN** a German catalogue value contains a misspelled word
- **THEN** the CSpell check reports it

### Requirement: Language choice

The user SHALL be able to follow the system language or choose one of the shipped languages in the app's settings.

#### Scenario: German phone, English preferred

- **WHEN** a user on a German system selects English in the app
- **THEN** the app shows English copy
