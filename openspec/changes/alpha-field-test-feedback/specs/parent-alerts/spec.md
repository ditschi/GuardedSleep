## MODIFIED Requirements

### Requirement: Test alert

The parent SHALL be able to run a test alert. The baby unit SHALL inject a digital test event into its alert pipeline without playing anything on the baby phone, and the parent SHALL run the noise-alert chain of the active alert style through the same notification channel, full-screen path and audio route as a real alert, name that style, then ask whether the user heard and felt it. Only the copy and the bookkeeping SHALL differ: the parent SHALL present the test as a confirmation ("Test alert received"), never as the child making sound, and the test SHALL NOT be shown as a detected sound, SHALL NOT start or join a noise event and SHALL NOT appear as the last sound. The time of the last test SHALL be stored.

#### Scenario: Test before going to bed

- **WHEN** the parent runs a test alert while connected
- **THEN** the parent phone runs the full alert chain, shows "Test alert received" and the baby phone stays silent

#### Scenario: Test in discreet style

- **WHEN** the parent runs a test alert while the alert style is "Vibrate only"
- **THEN** the test sheet says the style is "Vibrate only", the phone vibrates and shows the test confirmation, and no audio plays

#### Scenario: Test on a locked phone

- **WHEN** the parent runs a test alert and locks the phone before it arrives
- **THEN** the test appears full screen over the lock screen like a real alert

#### Scenario: After the test

- **WHEN** a test alert has finished and the room stayed quiet
- **THEN** the monitor shows no "Sound detected" or "Sound at …" for the test, only that alerts were tested at that time

#### Scenario: Real sound during a test (failure guard)

- **WHEN** the baby unit detects a real sound while a test alert is shown
- **THEN** the parent raises the real noise alert as usual

### Requirement: Baby battery warnings

The core SHALL report a low-battery warning once when the baby unit falls below 15 % while not charging, and again only after it was charging or back at 15 % or more. It SHALL report a critical warning below 5 % regardless of charging, and SHALL report once when the baby unit stops charging during monitoring after it was charging. Battery warnings SHALL NOT be snoozable. The parent SHALL show the baby unit's battery as one indicator (the level, with a charging mark only while it charges) and SHALL NOT show a separate permanent "not charging" notice.

#### Scenario: Charger pulled out

- **WHEN** someone unplugs the charging baby phone during monitoring
- **THEN** the parent is told once that the baby unit is no longer charging

#### Scenario: Baby device never charged

- **WHEN** the baby unit runs on battery from the start of monitoring
- **THEN** the parent shows only the battery level, without a "no longer charging" notice

#### Scenario: Battery runs low on a device that never charged (failure)

- **WHEN** the baby unit falls below 15 % without ever charging
- **THEN** the low-battery warning appears as usual
