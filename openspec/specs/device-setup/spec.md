# device-setup Specification

## Purpose

Prepare each phone for its role, baby unit or parent unit, so that monitoring keeps working in the background, and keep the baby unit dark, locked and silent next to the child. Migrated from ADR-009, ADR-010 (amended 2026-10-21), ADR-018 §2 and the device-management and user-interface feature docs, as built on Android.

## Requirements

### Requirement: Role choice

The role, baby unit or parent unit, SHALL be chosen per session from two large choices. A device SHALL have exactly one role at a time and SHALL keep the pairing of each role when the user switches; a role SHALL never be changed remotely.

#### Scenario: Update from a single-role install

- **WHEN** a device that chose a role before this change starts for the first time after the update
- **THEN** its start mode is that role and its pairing belongs to that role

### Requirement: Start mode

The setting "Start mode" SHALL offer _Ask every time_, _Baby unit_ and _Parent unit_, and the role screen SHALL offer "Use as default", which sets it. When the app starts with monitoring not active, it SHALL open in the default role, or show the role screen when there is none. When it starts with monitoring active, it SHALL resume the running role.

#### Scenario: Start without a default

- **WHEN** the app starts on a device whose start mode is _Ask every time_ and monitoring is not active
- **THEN** it shows the role screen

#### Scenario: Second start

- **WHEN** the app starts on a device whose start mode is _Parent unit_ and monitoring is not active
- **THEN** it opens as parent unit without asking

#### Scenario: Restart during monitoring

- **WHEN** the process restarts while the baby unit is monitoring and the default is _Parent unit_
- **THEN** the app resumes as baby unit

### Requirement: Setup checklist per role

Each role SHALL have one checklist screen whose rows show a state and open the matching system setting: on the baby unit the microphone, background operation (battery optimisation exemption) and silence access; on the parent unit notifications, lock-screen and full-screen alerts, Do Not Disturb bypass and background operation. Permissions SHALL be requested only when needed, with a plain reason.

#### Scenario: Battery optimisation still on

- **WHEN** the baby unit's app is still battery-optimised
- **THEN** the background row is marked to do and its button opens the system's battery optimisation setting

### Requirement: Baby device check

The baby unit SHALL check its microphone level, battery, signaling reachability, permissions and Do Not Disturb access, and show the result as OK, limited or not recommended with plain explanations. The result SHALL NOT block monitoring; the user decides.

#### Scenario: Old phone with a weak battery

- **WHEN** the device check finds a low battery that does not charge
- **THEN** it shows "limited" or "not recommended" with the reason, and monitoring can still start

### Requirement: Battery safety copy

Baby-unit setup SHALL tell the user not to use phones with swollen or damaged batteries, to place the phone on a stable, non-flammable surface and to follow the manufacturer's charging guidance.

#### Scenario: Setting up the baby unit

- **WHEN** a user sets up a phone as baby unit
- **THEN** the battery safety advice is shown during setup

### Requirement: Baby kiosk

While monitoring, the baby unit SHALL show a dark kiosk screen at minimum brightness that locks itself 30 seconds after the last touch. A touch SHALL wake it at low brightness (about 10 %) to show the status, and unlocking SHALL need a hold-and-slide gesture. The kiosk SHALL never use sound, vibration or haptics.

#### Scenario: Toddler taps the baby phone

- **WHEN** someone taps the locked baby unit
- **THEN** the screen wakes dimly and shows the status, and only hold-and-slide unlocks it

### Requirement: Settings in Basic and Advanced

Basic settings SHALL contain the noise threshold and hold time, alert output and gain, theme (dark by default), language and, on the baby unit, silencing the phone while monitoring. Settings that can break monitoring SHALL live only in Advanced: the signaling URL list (validated as `ws://` or `wss://` URLs), an optional TURN server and the heartbeat interval (5–15 s).

#### Scenario: Invalid server address

- **WHEN** the user enters a signaling URL that is not a `ws://` or `wss://` URL with a host
- **THEN** the URL is rejected with an error and not saved

### Requirement: Stopping monitoring on the parent

On the parent unit, "Stop monitoring" SHALL be reachable only from the menu and SHALL ask for confirmation, so it cannot be triggered by accident.

#### Scenario: Accidental tap at night

- **WHEN** the parent opens the menu and taps "Stop monitoring"
- **THEN** monitoring stops only after the confirmation

### Requirement: TURN server entry

Advanced settings on Android and web SHALL let the user enter one own TURN server: an address that starts with `turn:` or `turns:` and names a host, with an optional port and `transport=udp` or `transport=tcp`, and a user name and password that are both set or both empty. Both clients SHALL offer it to WebRTC as one ICE server next to the ICE servers of the pairing's relays (`signaling-relay` "Client ICE configuration"), also when the project server supplies its own TURN.

#### Scenario: Incomplete credentials

- **WHEN** the user saves a TURN address with a user name but no password
- **THEN** the entry is rejected with an error and not saved

#### Scenario: Used for the next attempt

- **WHEN** a TURN server is saved and a new attempt starts
- **THEN** the media engine is configured with the relay's ICE servers and the TURN server with its user name and password

### Requirement: TURN credential protection

A saved TURN password SHALL never be shown again, logged, or kept in saved UI state; the screen SHALL only say that a password is saved. Android SHALL store the TURN entry encrypted with an Android Keystore key, separate from the pairing. The web client SHALL keep it in memory only, never in browser storage.

#### Scenario: Password not shown again

- **WHEN** the user opens Advanced after saving a TURN server with a password
- **THEN** the address and user name are shown, and the empty password field says a password is saved

#### Scenario: Keep the saved password

- **WHEN** the user saves the same address and user name with an empty password field
- **THEN** the saved password is kept

#### Scenario: Web reload

- **WHEN** the user reloads the web client after entering a TURN server
- **THEN** no TURN user name or password is found in browser storage

### Requirement: Switching mode

While monitoring is not active, the home and pairing screens SHALL offer "Switch mode", which cancels an open invite and shows the role screen. Switching SHALL NOT change the start mode and SHALL NOT delete a pairing. While monitoring is active, including the baby kiosk, no switch SHALL be offered.

#### Scenario: Swap for one night

- **WHEN** a phone whose default is _Parent unit_ switches to baby unit on its home screen
- **THEN** it opens as baby unit with its baby pairing, and the next start without monitoring opens as parent unit again with the parent pairing intact

#### Scenario: Monitoring running

- **WHEN** the parent unit is monitoring
- **THEN** no "Switch mode" action is shown

### Requirement: Parent pairing entry

The parent unit SHALL open the scan screen from "Set up new devices" whenever fewer than the cap of parent pairings are saved, asking for the camera permission there. With no saved pairing, entering the parent home and finishing the parent setup checklist SHALL open the scan screen directly; cancelling it SHALL return to the empty home with "Set up new devices" and SHALL NOT reopen the scan screen until the home is entered again.

#### Scenario: First parent start

- **WHEN** a parent with no saved pairing finishes the setup checklist
- **THEN** the scan screen opens and the camera permission is requested

#### Scenario: Scanning cancelled

- **WHEN** the parent cancels the scan screen with no saved pairing
- **THEN** the empty home is shown and "Set up new devices" opens the scan screen again

### Requirement: Automatic start with the default connection

On the parent unit the default connection SHALL be a user choice, independent of the start mode, that can be set and removed on the home screen and in Settings → Connections; a new pairing SHALL NOT become the default by itself. On the first entry into the parent role in an app start, unless the baby unit monitored or showed an invite before, a saved default SHALL start monitoring, at most once per app start; without one the home menu SHALL be shown.

#### Scenario: Default marked

- **WHEN** the app opens in the parent role and "Home" is the default connection
- **THEN** monitoring starts with "Home" without a tap

#### Scenario: Stopped after the automatic start

- **WHEN** the parent stops monitoring after an automatic start
- **THEN** the home menu is shown and monitoring does not start again until the next app start

#### Scenario: No default

- **WHEN** the app opens in the parent role with two saved connections and no default
- **THEN** the home menu shows the connections and "Set up new devices", and nothing starts

#### Scenario: Default role baby, switching to parent

- **WHEN** the start mode is _Baby unit_, "Home" is the parent default connection and the user switches to parent mode on the baby home right after opening the app
- **THEN** monitoring starts with "Home" without a further tap

#### Scenario: Baby used first

- **WHEN** the baby unit monitored in this app start and the user then switches to parent mode
- **THEN** nothing starts and the home menu is shown

### Requirement: Leaving the automatic start

After an automatic start the parent unit SHALL offer "Choose another connection", which stops monitoring and shows the home menu with the saved connections listed. While the link has not verified yet it SHALL be a visible button without confirmation; afterwards it SHALL be in the menu and ask for confirmation like "Stop monitoring".

#### Scenario: Wrong place

- **WHEN** the default "Home" starts automatically while the parent is travelling and the link is still connecting
- **THEN** one tap on "Choose another connection" stops it and shows the list, where "Travel" can be started
