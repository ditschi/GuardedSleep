## MODIFIED Requirements

### Requirement: Microphone permission before the first talk

On Android, the parent unit SHALL take the microphone permission from the first-start permission screen; the web client SHALL ask at the first talk. If it is still missing at a press, the parent unit SHALL ask for it in a separate step, never during a hold; a press without permission SHALL send no talk request and SHALL offer the permission step instead.

#### Scenario: Allowed on first start

- **WHEN** the parent allowed the microphone on the first-start screen and presses the talk button for the first time
- **THEN** the talk request is sent without a permission step

#### Scenario: First press without permission

- **WHEN** the parent presses the talk button without microphone permission
- **THEN** no talk request is sent, and a sheet explains why the microphone is needed and asks for it

#### Scenario: Permission refused

- **WHEN** the parent refuses microphone permission
- **THEN** no talk request is sent, and the parent unit says that talk-back needs the microphone and how to allow it

### Requirement: Gentle start

Before the parent's voice, the baby unit SHALL play a soft chime, unless the chime is switched off on the baby unit, and then fade the voice in, at the talk-back volume set on the baby unit and independent of the ringer mode. The chime and the voice SHALL play through the baby unit's built-in loudspeaker, never its earpiece; a connected Bluetooth or wired output MAY be used instead. The parent unit SHALL NOT change the baby unit's volume.

#### Scenario: Baby phone ringer off

- **WHEN** the baby phone's ringer is off and a talk is granted
- **THEN** the chime and the voice play at the talk-back volume, and the ringer stays off

#### Scenario: Loudspeaker, not earpiece (failure guard)

- **WHEN** a talk is granted on a baby phone with no headset connected
- **THEN** the voice comes out of the loudspeaker and is audible across the room at the default volume

### Requirement: Loudness cap on the baby unit

The baby unit SHALL cap the loudness of talk audio by itself, independent of the sender: it SHALL judge the incoming talk level before its own gain, SHALL lower the gain within 200 ms when that level exceeds a fixed ceiling, and its highest volume SHALL stay at or below about 70 % of the device's maximum alarm volume. The cap SHALL NOT lower the gain for speech at normal level.

#### Scenario: Sender sends full-scale audio (failure)

- **WHEN** talk audio jumps from silence to full scale
- **THEN** the baby unit lowers the gain within 200 ms and never plays above its ceiling

#### Scenario: Normal speech

- **WHEN** the parent speaks at a normal level close to the phone
- **THEN** the voice plays at the set volume without the cap lowering it

### Requirement: Talk-back settings

The baby unit's Basic settings SHALL let an adult switch talk-back off and on for each saved parent pairing (on by default), set its volume with a slider in steps of the alarm volume (about 10 % to 70 % of its maximum, about 50 % by default) with a button that plays the chime at the chosen volume, and switch the chime off and on (on by default). These settings SHALL be changeable only on the baby unit; no message from a parent SHALL change them. Switching talk-back off during a talk SHALL end it at once.

#### Scenario: Silent baby unit wanted

- **WHEN** an adult switches talk-back off on the baby unit
- **THEN** the baby unit denies every talk request and the parent's talk button shows that talk-back is off on the baby device

#### Scenario: Switched off during a talk

- **WHEN** an adult switches talk-back off while a parent talks
- **THEN** the playback stops at once and the parent sees that talk-back was switched off

#### Scenario: Remote change attempt (failure)

- **WHEN** a trusted parent sends any message that tries to turn talk-back on or change its volume
- **THEN** the baby unit's settings stay unchanged

#### Scenario: Trying the volume

- **WHEN** the adult moves the volume slider and taps the try button
- **THEN** the chime plays through the loudspeaker at the chosen volume
