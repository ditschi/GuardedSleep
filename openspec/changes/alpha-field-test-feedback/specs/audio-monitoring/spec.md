## MODIFIED Requirements

### Requirement: Local noise detection

While monitoring, the baby unit SHALL capture audio continuously and analyse it on the device. Each level sample SHALL count for the time since the previous sample, at most 1 second. The noise gate SHALL turn loud on a sample at or above the threshold when the samples at or above the threshold cover at least half of the last hold time; only the last hold time SHALL count, whatever came before. It SHALL turn quiet when no sample has reached the threshold for 2 seconds. A restart of the hold time (for example after a talk-back pause) SHALL clear the counted history. Changing the threshold or hold time SHALL NOT fire an alert by itself.

#### Scenario: Door slams

- **WHEN** a single loud sound lasts shorter than half the hold time
- **THEN** no noise event is sent

#### Scenario: Baby cries

- **WHEN** the level stays above the threshold for the hold time
- **THEN** the baby unit sends a noise event with its level to the parent

#### Scenario: Talking with natural pauses

- **WHEN** people talk near the baby unit above the threshold for 6 seconds with pauses of up to half a second between words, and the hold time is 5 seconds
- **THEN** the gate turns loud and the baby unit sends a noise event

#### Scenario: Crying after a long noisy background (failure guard)

- **WHEN** a TV kept the level above the threshold for a third of the time during the last minute and then the child cries continuously
- **THEN** the gate turns loud no later than one hold time plus one sample after the crying started

#### Scenario: Sparse samples

- **WHEN** level samples arrive only once per second (a throttled browser tab) and the child cries continuously
- **THEN** the gate still turns loud after the hold time

#### Scenario: Occasional knocks

- **WHEN** short knocks above the threshold cover less than half of every hold-time window
- **THEN** no noise event is sent

### Requirement: Sensitivity settings

The baby unit SHALL let the user set the threshold with a slider and a live level preview, and the hold time to 3, 5, 10 or 15 seconds (default 5). The slider SHALL move in even steps of loudness (decibels) from about −55 dBFS at its sensitive end to about −15 dBFS; its default SHALL detect normal talking at about 2 metres on a reference phone without firing in a quiet room. Every threshold the slider can set SHALL be accepted by both devices in every message that carries it.

#### Scenario: Setting up a noisy room

- **WHEN** the user moves the threshold slider while the room is quiet
- **THEN** the live preview shows the current level against the new threshold

#### Scenario: Slider halfway

- **WHEN** the user sets the slider to its middle
- **THEN** the threshold is halfway between both ends in decibels, not in linear level

#### Scenario: Most sensitive setting reaches the parent (failure guard)

- **WHEN** the user sets the most sensitive threshold
- **THEN** the parent still receives and shows the baby unit's status, including battery and microphone state

## ADDED Requirements

### Requirement: Raw microphone capture

The baby unit SHALL capture without echo cancellation, noise suppression and automatic gain on every platform, so the noise gate and the parent hear the room as it is. Android SHALL use the unprocessed microphone source where the device supports it, otherwise the plain microphone source.

#### Scenario: White-noise machine in the room

- **WHEN** a steady white-noise machine runs in the baby room
- **THEN** its sound is neither removed nor raised by the capture path, and the parent hears it while listening
