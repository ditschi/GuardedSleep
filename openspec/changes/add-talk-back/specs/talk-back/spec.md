# Spec Delta

## Purpose

Let a parent speak to the child through the baby unit's loudspeaker while holding a talk button, only over a trusted link, briefly, gently and visibly, without ever raising a false noise alert.

## ADDED Requirements

### Requirement: Push-to-talk

The parent unit SHALL offer a talk button on the monitoring screen while the link is connected, and talk SHALL run only while the button is held. Pressing SHALL ask the baby unit for a talk; the parent's microphone SHALL be sent only after the grant and only until release; releasing SHALL end the talk at once. There SHALL be no way to keep talking without holding the button.

#### Scenario: Parent reassures the child

- **WHEN** the parent holds the talk button on a connected link and the baby unit grants the talk
- **THEN** the parent unit shows that it is talking, and the parent's voice plays on the baby unit until the button is released

#### Scenario: Not connected

- **WHEN** the link is not connected
- **THEN** the talk button is unavailable and pressing it sends nothing

#### Scenario: Released before the grant

- **WHEN** the parent releases the button before the baby unit granted the talk
- **THEN** the parent's microphone is never sent and the baby unit plays nothing, or stops its chime

### Requirement: Half duplex

While the parent holds the talk button, the parent unit SHALL NOT play the baby's audio, and it SHALL resume the previous listening state after release.

#### Scenario: Parent was listening

- **WHEN** the parent listens with the lock on, holds the talk button and releases it
- **THEN** the baby's audio is silent on the parent unit while the button is held and plays again after release

### Requirement: Talk only over a trusted link

The baby unit SHALL grant a talk only to a parent link that is trusted: its attempt verified, the parent pin-proven, and the link alive. Talk audio SHALL travel only on the verified attempt's media, encrypted by DTLS-SRTP. The baby unit SHALL play received talk audio only while a talk it granted on that link is running, and SHALL discard talk audio at any other time.

#### Scenario: Stranger in the room

- **WHEN** a client without the pairing key sends a talk request through any mailbox
- **THEN** the baby unit ignores it and plays nothing

#### Scenario: Audio without a grant (failure)

- **WHEN** audio arrives on the parent→baby direction while no talk is granted on that link
- **THEN** the baby unit plays none of it

#### Scenario: Link lost during a talk (failure)

- **WHEN** the attempt closes while a talk is running
- **THEN** the baby unit stops playing at once and the talk ends on both devices

### Requirement: Talk limits

The baby unit SHALL end a talk by itself 30 seconds after granting it, independent of the parent, and the parent unit SHALL show the remaining time for the last 5 seconds. The baby unit SHALL accept at most one talk request per link per second and SHALL deny requests while talk-back is off.

#### Scenario: Button stuck or parent app frozen (failure)

- **WHEN** the parent unit stops sending anything 5 seconds into a talk
- **THEN** the baby unit stops playing at the latest 30 seconds after the grant

#### Scenario: Long message

- **WHEN** the parent holds the button for 40 seconds
- **THEN** the talk ends after 30 seconds, and the parent can press again to continue

### Requirement: One talker at a time

The baby unit SHALL run at most one talk at a time. A talk request from another parent link while a talk runs SHALL be denied with the reason that another parent is talking, and every other trusted parent SHALL be told that a parent is talking and when it ends.

#### Scenario: Both parents press

- **WHEN** parent A is talking and parent B presses the talk button
- **THEN** B is denied, B's screen says that the other parent device is talking, and A's talk continues

### Requirement: Gentle start and volume

Before the parent's voice, the baby unit SHALL play a soft chime and then fade the voice in. It SHALL play at the talk-back volume set on the baby unit, independent of the ringer mode and the system media volume, and SHALL restore any volume it changed when the talk ends. The parent unit SHALL NOT change the baby unit's volume.

#### Scenario: Baby phone ringer off

- **WHEN** the baby phone's ringer is off and a talk is granted
- **THEN** the chime and the voice play at the talk-back volume, and the ringer stays off

#### Scenario: After the talk

- **WHEN** a talk ends
- **THEN** the baby unit's volumes are as they were before the talk

### Requirement: Talk-back indicators

While it plays a talk, the baby unit SHALL show a dim "Parent is talking" indicator that does not wake the kiosk screen to normal brightness, and its touch-wake status SHALL name it. The parent unit SHALL show the talk state: waiting for the baby unit, talking, ended, or denied with the reason (talk-back is off on the baby unit, the other parent device is talking, the baby device cannot play talk-back).

#### Scenario: Adult in the baby room

- **WHEN** an adult looks at the baby unit while a parent is talking
- **THEN** the dim indicator shows that a parent is talking

#### Scenario: Talk-back off on the baby unit

- **WHEN** the parent presses the talk button and talk-back is off on the baby unit
- **THEN** the parent sees that talk-back is switched off on the baby device and how to turn it on there

### Requirement: Talk-back settings

The baby unit's Basic settings SHALL let an adult switch talk-back off and on and choose its volume (low, medium or high). The baby unit SHALL report whether talk-back is on, off or not supported to every trusted parent, which SHALL show the talk button as unavailable, with the reason, while it is not on. These settings SHALL be changeable only on the baby unit, never remotely.

#### Scenario: Silent baby unit wanted

- **WHEN** an adult switches talk-back off on the baby unit
- **THEN** the baby unit denies every talk request and the parent's talk button shows that talk-back is off on the baby device

#### Scenario: Volume changed during monitoring

- **WHEN** an adult sets the talk-back volume to low on the baby unit while monitoring
- **THEN** the next talk plays at the low volume

### Requirement: No recording of talk audio

Neither device SHALL store, record or log talk audio, and talk audio SHALL NOT be kept beyond what the media pipeline needs to play it. The parent's microphone SHALL be open only while the talk button is held.

#### Scenario: After a talk

- **WHEN** a talk has ended
- **THEN** no talk audio exists on either device, and the parent's microphone is closed
