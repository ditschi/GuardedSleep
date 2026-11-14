# Spec Delta

## ADDED Requirements

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
