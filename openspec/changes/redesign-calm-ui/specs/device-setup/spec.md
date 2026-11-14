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

## ADDED Requirements

### Requirement: Settings structure

Settings SHALL have two groups: "Connections" (the saved connections per role, with rename and forget) and "This device" (Alerts, Sound detection, Display, Open at start, Check setup, Language, About, Advanced). Settings that can break monitoring SHALL stay in Advanced. While monitoring, the status bar SHALL stay on top of every settings page.

#### Scenario: Finding the alarm tone

- **WHEN** the parent opens Settings
- **THEN** the connection alarm tone is under This device › Alerts and the server addresses are under This device › Advanced

### Requirement: Connections read-only while monitoring

While monitoring, Settings › Connections SHALL show the saved connections without rename, forget or default actions, mark the one in use with a dot and words, and say "Stop monitoring to change connections". The monitor SHALL offer no way to switch the connection.

#### Scenario: Trying to rename during the night

- **WHEN** the parent opens Settings › Connections while monitoring
- **THEN** the rows cannot be edited and the note says to stop monitoring first
