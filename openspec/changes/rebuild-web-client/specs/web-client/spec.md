# Spec Delta

## Purpose

Offer the full GuardedSleep client in a browser, as baby unit or parent unit, on the same Rust core as Android, and be honest about what a browser cannot guarantee. Decided in ADR-019, ADR-020 §5 and ADR-018 §2/§6; built by this change.

## ADDED Requirements

### Requirement: Full client in both roles

The web client SHALL run as baby unit and as parent unit on the WASM build of the core `Link`, with the same pairing, link supervision, alert and status behaviour as the capability specs describe for Android, except where a browser cannot provide it.

#### Scenario: Laptop as baby unit

- **WHEN** a parent pairs a laptop browser as baby unit with an Android parent unit
- **THEN** noise alerts and connection-loss escalation work as with an Android baby unit

### Requirement: Keep this unit awake

A browser baby unit SHALL guide the user through a "keep this unit awake" setup (plugged in, lid open, no OS sleep or night-time restarts, tab visible) and SHALL hold a Screen Wake Lock while monitoring. It SHALL warn when the page was in the background or the system slept.

#### Scenario: Laptop went to sleep

- **WHEN** the browser baby unit resumes after the system slept
- **THEN** it warns that sounds may have been missed

### Requirement: Browser parent alerts

A browser parent unit SHALL alert while the tab is open, SHALL ask the user to tap once to turn on alert sound where the browser requires a gesture, and SHALL show a "keep this page open and the screen on" banner with a wake lock. It SHALL warn that alerts may have been missed after the page was in the background.

#### Scenario: Sound not yet unlocked

- **WHEN** a browser parent unit starts monitoring without a user gesture
- **THEN** it shows "Tap to turn on alert sound" until the user taps

### Requirement: Browser silence status

A browser baby unit SHALL report its silence status as unknown, and the parent SHALL show it in grey with the manual instruction to silence the device.

#### Scenario: Web baby unit

- **WHEN** the baby unit is a browser
- **THEN** the parent shows the silence status as unknown in grey, not as a warning

### Requirement: Pairing material in memory only

The web client SHALL keep the PSK and pairing material only in memory, never in `localStorage`, `sessionStorage`, IndexedDB or the Cache API, and a test SHALL enforce it.

#### Scenario: Page reload

- **WHEN** the user reloads a paired web client
- **THEN** no pairing material is found in browser storage

### Requirement: Browser security

The web client SHALL use a strict Content Security Policy with Trusted Types and SHALL load nothing from third-party origins.

#### Scenario: Injected script

- **WHEN** a script from another origin is injected into the page
- **THEN** the browser blocks it

### Requirement: Accessible navigation

On a route change the web client SHALL move focus to the screen heading and update the document title, and the dark theme SHALL apply before the first paint.

#### Scenario: Screen reader user changes screen

- **WHEN** a screen reader user opens the settings
- **THEN** focus lands on the settings heading
