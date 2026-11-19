# Spec Delta

## MODIFIED Requirements

### Requirement: Stopping monitoring on the parent

On the parent unit, "Stop monitoring" SHALL be reachable from the monitor's More sheet, set apart from the other actions, and as a quiet text button at the end of the full-screen connection alerts, and SHALL always ask for confirmation. Opening or cancelling the confirmation SHALL NOT silence or acknowledge any alert.

#### Scenario: Accidental tap at night

- **WHEN** the parent opens the More sheet and taps "Stop monitoring"
- **THEN** monitoring stops only after the confirmation

#### Scenario: Confirmation over a running alarm (failure)

- **WHEN** the loud alarm sounds and the parent opens the stop confirmation and then taps "Keep monitoring"
- **THEN** the alarm kept sounding the whole time and the alert screen is unchanged

### Requirement: Baby kiosk

While monitoring, the baby unit SHALL show a dark kiosk at minimum brightness that locks 30 s after the last touch; only the set-up confirmation and a touch-wake MAY raise it to low brightness (about 10 %), and the status line and marks SHALL stay within the kiosk luminance cap (`connection-status`). Unlocking SHALL need hold-and-slide. The kiosk SHALL never use vibration or haptics, and sound only for a granted talk (`talk-back`).

#### Scenario: Toddler taps the baby phone

- **WHEN** someone taps the locked baby unit
- **THEN** the screen wakes dimly and shows the status, and only hold-and-slide unlocks it

#### Scenario: Parent talks while the kiosk is dark

- **WHEN** a parent talks while the kiosk screen is dark
- **THEN** the dim talk indicator appears and the screen stays at minimum brightness

#### Scenario: Status line at night

- **WHEN** the kiosk has been untouched for 10 minutes while connected
- **THEN** the screen is at minimum brightness and only the dim status line and the short bar are visible

### Requirement: Role choice

The role, baby unit or parent unit, SHALL be chosen per session. On the first start, with no pairing in either role, the app SHALL show two large choices. After that, Home SHALL show both roles, each with its saved connections, and starting a connection SHALL choose its role. A device SHALL have exactly one role at a time and SHALL keep the pairing of each role; a role SHALL never be changed remotely.

#### Scenario: First start

- **WHEN** the app starts for the first time with no pairing in either role
- **THEN** it shows the two large choices "Baby device" and "Parent device", without "Use as default"

#### Scenario: Returning user

- **WHEN** the app opens on Home on a device with a parent pairing "Nursery" and a baby pairing "Travel cot"
- **THEN** Home shows both role headers with their connections, and Start on "Travel cot" starts the baby unit with that pairing

#### Scenario: Update from a single-role install

- **WHEN** a device that chose a role before this change starts for the first time after the update
- **THEN** its start mode is that role and its pairing belongs to that role

### Requirement: Start mode

Settings › This device › Open at start SHALL offer _Ask every time_ (default), _Baby unit_ and _Parent unit_; there SHALL be no "Use as default" on the first-start choices. With _Ask every time_ and monitoring not active, the app SHALL open on Home. With a role and monitoring not active, it SHALL open in that role, where the automatic start applies to the parent. When it starts with monitoring active, it SHALL resume the running role.

#### Scenario: Start without a default

- **WHEN** the app starts on a device whose start mode is _Ask every time_ and monitoring is not active
- **THEN** it shows Home with both roles

#### Scenario: Second start

- **WHEN** the app starts on a device whose start mode is _Parent unit_, with no parent default connection and monitoring not active
- **THEN** it opens in the parent role on Home without asking

#### Scenario: Restart during monitoring

- **WHEN** the process restarts while the baby unit is monitoring and the start mode is _Parent unit_
- **THEN** the app resumes as baby unit

### Requirement: Switching mode

There SHALL be no separate "Switch mode" action: while monitoring is not active, starting a connection of the other role on Home SHALL switch the role. Switching SHALL NOT change the start mode and SHALL NOT delete a pairing. While monitoring is active, including the baby kiosk, no switch SHALL be offered.

#### Scenario: Swap for one night

- **WHEN** a phone whose start mode is _Parent unit_ starts its baby connection "Travel cot" on Home
- **THEN** it monitors as baby unit with that pairing, and the next start without monitoring opens in the parent role again with the parent pairing intact

#### Scenario: Monitoring running

- **WHEN** the parent unit is monitoring
- **THEN** no role or connection switch is shown

### Requirement: Parent pairing entry

"Set up new devices" on Home SHALL lead to the parent scan screen whenever fewer than the cap of parent pairings are saved, asking for the camera permission there. With no saved parent pairing, choosing "Parent device" on the first start and finishing the parent setup checklist SHALL open the scan screen directly; cancelling it SHALL return to Home with "Set up new devices" and SHALL NOT reopen the scan screen until Home is entered again.

#### Scenario: First parent start

- **WHEN** a parent with no saved pairing finishes the setup checklist
- **THEN** the scan screen opens and the camera permission is requested

#### Scenario: Scanning cancelled

- **WHEN** the parent cancels the scan screen with no saved pairing
- **THEN** Home is shown and "Set up new devices" opens the scan screen again

### Requirement: Automatic start with the default connection

On the parent unit the default connection SHALL be a user choice, independent of the start mode, set and removed in Settings › This device › Open at start; a new pairing SHALL NOT become the default by itself. When the app opens in the parent role, unless the baby unit monitored or showed an invite before in this app start, a saved default SHALL start monitoring, at most once per app start; without one, Home SHALL be shown.

#### Scenario: Default marked

- **WHEN** the app opens in the parent role and "Home" is the default connection
- **THEN** monitoring starts with "Home" without a tap

#### Scenario: Stopped after the automatic start

- **WHEN** the parent stops monitoring after an automatic start
- **THEN** Home is shown and monitoring does not start again until the next app start

#### Scenario: No default

- **WHEN** the app opens in the parent role with two saved connections and no default
- **THEN** Home shows the connections and "Set up new devices", and nothing starts

#### Scenario: Default role baby, switching to parent

- **WHEN** the start mode is _Baby unit_, "Home" is the parent default connection, and right after opening the app the user goes back to Home and taps Start on "Home"
- **THEN** monitoring starts with "Home" with that one tap

#### Scenario: Baby used first

- **WHEN** the baby unit monitored in this app start and the parent role is opened afterwards
- **THEN** nothing starts and Home is shown

### Requirement: Leaving the automatic start

After an automatic start the parent unit SHALL offer "Choose another connection", which stops monitoring and shows Home with the saved connections listed. Before the link has verified and at levels 0 and 1 it SHALL be one tap without confirmation; afterwards, and from level 2, it SHALL go through the stop confirmation (`connection-status` "Leaving from the alarm needs confirmation").

#### Scenario: Wrong place

- **WHEN** the default "Home" starts automatically while the parent is travelling and the link is still connecting
- **THEN** one tap on "Choose another connection" stops it and shows Home, where "Travel" can be started

## ADDED Requirements

### Requirement: Settings structure

Settings SHALL have two groups: "Connections" (the saved connections per role, with rename and forget) and "This device", whose rows depend on the device's role. Settings that can break monitoring SHALL stay in Advanced. While monitoring, the status bar SHALL stay on top of every settings page.

#### Scenario: Rows depend on the role

- **WHEN** a parent device opens Settings
- **THEN** This device lists Alerts, Display, Open at start, Check setup, Language, About and Advanced
- **WHEN** a baby device opens Settings
- **THEN** This device lists Sound detection, Quiet mode, Talk-back, Open at start, Check setup, Language, About and Advanced, and no Display row because the baby device is always dark

#### Scenario: Finding the alarm tone

- **WHEN** the parent opens Settings
- **THEN** the connection alarm tone is under This device › Alerts and the server addresses are under This device › Advanced

### Requirement: Connections read-only while monitoring

While monitoring, Settings › Connections SHALL show the saved connections without rename, forget or default actions, mark the one in use with a dot and words, and say "Stop monitoring to change connections". The monitor SHALL offer no way to switch the connection.

#### Scenario: Trying to rename during the night

- **WHEN** the parent opens Settings › Connections while monitoring
- **THEN** the rows cannot be edited and the note says to stop monitoring first
