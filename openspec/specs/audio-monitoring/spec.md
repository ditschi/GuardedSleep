# audio-monitoring Specification

## Purpose

Detect the child's noise on the baby unit, bring the child's audio to the parent only when needed and only in that direction, and never make a sound, vibration or light next to the child. Migrated from ADR-003 §1, ADR-018 §2–§3 and the audio, communication and UI feature docs, as built in the core and on Android.

## Requirements

### Requirement: Local noise detection

While monitoring, the baby unit SHALL capture audio continuously and analyse it on the device. The noise gate SHALL turn loud when the level stays at or above the threshold for the hold time, and quiet as soon as the level falls below it. Changing the threshold or hold time SHALL NOT fire an alert by itself.

#### Scenario: Door slams

- **WHEN** a single loud sound lasts shorter than the hold time
- **THEN** no noise event is sent

#### Scenario: Baby cries

- **WHEN** the level stays above the threshold for the hold time
- **THEN** the baby unit sends a noise event with its level to the parent

### Requirement: Sensitivity settings

The baby unit SHALL let the user set the threshold with a slider and a live level preview, and the hold time to 3, 5, 10 or 15 seconds (default 5).

#### Scenario: Setting up a noisy room

- **WHEN** the user moves the threshold slider while the room is quiet
- **THEN** the live preview shows the current level against the new threshold

### Requirement: Audio only on demand

The baby unit SHALL send audio only while the noise gate is loud, the parent is listening, or a test alert runs; otherwise the pre-negotiated audio track SHALL stay muted. Audio SHALL flow only from the baby unit to the parent unit, encrypted by DTLS-SRTP.

#### Scenario: Quiet night

- **WHEN** the room is quiet and the parent is not listening
- **THEN** no audio from the baby unit is sent

### Requirement: Listen mode

The parent SHALL listen by holding the listen button and SHALL keep listening by sliding up to lock; a tap SHALL stop locked listening. Locked listening SHALL stop by itself after 60 minutes, and listening SHALL be unavailable while not connected. The baby unit SHALL show "Parent is listening" only when someone wakes its screen.

#### Scenario: Forgotten open microphone

- **WHEN** the parent locks listening and forgets it
- **THEN** listening stops after 60 minutes

### Requirement: Parent monitoring screen

While connected, the parent screen SHALL show a live level meter, the baby unit's battery and charging state, and "All quiet since {time}" when there was no recent noise. "All quiet" SHALL NOT be shown while disconnected, and after a recovery only when the missed-events report covers the whole outage.

#### Scenario: Reconnected after the baby process restarted

- **WHEN** the link recovers and the missed-events report is incomplete
- **THEN** the parent does not see "All quiet since" for the outage

### Requirement: Microphone health

The core SHALL report `mic_silent` when the baby unit has received only digital silence for 30 seconds while monitoring, and the parent SHALL show it.

#### Scenario: Microphone blocked by another app

- **WHEN** the baby phone delivers only digital silence for 30 seconds
- **THEN** the parent sees that the baby unit's microphone hears nothing

### Requirement: Silent baby unit

GuardedSleep SHALL never produce sound, vibration or haptics on the baby unit: no audio playout (the baby's SDP must be send-only), only silent minimum-importance notification channels, no alert sounds and no text-to-speech.

#### Scenario: Parent talks into the phone

- **WHEN** the parent phone's microphone picks up sound during listening
- **THEN** nothing is played on the baby unit

### Requirement: Baby phone Do Not Disturb

With a one-time Do Not Disturb access grant, the baby unit SHALL activate, while monitoring, an automatic rule that allows alarms only and suppresses visual interruptions; this SHALL be on by default once access is granted. Stopping SHALL restore the user's own mode, and a rule left over after a crash SHALL be removed at the next start and the user told.

#### Scenario: App crashed with the rule active

- **WHEN** the app starts and finds its rule still active without monitoring
- **THEN** it removes the rule and tells the user that sounds are back on

### Requirement: Silence status on the parent

The baby unit SHALL report its ringer mode (vibrate counts as not silent), Do Not Disturb state, whether alarms are allowed, the time to the next alarm clock, its Do Not Disturb access, `mic_silent`, battery and charging. The parent SHALL show one silence value with a fix, and always the caveat that alarms and emergency alerts on the baby phone can still sound.

#### Scenario: Baby phone on vibrate

- **WHEN** the baby phone is on vibrate without the app's rule
- **THEN** the parent sees that the baby phone may buzz, with how to fix it

### Requirement: Remote silence request

The parent SHALL be able to request silencing on the baby unit, but never to turn it off. A request SHALL be accepted only on a verified attempt, at most once every 10 seconds, and SHALL be shown on the baby unit's screen. When the baby phone is also someone's everyday phone, the baby screen SHALL say that calls and messages stay silent until monitoring stops.

#### Scenario: Silence lost during the night

- **WHEN** the baby unit lost its silence rule and the parent requests silencing
- **THEN** the baby unit re-applies the rule and shows that the parent requested it
