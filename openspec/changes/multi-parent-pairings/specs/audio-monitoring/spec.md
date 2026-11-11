# Spec Delta

## ADDED Requirements

### Requirement: Audio per parent link

With several parent links the baby unit SHALL open one microphone and SHALL enable audio separately for each link, in addition to "Audio only on demand". Audio SHALL reach a link only when that link is trusted and has a reason to send: noise from the shared noise gate (every trusted link), its own parent listening, or its own test alert. While a test alert's probe tone runs, every other link SHALL send no audio.

#### Scenario: Parent A listens

- **WHEN** parent A listens and the room is quiet
- **THEN** audio is sent on A's link only

#### Scenario: Unverified second attempt

- **WHEN** A's link sends audio and B's link has an attempt that has not passed its proof
- **THEN** no audio is sent on B's link

#### Scenario: Test during noise

- **WHEN** parent A runs a test alert while the room is loud
- **THEN** parent B's link sends no audio until the probe tone has ended
