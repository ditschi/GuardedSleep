# ui-appearance Specification

## ADDED Requirements

### Requirement: Colour schemes

Android and web SHALL offer the colour schemes Dusk (dark, default) and Day (light) from one shared token set, and night mode on top of either. Body text SHALL reach at least 4.5:1 against its surface in every scheme, and text in the main monitoring states at least 7:1.

#### Scenario: First start

- **WHEN** the app starts for the first time
- **THEN** it uses the Dusk scheme and night mode is off

### Requirement: Fixed safety colours

The safety colours (info blue, warning orange, alert red, caution yellow, ok green) and the full-screen alert palettes SHALL be defined outside the colour schemes and SHALL NOT change with Dusk, Day or night mode. No other element SHALL use a hue within 20° of a safety hue.

#### Scenario: Day scheme at the orange stage

- **WHEN** the Day scheme is active and the link reaches level 1
- **THEN** the status bar uses the same orange safety colour, outline, symbol and words as in Dusk

### Requirement: Night mode

The parent and baby devices SHALL offer night mode with the choices Off (default), On and Automatic, from the monitor tool row, the More sheet and Settings › Display. Night mode SHALL dim the interface with red-shifted colours on black while keeping text at least 4.5:1, and the dimness SHALL come from the screen brightness, not from lower contrast.

#### Scenario: Switching on at the bedside

- **WHEN** the parent taps the night icon on the monitor
- **THEN** the monitor switches to night colours at once and the choice is still active after the app restarts

### Requirement: Automatic night mode by time

With Automatic, night mode SHALL be on inside a user-set daily time window and off outside it, using the device clock only; no light sensor or other sensor SHALL be used. A manual change SHALL win until the next start or end of the window.

#### Scenario: Window ends in the morning

- **WHEN** the window is 21:00–07:00, the parent turned night mode off at 23:00, and the clock passes 07:00 and then 21:00
- **THEN** night mode stays off until 21:00 and turns on at 21:00

### Requirement: Night mode never hides safety signals

Night mode SHALL NOT apply to full-screen alerts, to the status bar from level 1 on and in the grey and red states, to warning banners or to safety chips; they SHALL keep their fixed safety colours.

#### Scenario: Orange stage at night (failure)

- **WHEN** night mode is on and the link reaches level 1
- **THEN** the status bar shows the fixed orange safety colour with outline, symbol and words, while the rest of the screen stays in night colours

### Requirement: Bundled typeface

Both clients SHALL use the bundled Plus Jakarta Sans typeface with tabular numerals for counters and times, shipped inside the app and served from the web client's own origin, and SHALL NOT load fonts from a third-party server. Scripts the typeface does not cover SHALL fall back to the system font. Running text SHALL be at least 14 sp and caps labels at least 12 sp.

#### Scenario: Web client offline

- **WHEN** the web client runs in local PC mode without internet
- **THEN** the typeface loads from the own origin and no font request leaves the device's network

### Requirement: Touch targets and gesture alternatives

Every interactive control SHALL be at least 48 dp in both dimensions, and every gesture SHALL have an alternative: Listen is a toggle, Talk keeps the hold alternative of the platform screen reader (`talk-back`), kiosk unlocking has an accessibility action with confirmation, and every sheet has a visible Close button.

#### Scenario: Switch access user

- **WHEN** a parent using switch access opens the More sheet
- **THEN** a Close button of at least 48 dp closes it without a drag gesture

### Requirement: Large text layout

At 200 % text size no action SHALL leave the screen: alert bodies scroll with the main action pinned, the monitor's status bar and Listen stay pinned, and buttons never wrap or cut their label. Snapshot tests at font scale 2.0 SHALL cover the monitor and the full-screen alerts.

#### Scenario: Loud alarm at 200 %

- **WHEN** the font scale is 2.0 and the loud alarm screen shows
- **THEN** "I'm checking" is fully visible without scrolling and the rest of the body scrolls

### Requirement: Back behaviour while monitoring

While monitoring, back on the monitor SHALL move the app to the background without stopping, back on a full-screen alert SHALL do nothing, back on a sheet SHALL close it, and back in Settings SHALL return to the monitor.

#### Scenario: Back pressed on the monitor

- **WHEN** the parent presses back on the monitor screen
- **THEN** the app goes to the background and monitoring continues
