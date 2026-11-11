# Spec Delta

## MODIFIED Requirements

### Requirement: Audio only on demand

The baby unit SHALL send audio only while the noise gate is loud, the parent is listening, or a test alert runs. Otherwise it SHALL detach the microphone track from the audio sender, so no RTP packets are sent, and SHALL keep the track disabled as well, so a failed detach can only send silence. Capture SHALL keep running while the track is detached, so the noise gate keeps hearing the room. Audio from the baby unit SHALL flow only to the parent unit. Audio from the parent unit SHALL flow to the baby unit only while a talk granted by the baby unit runs (`talk-back`); otherwise the parent unit SHALL send no audio and keep its microphone closed. Both directions SHALL be encrypted by DTLS-SRTP.

#### Scenario: Quiet night

- **WHEN** the room is quiet and the parent is not listening
- **THEN** the baby unit sends no RTP packets, and the noise gate keeps receiving microphone buffers

#### Scenario: Noise after a quiet hour is detected

- **WHEN** the room was quiet for an hour with the track detached and a baby cries
- **THEN** the core raises the noise event within the hold time

#### Scenario: Audio starts quickly after a quiet hour

- **WHEN** the noise gate turns loud after an hour without audio
- **THEN** the first audio packet leaves the baby unit within 250 ms of the gate change

#### Scenario: Parent not talking

- **WHEN** the parent is monitoring and does not hold the talk button
- **THEN** the parent unit sends no audio and its microphone is closed

### Requirement: Silent baby unit

GuardedSleep SHALL never produce sound, vibration or haptics on the baby unit, with one exception: while a talk granted to a trusted parent runs, the baby unit SHALL play the talk chime and the parent's voice (`talk-back`). Outside a granted talk there SHALL be no audio playout, only silent minimum-importance notification channels, no alert sounds and no text-to-speech, and the baby unit SHALL never vibrate.

#### Scenario: Parent talks into the phone

- **WHEN** the parent phone's microphone picks up sound during listening without the talk button held
- **THEN** nothing is played on the baby unit

#### Scenario: Talk audio after the talk ended (failure)

- **WHEN** audio from the parent still arrives after the talk ended
- **THEN** the baby unit plays none of it

### Requirement: Baby phone Do Not Disturb

With a one-time Do Not Disturb access grant, the baby unit SHALL activate, while monitoring, an automatic rule that allows alarms only and suppresses visual interruptions; this SHALL be on by default once access is granted. Talk-back SHALL stay audible while the rule is active, and the rule SHALL still silence calls, messages and other apps. Stopping SHALL restore the user's own mode, and a rule left over after a crash SHALL be removed at the next start and the user told.

#### Scenario: App crashed with the rule active

- **WHEN** the app starts and finds its rule still active without monitoring
- **THEN** it removes the rule and tells the user that sounds are back on

#### Scenario: Talk while the rule is active

- **WHEN** the rule is active and a parent's talk is granted
- **THEN** the chime and the parent's voice play, and a message arriving meanwhile stays silent

## ADDED Requirements

### Requirement: Noise detection paused during talk-back

The core SHALL NOT feed the baby unit's level into the noise gate from the start of a talk's chime until 2 seconds after its playback ended, so the baby unit's own speaker never raises a noise alert, a noise escalation or a `mic_silent` change. A noise event running at the start SHALL continue; no noise event SHALL start or end during the pause, and afterwards the gate SHALL start a fresh hold time.

#### Scenario: Parent talks in a quiet room

- **WHEN** a parent talks for 20 seconds at high volume in a quiet room
- **THEN** no noise alert reaches any parent, during or after the talk

#### Scenario: Baby starts crying during a talk (failure)

- **WHEN** the baby starts crying during a 30-second talk and keeps crying
- **THEN** a noise alert reaches the parents no later than the talk limit plus the 2-second tail plus the hold time after the cry started, and both parents saw that noise detection was paused

#### Scenario: Baby already crying

- **WHEN** a noise event is running and a parent starts talking
- **THEN** the noise event continues without a new alert, and audio to the parents keeps flowing as before

### Requirement: Paused levels do not reach the parent

During a talk's noise pause the baby unit SHALL send heartbeats without a level and mark the pause in its status, and the parent SHALL NOT let levels from the pause extend or restart a noise event.

#### Scenario: Talk does not keep a noise event alive (failure)

- **WHEN** a noise event is running, the baby falls quiet and a parent then talks loudly for 30 seconds
- **THEN** the levels during the talk neither extend the event nor restart its alert after a snooze ends

### Requirement: Noise pause is bounded and shown

The noise-detection pause of a talk SHALL never last longer than the talk limit plus the tail, all talk pauses together SHALL stay within the budget of `talk-back` "Talk cooldown and budget", and every trusted parent SHALL see that noise detection is paused while it lasts.

#### Scenario: Other parent during a talk

- **WHEN** parent A talks and parent B is connected
- **THEN** B sees that noise detection is paused while A talks, and the note disappears after the tail
