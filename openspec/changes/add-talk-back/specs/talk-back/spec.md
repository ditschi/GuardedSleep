# Spec Delta

## Purpose

Let a parent speak to the child through the baby unit's loudspeaker while holding a talk button, only over a trusted, pinned link, briefly, gently, accountably and without ever hiding the child's crying or raising a false noise alert.

## ADDED Requirements

### Requirement: Push-to-talk

The parent unit SHALL offer a talk button on the monitoring screen while the link is connected and the device is unlocked, and talk SHALL run only while the button is held. Pressing SHALL ask the baby unit for a talk; the parent's microphone SHALL be sent only after the grant and only until release; releasing SHALL end the talk at once. There SHALL be no way to keep talking without holding the button.

#### Scenario: Parent reassures the child

- **WHEN** the parent holds the talk button on a connected link and the baby unit grants the talk
- **THEN** the parent unit shows "Speak now", and the parent's voice plays on the baby unit until the button is released

#### Scenario: Not connected

- **WHEN** the link is not connected
- **THEN** the talk button is unavailable and pressing it sends nothing

#### Scenario: Released before the grant

- **WHEN** the parent releases the button before the baby unit granted the talk
- **THEN** the parent's microphone is never sent, and the baby unit stops its chime and plays nothing else

#### Scenario: No answer from the baby unit (failure)

- **WHEN** neither a grant nor a denial arrives within 3 seconds of the press
- **THEN** the parent unit ends the request and says that the baby device did not answer

### Requirement: Talk ends when the hold is interrupted

The parent unit SHALL end a talk and close its microphone at once when the touch is cancelled, the app leaves the foreground, the screen turns off, a phone call starts, the microphone fails or its permission is revoked.

#### Scenario: Incoming call mid-hold

- **WHEN** a call arrives on the parent phone while the parent holds the talk button
- **THEN** the talk ends, the microphone closes and the baby unit stops playing

#### Scenario: Locked parent device (failure)

- **WHEN** someone touches the screen of the parent device over its lock screen
- **THEN** no talk request is sent and the baby unit plays nothing

### Requirement: Microphone permission before the first talk

The parent unit SHALL ask for microphone permission in a separate step before the first talk, never during a hold. A press without permission SHALL send no talk request and SHALL offer the permission step instead.

#### Scenario: First press

- **WHEN** the parent presses the talk button for the first time without microphone permission
- **THEN** no talk request is sent, and a sheet explains why the microphone is needed and asks for it

#### Scenario: Permission refused

- **WHEN** the parent refuses microphone permission
- **THEN** no talk request is sent, and the parent unit says that talk-back needs the microphone and how to allow it

### Requirement: Half duplex

While the parent holds the talk button, the parent unit SHALL NOT play the baby's audio and SHALL keep showing the live level meter. After release it SHALL resume the previous listening state; if the parent was not listening, it SHALL listen for 10 seconds so the parent hears the child's reaction.

#### Scenario: Parent was listening

- **WHEN** the parent listens with the lock on, holds the talk button and releases it
- **THEN** the baby's audio is silent on the parent unit while the button is held and plays again after release

#### Scenario: Reaction after the talk

- **WHEN** the parent was not listening and releases the talk button
- **THEN** the parent unit listens for 10 seconds and then stops, unless the parent keeps listening

### Requirement: Talk only over a trusted, pinned link

The baby unit SHALL grant a talk only to a parent link that is trusted and pinned: the attempt verified, a parent secret pinned for this pairing and its proof passed on this attempt, and the link alive. Talk audio SHALL travel only on that attempt's media, encrypted by DTLS-SRTP. No talk message SHALL be accepted through a mailbox.

#### Scenario: Stranger in the room

- **WHEN** a client without the pairing key sends a talk request through any mailbox
- **THEN** the baby unit ignores it and plays nothing

#### Scenario: Verified attempt before the pin (failure)

- **WHEN** a talk request arrives on a verified attempt before any parent secret is pinned
- **THEN** the baby unit ignores it and plays nothing

### Requirement: Playback only for the granted link

The baby unit SHALL play talk audio only from the talk line of the attempt whose talk it granted, and only while that talk runs. Talk receivers SHALL start disabled before any audio can play, every other talk receiver SHALL stay disabled during a talk, and talk audio received outside a grant SHALL be discarded and counted as a security warning, without its content.

#### Scenario: Audio without a grant (failure)

- **WHEN** audio arrives on a talk line while no talk is granted on that attempt
- **THEN** the baby unit plays none of it and records a security warning

#### Scenario: Second parent streams during another talk (failure)

- **WHEN** parent A's talk is granted and parent B's attempt sends audio on its talk line
- **THEN** only A's audio plays

### Requirement: Talk ends with the link

The baby unit SHALL end a talk when its attempt closes or is declared lost, when monitoring stops or the link is released, and at the latest at the talk deadline. Every grant SHALL carry a playback deadline that the baby unit's media layer enforces by itself, so the speaker turns silent at the deadline even if a stop instruction is lost.

#### Scenario: Link lost during a talk (failure)

- **WHEN** the attempt closes or liveness declares it lost while a talk is running
- **THEN** the baby unit stops playing, and the talk ends on both devices

#### Scenario: Stop instruction lost (failure)

- **WHEN** the baby unit's media layer misses the instruction to stop a talk
- **THEN** the speaker turns silent at the talk deadline on its own

#### Scenario: Baby app killed mid-talk (failure)

- **WHEN** the baby app is killed while a talk plays
- **THEN** the speaker is silent, and at the next start any volume the app changed is restored

### Requirement: Talk limits

The baby unit SHALL end a talk by itself 30 seconds after granting it, independent of the parent. The parent unit SHALL show the remaining time and give a haptic signal 5 seconds before the end. The baby unit SHALL accept at most one talk request per link per second.

#### Scenario: Button stuck or parent app frozen (failure)

- **WHEN** the parent unit stops sending anything 5 seconds into a talk
- **THEN** the baby unit stops playing at the latest 30 seconds after the grant

#### Scenario: Long message

- **WHEN** the parent holds the button for 40 seconds
- **THEN** the talk ends after 30 seconds, and the parent unit says that the limit was reached and when the button can be held again

### Requirement: Talk cooldown and budget

After a talk's noise pause ends, the baby unit SHALL deny new talk requests from every parent link until the noise gate has run unpaused for at least the hold time plus 1 second and at least 10 seconds. Across all parent links, talks SHALL pause noise detection for at most 90 seconds within any 10 minutes; a request beyond that budget SHALL be denied. A talk released before its grant SHALL count toward both.

#### Scenario: Chained presses while the baby cries (failure)

- **WHEN** one or two parents press and release the talk button as often as they can for 10 minutes and the baby starts crying at minute 3
- **THEN** a noise alert reaches the parents no later than one talk limit plus the tail plus the cooldown after the cry started

#### Scenario: Budget used up

- **WHEN** talks have paused noise detection for 90 seconds within the last 10 minutes
- **THEN** the next talk request is denied, the parent sees when talk-back is available again, and noise detection keeps running

### Requirement: One talker at a time

The baby unit SHALL run at most one talk at a time. A talk request from another parent link while a talk runs SHALL be denied, and every other trusted parent SHALL be told that a parent is talking and when it ends.

#### Scenario: Both parents press

- **WHEN** parent A is talking and parent B presses the talk button
- **THEN** B is denied, B's screen says that the other parent device is talking, and A's talk continues

### Requirement: Gentle start and safe loudness

Before the parent's voice, the baby unit SHALL play a soft chime and then fade the voice in, at the talk-back volume set on the baby unit and independent of the ringer mode. The baby unit SHALL cap the loudness of talk audio by itself, independent of the sender: it SHALL lower the gain within 200 ms when the talk level exceeds a fixed ceiling, and its highest volume level SHALL stay below the device's maximum alarm volume. The parent unit SHALL NOT change the baby unit's volume.

#### Scenario: Baby phone ringer off

- **WHEN** the baby phone's ringer is off and a talk is granted
- **THEN** the chime and the voice play at the talk-back volume, and the ringer stays off

#### Scenario: Sender sends full-scale audio (failure)

- **WHEN** talk audio jumps from silence to full scale
- **THEN** the baby unit lowers the gain within 200 ms and never plays above its ceiling

### Requirement: Volume restored after a talk

The baby unit SHALL restore any system volume it changed for a talk when the talk ends, but only if the user has not changed that volume meanwhile.

#### Scenario: After the talk

- **WHEN** a talk ends and nobody touched the volume
- **THEN** the baby unit's volumes are as they were before the talk

#### Scenario: User changed the alarm volume during a talk

- **WHEN** the user changes the alarm volume on the baby phone during a talk
- **THEN** the baby unit keeps the user's new volume when the talk ends

### Requirement: Honest playability

The baby unit SHALL report talk-back to every trusted parent as on, off, muted or not supported, and SHALL report muted, and deny requests as muted, when the device would not make the talk audible (alarm sounds blocked by the user's own Do Not Disturb, or the volume could not be set). Before a press, the parent unit SHALL show the talk button as unavailable with the reason whenever talk-back is not on.

#### Scenario: Baby phone in total silence

- **WHEN** the baby phone's own Do Not Disturb blocks alarm sounds
- **THEN** the parent's talk button says that the baby device cannot play sound right now, and no talk is granted

### Requirement: Alerts take precedence over talk

A talk press SHALL count as acknowledgment of the running noise event for that parent. Any alert sound or vibration on the parent unit SHALL end a running talk first and then play at its normal level. A test alert SHALL be unavailable while a talk runs. After a talk, the parent unit's alert audio SHALL play on the same output at the same level as before.

#### Scenario: Battery warning during a talk

- **WHEN** a baby battery warning with sound is due on the parent unit while the parent holds the talk button
- **THEN** the talk ends, the microphone closes, and then the warning plays

#### Scenario: Alert audio after a talk (failure)

- **WHEN** a noise alert plays on the parent unit after a talk ended
- **THEN** it plays on the same output and at the same level as before the talk

### Requirement: Talk-back indicators and accountability

While it plays a talk, the baby unit SHALL show a static, dim microphone indicator with "Parent is talking" that does not wake the kiosk screen to normal brightness. Its touch-wake status SHALL name the last talk with the parent device's local connection name, time and duration, kept only on the device. The parent unit SHALL show the talk state: one moment, speak now, ended with the reason, or denied with the reason.

#### Scenario: Adult in the baby room

- **WHEN** an adult wakes the baby unit's screen after a parent talked
- **THEN** the status shows which parent device talked, when and for how long

### Requirement: No speech into the open microphone

While its microphone is open, the parent unit SHALL make no spoken screen-reader announcements and play no sounds of its own, and SHALL signal grant, 5 seconds left and end by haptics and visible state.

#### Scenario: Screen reader on the parent

- **WHEN** a parent uses a screen reader and holds the talk button
- **THEN** nothing is spoken by the parent device until the talk ends, and the grant is felt as a haptic signal

### Requirement: Talk-back settings

The baby unit's Basic settings SHALL let an adult switch talk-back off and on for each saved parent pairing and choose its volume (low, medium or high), with a button that plays the chime at the chosen volume. These settings SHALL be changeable only on the baby unit; no message from a parent SHALL change them. Switching talk-back off during a talk SHALL end it at once.

#### Scenario: Silent baby unit wanted

- **WHEN** an adult switches talk-back off on the baby unit
- **THEN** the baby unit denies every talk request and the parent's talk button shows that talk-back is off on the baby device

#### Scenario: Switched off during a talk

- **WHEN** an adult switches talk-back off while a parent talks
- **THEN** the playback stops at once and the parent sees that talk-back was switched off

#### Scenario: Remote change attempt (failure)

- **WHEN** a trusted parent sends any message that tries to turn talk-back on or change its volume
- **THEN** the baby unit's settings stay unchanged

### Requirement: No recording of talk audio

Neither device SHALL store, record or log talk audio, and talk audio SHALL NOT be kept beyond what the media pipeline needs to play it. The parent's microphone SHALL be open only while the talk button is held.

#### Scenario: After a talk

- **WHEN** a talk has ended
- **THEN** no talk audio exists on either device, and the parent's microphone is closed
