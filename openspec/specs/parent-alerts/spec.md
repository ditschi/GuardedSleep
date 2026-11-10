# parent-alerts Specification

## Purpose

Make sure the parent notices every noise event and every problem with the baby unit, even with the phone on silent or Do Not Disturb, and let the parent verify that alerts will work before relying on them. Migrated from ADR-005, ADR-011, ADR-018 §2 and `alpha-ux-flows.md` §3.7–§3.11, as built on Android.

## Requirements

### Requirement: Noise alert chain

When the baby unit reports a noise event, the parent unit SHALL vibrate, turn the screen on, show a full-screen alert over the lock screen when the app is not in front (if the platform allows full-screen alerts), and show the live level meter. With the alert style "Sound" (for "Discreet" see below) it SHALL also play the child's audio on the alarm stream through the chosen alert output; the child's audio SHALL be the alert, and there SHALL be no separate alarm ringtone for noise.

#### Scenario: Phone locked in a pocket

- **WHEN** a noise event arrives while the parent phone is locked and the alert style is "Sound"
- **THEN** the phone vibrates, the full-screen alert appears over the lock screen and the child's audio plays

### Requirement: Noise event lifecycle

While a noise event is not stopped, the parent SHALL repeat the noise vibration every 4 seconds. "Stop sound" SHALL end audio and vibration for the current event, while the visual alert state stays until the level has been quiet for 2 minutes. New noise within 2 minutes after an event ended SHALL continue that event (audio resumes, one short vibration, no new full-screen alert); later noise SHALL start a new full alert.

#### Scenario: Baby fusses again after a minute

- **WHEN** the baby is quiet for 1 minute after an event and then cries again
- **THEN** the same event continues with audio and one short vibration, without a new full-screen alert

### Requirement: Silent mode and Do Not Disturb

Alert notification channels SHALL bypass Do Not Disturb once the user has granted the permission, and all alert audio SHALL use the alarm stream, which does not depend on the ringer mode. The app SHALL NOT change the device's ringer mode on the parent unit.

#### Scenario: Phone on silent

- **WHEN** the parent phone is on silent and a noise event arrives
- **THEN** the alert vibrates, shows and plays on the alarm stream at the alarm volume

### Requirement: Alert output and gain

The parent SHALL choose the alert output: "System default" (follows system routing, e.g. headphones) or "Speaker only". An alert playback gain SHALL scale the alert audio after decoding; the system alarm volume stays the ceiling.

#### Scenario: Headphones connected with "Speaker only"

- **WHEN** headphones are connected and the output is "Speaker only"
- **THEN** alert audio plays on the phone speaker

### Requirement: Test alert

The parent SHALL be able to run a test alert. The baby unit SHALL inject a digital test event into its alert pipeline without playing anything on the baby phone, and the parent SHALL run the noise-alert chain of the active alert style labelled as a test and name that style, then ask whether the user heard and felt it. The time of the last test SHALL be stored.

#### Scenario: Test before going to bed

- **WHEN** the parent runs a test alert while connected
- **THEN** the parent phone runs the full alert chain marked as a test and the baby phone stays silent

#### Scenario: Test in discreet style

- **WHEN** the parent runs a test alert while the alert style is "Discreet"
- **THEN** the test sheet says the style is "Discreet", the phone vibrates and shows the test alert, and no audio plays

### Requirement: Readiness check

The parent unit SHALL check at start, on resume and every 5 minutes whether notifications are allowed, the alarm volume is above zero, full-screen alerts are allowed, Do Not Disturb bypass is granted, battery optimisation is off, its own battery is at least 20 % or charging, no headphones are connected, and a test alert ran within the last 30 days. The worst failing item SHALL be shown with a way to fix it.

#### Scenario: Alarm volume at zero

- **WHEN** the parent's alarm volume is set to zero
- **THEN** the parent sees a readiness warning with a fix within 5 minutes

### Requirement: Snooze

The parent SHALL be able to snooze noise alerts for 10 or 30 minutes and end a snooze early. Snooze SHALL silence noise sound, vibration and full-screen alerts only; it SHALL NOT silence connection levels, a stopped baby unit, battery or readiness warnings. Snooze SHALL be stored as an end time that survives reconnects and app restarts. When a snooze ends while noise is above the threshold, a full alert SHALL start immediately.

#### Scenario: Snooze ends during crying

- **WHEN** a 10-minute snooze ends while the baby is still crying
- **THEN** the full noise alert starts immediately

### Requirement: Baby battery warnings

The core SHALL report a low-battery warning once when the baby unit falls below 15 % while not charging, and again only after it was charging or back at 15 % or more. It SHALL report a critical warning below 5 % regardless of charging, and SHALL report when the baby unit is unplugged while monitoring. Battery warnings SHALL NOT be snoozable.

#### Scenario: Charger pulled out

- **WHEN** someone unplugs the baby phone during monitoring
- **THEN** the parent is told that the baby unit is no longer charging

### Requirement: Monitoring visibility without push services

While monitoring, the parent unit SHALL run a foreground service with a persistent notification, so a killed app is noticeable. Alerts SHALL NOT depend on a push service (no FCM or APNs).

#### Scenario: App killed by the system

- **WHEN** the system kills the parent app process
- **THEN** the persistent monitoring notification disappears and the service restarts monitoring from the stored pairing

### Requirement: Alert style

The parent SHALL choose an alert style for noise alerts: "Sound" (default) or "Discreet". The style SHALL be stored on the parent device, SHALL NOT be part of any pairing material, and SHALL be changeable from the parent monitor screen in at most two taps. While "Discreet" is active, the parent monitor screen SHALL show a persistent, clearly visible indicator that alerts make no sound.

#### Scenario: Switching to discreet before an event

- **WHEN** the parent opens More on the monitor screen and chooses "Discreet"
- **THEN** the monitor screen shows the indicator "Discreet alerts: no sound" and the choice is still active after the app restarts

### Requirement: Discreet noise alert

With the alert style "Discreet" the parent unit SHALL NOT play the child's audio for a noise event until the parent starts Listen on the alert or the escalation to sound fires. The vibration repeat, the full-screen alert, the level meter and the quiet rule SHALL be the same as for "Sound", and Listen SHALL then play the audio on the chosen alert output.

#### Scenario: Discreet alert at a party

- **WHEN** a noise event arrives while the alert style is "Discreet"
- **THEN** the phone vibrates, the full-screen alert with the level meter appears and no audio plays until the parent taps Listen

#### Scenario: Back to sound during an event

- **WHEN** the parent switches from "Discreet" to "Sound" during a running event that is not snoozed
- **THEN** the child's audio starts playing at once

#### Scenario: Listen after a discreet alert

- **WHEN** the parent taps Listen on a discreet alert
- **THEN** the child's audio plays on the chosen alert output

### Requirement: Discreet escalation to sound

In "Discreet", a noise event that stays unacknowledged SHALL switch to sound after a configurable delay of 1, 2 (default) or 5 minutes: the child's audio plays as in "Sound" and the alert states "Switched to sound: no reaction for {minutes} min". The timer SHALL run in the core.

#### Scenario: Missed vibration

- **WHEN** the style is "Discreet" with the default delay and the parent does not react to a noise event for 2 minutes
- **THEN** the child's audio starts playing and the alert shows that it switched to sound

### Requirement: Escalation off warning

The parent MAY choose escalation "Off", and choosing it SHALL show a warning that a forgotten discreet mode can then stay silent all night.

#### Scenario: Escalation off (failure)

- **WHEN** the escalation is "Off" and the noise lasts 10 minutes without a reaction
- **THEN** no audio plays, and the setting showed the warning when it was chosen

### Requirement: Escalation acknowledgment

Listen, Stop sound or Stop vibration, and Snooze SHALL acknowledge a discreet noise event, and a new event after the 2-minute quiet rule SHALL restart the count.

#### Scenario: Escalated event ends listening

- **WHEN** the event switched to sound and the parent then ends Listen
- **THEN** the audio keeps playing until the event ends

#### Scenario: Parent reacts in time

- **WHEN** the parent taps Stop vibration after 1 minute
- **THEN** the audio does not start for that event

### Requirement: Alert style affects noise alerts only

The alert style SHALL affect noise alerts only: connection levels 1 to 3, a stopped baby unit, battery warnings, readiness warnings and snooze SHALL behave the same in both styles.

#### Scenario: Connection lost while discreet (failure)

- **WHEN** the alert style is "Discreet" and the connection to the baby unit reaches level 3
- **THEN** the audible alarm with Do Not Disturb bypass starts as in "Sound"
