# web-client Specification

## Purpose

Offer the full GuardedSleep client in a browser, as baby unit or parent unit, on the same Rust core as Android, and be honest about what a browser cannot guarantee. Decided in ADR-019, ADR-020 §5 and ADR-018 §2/§6; built by `rebuild-web-client` (archived 2026-11-02).

## Requirements

### Requirement: Full client in both roles

The web client SHALL run as baby unit and as parent unit on the WASM build of the core `Link`, with the same pairing, link supervision, alert and status behaviour as the capability specs describe for Android, except where a browser cannot provide it. It SHALL render only contract models that mirror `UiContract.kt` and SHALL NOT import WASM types in UI code.

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

### Requirement: Night screen

While monitoring, both roles SHALL offer a night screen: a pure-black full-screen overlay that keeps the page visible and the wake lock held, shows only a slowly drifting dim dot, shows the monitoring status for a few seconds on the first tap and closes on the second. On the parent unit an alert SHALL replace the night screen at once.

#### Scenario: Noise during night screen

- **WHEN** the parent unit shows the night screen and the core raises a noise alert
- **THEN** the night screen closes and the in-page alert appears with sound

### Requirement: Background loss is reported

While monitoring, the web client SHALL show a system notification when the page becomes hidden (if notifications are allowed), SHALL ask for confirmation before the page is closed or left, and SHALL warn when the wake lock is released. After the page was hidden it SHALL show the background warning with the missed-events summary of the core.

#### Scenario: Parent switches to another app

- **WHEN** the parent unit page becomes hidden while monitoring and notifications are allowed
- **THEN** a notification says that GuardedSleep cannot alert while the page is in the background

### Requirement: Browser silence status

A browser baby unit SHALL report its silence status as unknown, and the parent SHALL show it in grey with the manual instruction to silence the device.

#### Scenario: Web baby unit

- **WHEN** the baby unit is a browser
- **THEN** the parent shows the silence status as unknown in grey, not as a warning

### Requirement: Copy code without sharing

The browser baby unit SHALL offer the invite as a QR code without a logo and as a "Copy code" action with a hint to paste it on the parent device within the validity and not to send it in a chat. It SHALL NOT offer a share action for the invite.

#### Scenario: Copy the code

- **WHEN** the user taps "Copy code" on the baby unit
- **THEN** the invite text is on the clipboard and no share sheet opens

### Requirement: Pairing material in memory only

The web client SHALL keep the PSK and pairing material only in memory, never in `localStorage`, `sessionStorage`, IndexedDB or the Cache API, and a test SHALL enforce it.

#### Scenario: Page reload

- **WHEN** the user reloads a paired web client
- **THEN** no pairing material is found in browser storage

### Requirement: Browser security

The web client SHALL use a strict Content Security Policy with Trusted Types, without inline scripts. It SHALL load scripts, styles, fonts and images only from its own origin, and SHALL connect only to its own origin, the signaling origins configured for the deployment and, only in a build of the official deployment, the project signaling server's origin; a blanket `wss:` SHALL NOT be allowed.

#### Scenario: Injected script

- **WHEN** a script from another origin is injected into the page
- **THEN** the browser blocks it

### Requirement: Accessible navigation

On a route change the web client SHALL move focus to the screen heading and update the document title, and the dark theme SHALL apply before the first paint.

#### Scenario: Screen reader user changes screen

- **WHEN** a screen reader user opens the settings
- **THEN** focus lands on the settings heading

### Requirement: Browser parent keeps the sound alert style

A browser parent unit SHALL NOT offer the discreet alert style until the browser can vibrate: it SHALL always play the child's audio with a noise alert, as the "Sound" style does. It SHALL ignore the core's noise escalation event.

#### Scenario: Settings in the browser

- **WHEN** a user opens the settings of a browser parent unit
- **THEN** there is no alert style choice and no discreet indicator
