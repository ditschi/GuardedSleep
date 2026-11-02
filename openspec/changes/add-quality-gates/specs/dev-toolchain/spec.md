# Spec Delta

## ADDED Requirements

### Requirement: Design tokens and contrast

Colours SHALL come from one DTCG-shaped token file generated for Compose and CSS, every status token SHALL have an icon id and a message key, and a test SHALL check WCAG AA contrast (4.5:1 for text, 3:1 for status glyphs) in the dark, light and dim baby-unit themes.

#### Scenario: Low-contrast status colour

- **WHEN** a token change drops a status glyph below 3:1 against its background
- **THEN** the contrast test fails

### Requirement: Accessibility goldens

Roborazzi with an accessibility check SHALL cover 11 goldens: connected, alert, reconnecting, link lost and microphone denied, each in English at 100 % and German at 200 % font size on a small phone, plus a light-mode monitor screen; a text-overflow check SHALL run without baselines.

#### Scenario: Clipped German alert

- **WHEN** the German alert text is clipped at 200 % font size
- **THEN** the overflow check fails

### Requirement: Log canary

Simulation and emulator-gate runs SHALL pair with a known PSK, and CI SHALL fail if that PSK appears in any log in any encoding.

#### Scenario: PSK logged in base64

- **WHEN** a code path logs the PSK encoded as base64
- **THEN** the log canary fails the run

### Requirement: Fuzzing and size budgets

Two cargo-fuzz targets (signaling envelope decode, invite parse) SHALL run nightly, and size budgets SHALL fail CI when the WASM bundle or the APK grows past its limit.

#### Scenario: Parser panic found nightly

- **WHEN** the nightly fuzz run finds an input that panics the envelope decoder
- **THEN** the nightly job fails and the input becomes a regression test
