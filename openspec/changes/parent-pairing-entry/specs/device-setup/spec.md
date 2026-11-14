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

On the parent unit the default connection SHALL be a user choice: a new pairing SHALL NOT become the default by itself, and the user SHALL be able to set and remove it on the home screen and in Settings → Connections. When the app starts in the parent role with monitoring not active and a default connection that is still saved, it SHALL start monitoring with that connection, at most once per app start. Without a default it SHALL show the home menu with the saved connections and "Set up new devices".

#### Scenario: Default marked

- **WHEN** the app opens in the parent role and "Home" is the default connection
- **THEN** monitoring starts with "Home" without a tap

#### Scenario: Stopped after the automatic start

- **WHEN** the parent stops monitoring after an automatic start
- **THEN** the home menu is shown and monitoring does not start again until the next app start

#### Scenario: No default

- **WHEN** the app opens in the parent role with two saved connections and no default
- **THEN** the home menu shows the connections and "Set up new devices", and nothing starts
