# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Snooze`
- TO: `### Requirement: Pause noise alerts`
- FROM: `### Requirement: Discreet noise alert`
- TO: `### Requirement: Vibrate-only noise alert`
- FROM: `### Requirement: Discreet escalation to sound`
- TO: `### Requirement: Vibrate-only escalation to sound`

## MODIFIED Requirements

### Requirement: Noise alert chain

When the baby unit reports a noise event, the parent unit SHALL vibrate, turn the screen on, show a full-screen alert over the lock screen when the app is not in front (if the platform allows full-screen alerts), and show the live level meter. With the alert style "Sound" (for "Vibrate only" see below) it SHALL also play the child's audio on the alarm stream through the chosen alert output; the child's audio SHALL be the alert, and there SHALL be no separate alarm ringtone for noise.

#### Scenario: Phone locked in a pocket

- **WHEN** a noise event arrives while the parent phone is locked and the alert style is "Sound"
- **THEN** the phone vibrates, the full-screen alert appears over the lock screen and the child's audio plays

### Requirement: Test alert

The parent SHALL be able to run a test alert. The baby unit SHALL inject a digital test event into its alert pipeline without playing anything on the baby phone, and the parent SHALL run the noise-alert chain of the active alert style labelled as a test and name that style, then ask whether the user heard and felt it. The time of the last test SHALL be stored.

#### Scenario: Test before going to bed

- **WHEN** the parent runs a test alert while connected
- **THEN** the parent phone runs the full alert chain marked as a test and the baby phone stays silent

#### Scenario: Test in discreet style

- **WHEN** the parent runs a test alert while the alert style is "Vibrate only"
- **THEN** the test sheet says the style is "Vibrate only", the phone vibrates and shows the test alert, and no audio plays

### Requirement: Readiness check

The parent unit SHALL check at start, on resume and every 5 minutes, in this order of severity: notifications allowed, full-screen alerts allowed, alarm volume above zero, Do Not Disturb bypass granted, battery optimisation off, its own battery at least 20 % or charging, no headphones connected, a test alert within the last 30 days. The worst failing item SHALL be shown with one way to fix it.

#### Scenario: Alarm volume at zero

- **WHEN** the parent's alarm volume is set to zero
- **THEN** the parent sees a readiness warning with a fix within 5 minutes

#### Scenario: Two items fail

- **WHEN** notifications are off and the parent's battery is at 15 %
- **THEN** the readiness banner names the notifications first, with the fix that opens the notification setting

### Requirement: Pause noise alerts

The parent SHALL be able to pause noise alerts for 10 or 30 minutes and end a pause early; a pause started while no noise event runs SHALL offer Undo for 8 seconds. A pause SHALL silence noise sound, vibration and full-screen alerts only; it SHALL NOT silence connection levels, a stopped baby unit, battery or readiness warnings. A pause SHALL be stored as an end time that survives reconnects and app restarts. When a pause ends while noise is above the threshold, a full alert SHALL start immediately.

#### Scenario: Snooze ends during crying

- **WHEN** a 10-minute pause ends while the baby is still crying
- **THEN** the full noise alert starts immediately

#### Scenario: Paused by mistake

- **WHEN** no noise event runs and the parent taps "Pause noise alerts" on the monitor and then "Undo" within 8 seconds
- **THEN** the pause ends and noise alerts are active again

#### Scenario: Pause from a running alert

- **WHEN** the parent pauses noise alerts from a running vibrate-only alert
- **THEN** no Undo is offered, because the pause has acknowledged that event

### Requirement: Alert style

The parent SHALL choose an alert style for noise alerts: "Sound" (default) or "Vibrate only". The style SHALL be stored on the parent device, SHALL NOT be part of any pairing material, and SHALL be changeable from the parent monitor screen in at most two taps. While "Vibrate only" is active, the monitor SHALL show a persistent "Vibrate only" banner, or, while it is folded, the labelled chip "Alert: Vibrate only"; its text SHALL say that noise alerts make no sound, or with escalation Off never do.

#### Scenario: Switching to discreet before an event

- **WHEN** the parent opens the alert chip on the monitor screen and chooses "Vibrate only"
- **THEN** the monitor screen shows the "Vibrate only" indicator and the choice is still active after the app restarts

#### Scenario: Indicator folded

- **WHEN** "Vibrate only" is active and two banners of higher priority are shown
- **THEN** the "Vibrate only" banner folds into "+N more" and the alert chip shows "Alert: Vibrate only" in words

### Requirement: Vibrate-only noise alert

With the alert style "Vibrate only" the parent unit SHALL NOT play the child's audio for a noise event until the parent starts Listen on the alert or the escalation to sound fires. The vibration repeat, the full-screen alert, the level meter and the quiet rule SHALL be the same as for "Sound", and Listen SHALL then play the audio on the chosen alert output.

#### Scenario: Discreet alert at a party

- **WHEN** a noise event arrives while the alert style is "Vibrate only"
- **THEN** the phone vibrates, the full-screen alert with the level meter appears and no audio plays until the parent taps Listen

#### Scenario: Back to sound during an event

- **WHEN** the parent switches from "Vibrate only" to "Sound" during a running event that is not paused
- **THEN** the child's audio starts playing at once

#### Scenario: Listen after a discreet alert

- **WHEN** the parent taps Listen on a vibrate-only alert
- **THEN** the child's audio plays on the chosen alert output

### Requirement: Vibrate-only escalation to sound

In "Vibrate only", a noise event that stays unacknowledged SHALL switch to sound after a configurable delay of 1, 2 (default) or 5 minutes: the child's audio plays as in "Sound" and the alert states "Switched to sound: no reaction for {minutes} min". The timer SHALL run in the core.

#### Scenario: Missed vibration

- **WHEN** the style is "Vibrate only" with the default delay and the parent does not react to a noise event for 2 minutes
- **THEN** the child's audio starts playing and the alert shows that it switched to sound

### Requirement: Escalation off warning

The parent MAY choose escalation "Off", and choosing it SHALL show a warning that a forgotten "Vibrate only" can then stay silent all night.

#### Scenario: Escalation off (failure)

- **WHEN** the escalation is "Off" and the noise lasts 10 minutes without a reaction
- **THEN** no audio plays, and the setting showed the warning when it was chosen

### Requirement: Alert style affects noise alerts only

The alert style SHALL affect noise alerts only: connection levels 1 to 3, a stopped baby unit, battery warnings, readiness warnings and pausing noise alerts SHALL behave the same in both styles.

#### Scenario: Connection lost while discreet (failure)

- **WHEN** the alert style is "Vibrate only" and the connection to the baby unit reaches level 3
- **THEN** the audible alarm with Do Not Disturb bypass starts as in "Sound"

## ADDED Requirements

### Requirement: Full-screen alert template

Every full-screen alert (noise, connection from level 2, baby battery critical) SHALL use one template: a label saying what the phone does, a headline, a counter or figure, one sentence on what happens next, and one filled action pinned while the body scrolls. Each kind SHALL keep a fixed dark palette; the red palettes (`alert`, `noConnection`, `loudAlarm`) SHALL be used only when the child cannot be heard. Sound and vibration stay as built.

#### Scenario: Large text

- **WHEN** the system text size is 200 % and a noise alert is shown
- **THEN** the body scrolls and the main action stays fully visible at the bottom

#### Scenario: Baby battery almost empty

- **WHEN** the baby device's battery falls below 5 %
- **THEN** the parent shows the alert template on the fixed amber palette, not on red

### Requirement: Banner priority on the monitor

The parent monitor SHALL order its banners as: security warning, microphone silent, readiness, baby battery low or unplugged, baby device may ring (silence status), weak connection, noise alerts paused, Vibrate only. It SHALL show at most two banners and fold the rest into one "+N more" row that lists them. Each new banner of warning level or higher SHALL be announced once.

#### Scenario: Four notices at once

- **WHEN** the microphone is silent, the alarm volume is zero, the baby battery is low and "Vibrate only" is on
- **THEN** the monitor shows the microphone and readiness banners and a "+2 more" row for the others

### Requirement: Readiness on the monitor

While monitoring, the parent monitor SHALL show the worst failing readiness item in its banner slot with a "Fix" action that opens the matching setting. When the alarm volume is above zero but below half, it SHALL show a hint to raise it, ranked below every failing item.

#### Scenario: Notifications switched off at night

- **WHEN** the parent turns notifications off while monitoring
- **THEN** within 5 minutes the monitor shows "Notifications are off" with "Fix"

### Requirement: Vibrate-only reminder at start

While the alert style is "Vibrate only", the parent connections on Home SHALL carry one line saying that noise alerts will only vibrate, so the parent sees it before tapping Start. It SHALL NOT ask for confirmation, block the start or change any alert behaviour.

#### Scenario: Forgotten after a party

- **WHEN** the alert style is still "Vibrate only" from the evening before and the parent opens Home
- **THEN** a line under the parent connections says noise alerts will only vibrate, and Start begins monitoring with one tap
