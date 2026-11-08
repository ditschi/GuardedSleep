# Spec Delta

## ADDED Requirements

### Requirement: Alert style

The parent SHALL choose an alert style for noise alerts: "Sound" (default) or "Discreet". The style SHALL be stored on the parent device, SHALL NOT be part of any pairing material, and SHALL be changeable from the parent monitor screen in at most two taps. While "Discreet" is active, the parent monitor screen SHALL show a persistent, clearly visible indicator that alerts make no sound.

#### Scenario: Switching to discreet before an event

- **WHEN** the parent opens More on the monitor screen and chooses "Discreet"
- **THEN** the monitor screen shows the indicator "Discreet alerts: no sound" and the choice is still active after the app restarts

### Requirement: Discreet escalation to sound

In "Discreet", a noise event that stays unacknowledged SHALL switch to sound after a configurable delay of 1, 2 (default) or 5 minutes: the child's audio plays as in "Sound" and the alert states "Switched to sound: no reaction for {minutes} min". The parent MAY choose "Off", which SHALL show a warning that a forgotten discreet mode can then stay silent all night. Listen, Stop sound or Stop vibration, and Snooze SHALL acknowledge. A new event after the 2-minute quiet rule SHALL restart the count. The timer SHALL run in the core.

#### Scenario: Missed vibration

- **WHEN** the style is "Discreet" with the default delay and the parent does not react to a noise event for 2 minutes
- **THEN** the child's audio starts playing and the alert shows that it switched to sound

#### Scenario: Parent reacts in time

- **WHEN** the parent taps Stop vibration after 1 minute
- **THEN** the audio does not start for that event

#### Scenario: Escalation off (failure)

- **WHEN** the escalation is "Off" and the noise lasts 10 minutes without a reaction
- **THEN** no audio plays, and the setting showed the warning when it was chosen

### Requirement: Alert style affects noise alerts only

The alert style SHALL affect noise alerts only: connection levels 1 to 3, a stopped baby unit, battery warnings, readiness warnings and snooze SHALL behave the same in both styles.

#### Scenario: Connection lost while discreet (failure)

- **WHEN** the alert style is "Discreet" and the connection to the baby unit reaches level 3
- **THEN** the audible alarm with Do Not Disturb bypass starts as in "Sound"

## MODIFIED Requirements

### Requirement: Noise alert chain

When the baby unit reports a noise event, the parent unit SHALL vibrate, turn the screen on, show a full-screen alert over the lock screen when the app is not in front (if the platform allows full-screen alerts), and show the live level meter. With the alert style "Sound" it SHALL also play the child's audio on the alarm stream through the chosen alert output; the child's audio SHALL be the alert, and there SHALL be no separate alarm ringtone for noise. With the alert style "Discreet" it SHALL NOT play the child's audio until the parent starts Listen on the alert or the escalation to sound fires; the vibration repeat, the full-screen alert, the level meter and the quiet rule SHALL be the same as for "Sound", and Listen SHALL then play the audio on the chosen alert output.

#### Scenario: Phone locked in a pocket

- **WHEN** a noise event arrives while the parent phone is locked and the alert style is "Sound"
- **THEN** the phone vibrates, the full-screen alert appears over the lock screen and the child's audio plays

#### Scenario: Discreet alert at a party

- **WHEN** a noise event arrives while the alert style is "Discreet"
- **THEN** the phone vibrates, the full-screen alert with the level meter appears and no audio plays until the parent taps Listen

#### Scenario: Back to sound during an event

- **WHEN** the parent switches from "Discreet" to "Sound" during a running event that is not snoozed
- **THEN** the child's audio starts playing at once

#### Scenario: Listen after a discreet alert

- **WHEN** the parent taps Listen on a discreet alert
- **THEN** the child's audio plays on the chosen alert output

### Requirement: Test alert

The parent SHALL be able to run a test alert. The baby unit SHALL inject a digital test event into its alert pipeline without playing anything on the baby phone, and the parent SHALL run the noise-alert chain of the active alert style labelled as a test and name that style, then ask whether the user heard and felt it. The time of the last test SHALL be stored.

#### Scenario: Test before going to bed

- **WHEN** the parent runs a test alert while connected
- **THEN** the parent phone runs the full alert chain marked as a test and the baby phone stays silent

#### Scenario: Test in discreet style

- **WHEN** the parent runs a test alert while the alert style is "Discreet"
- **THEN** the test sheet says the style is "Discreet", the phone vibrates and shows the test alert, and no audio plays
