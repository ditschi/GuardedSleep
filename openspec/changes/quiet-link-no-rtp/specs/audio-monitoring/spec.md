# Spec Delta

## MODIFIED Requirements

### Requirement: Audio only on demand

The baby unit SHALL send audio only while the noise gate is loud, the parent is listening, or a test alert runs. Otherwise it SHALL detach the microphone track from the audio sender, so no RTP packets are sent, and SHALL keep the track disabled as well, so a failed detach can only send silence. Audio SHALL flow only from the baby unit to the parent unit, encrypted by DTLS-SRTP.

#### Scenario: Quiet night

- **WHEN** the room is quiet and the parent is not listening
- **THEN** the baby unit sends no RTP packets

#### Scenario: Noise after a quiet hour

- **WHEN** the noise gate turns loud after an hour without audio
- **THEN** the first audio packet leaves the baby unit within 100 ms of the gate change

## ADDED Requirements

### Requirement: Quiet-send self-check

After muting, the baby unit SHALL check that its audio sender stops sending packets. If packets keep flowing, the baby unit SHALL show once a notice with the measured extra data per night and that Chrome, Edge or the Android app send almost nothing while it is quiet, and the parent SHALL show the same in its connection details.

#### Scenario: Browser that keeps sending

- **WHEN** the sender's packet count still grows 10 s after muting
- **THEN** the baby unit shows the notice with the estimated megabytes per night

### Requirement: Speech-grade audio profile

Both clients SHALL negotiate Opus mono with an audio bandwidth up to 8 kHz, in-band FEC on, DTX off, 60 ms packets, and an average bitrate of 16, 24 or 32 kbit/s from the parent's audio quality setting (default 24). The profile SHALL be written into the session description by the core.

#### Scenario: Default profile

- **WHEN** the devices connect with the default setting
- **THEN** the negotiated Opus parameters are mono, 16 kHz playback rate, FEC on, 60 ms packets and 24 kbit/s

### Requirement: Raw microphone capture

The baby unit SHALL capture without echo cancellation, noise suppression and automatic gain on every platform, so the noise gate and the parent hear the room as it is. Android SHALL use the unprocessed microphone source where the device supports it, otherwise the plain microphone source.

#### Scenario: White-noise machine in the room

- **WHEN** a steady white-noise machine runs in the baby room
- **THEN** its sound is neither removed nor raised by the capture path, and the parent hears it while listening
