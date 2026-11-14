# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Night screen`
- TO: `### Requirement: Black screen`

## MODIFIED Requirements

### Requirement: Black screen

While monitoring, both roles SHALL offer "Black screen": a pure-black full-screen cover that keeps the page visible and the wake lock held, shows only a slowly drifting dim dot, shows the monitoring status for a few seconds on the first tap and closes on the second. On the parent unit an alert SHALL replace the black screen at once. It SHALL be separate from night mode (`ui-appearance`).

#### Scenario: Noise during night screen

- **WHEN** the parent unit shows the black screen and the core raises a noise alert
- **THEN** the black screen closes and the in-page alert appears with sound

### Requirement: Accessible navigation

On a route change the web client SHALL move focus to the screen heading and update the document title, and the chosen colour scheme (Dusk, Day or night mode) SHALL apply before the first paint.

#### Scenario: Screen reader user changes screen

- **WHEN** a screen reader user opens the settings
- **THEN** focus lands on the settings heading

#### Scenario: Night mode on reload

- **WHEN** night mode is on and the parent reloads the page
- **THEN** the first painted frame already uses the night colours, without a bright flash

### Requirement: Browser parent keeps the sound alert style

A browser parent unit SHALL NOT offer the "Vibrate only" alert style until the browser can vibrate: it SHALL always play the child's audio with a noise alert, as the "Sound" style does. It SHALL ignore the core's noise escalation event.

#### Scenario: Settings in the browser

- **WHEN** a user opens the settings of a browser parent unit
- **THEN** there is no alert style choice and no "Vibrate only" indicator

## ADDED Requirements

### Requirement: Same monitor in the browser

A browser parent unit SHALL show the same status bar, monitor layout, dock and banner order as the Android parent, without the alert style chip, and add its browser banners (alert sound gate, keep this page open, back from the background) to the banner order below readiness. The alert sound gate SHALL cover the monitor but leave the status bar visible.

#### Scenario: Sound not unlocked yet

- **WHEN** a browser parent starts monitoring without a user gesture
- **THEN** the sound gate covers the monitor, the status bar still shows the connection state, and the first tap on any control unlocks sound
