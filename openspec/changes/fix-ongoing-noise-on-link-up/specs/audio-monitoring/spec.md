# Spec Delta

## MODIFIED Requirements

### Requirement: Local noise detection

While monitoring, the baby unit SHALL capture audio continuously and analyse it on the device. The noise gate SHALL turn loud when the level stays at or above the threshold for the hold time, and quiet as soon as the level falls below it. Changing the threshold or hold time SHALL NOT fire an alert by itself. When a link to the parent becomes trusted (the first link or a recovery) while the noise gate is loud, the baby unit SHALL send a noise event at once, in addition to the missed-events report.

#### Scenario: Door slams

- **WHEN** a single loud sound lasts shorter than the hold time
- **THEN** no noise event is sent

#### Scenario: Baby cries

- **WHEN** the level stays above the threshold for the hold time
- **THEN** the baby unit sends a noise event with its level to the parent

#### Scenario: Baby still crying when the link comes up

- **WHEN** the noise gate turned loud while no trusted link existed and the level is still above the threshold when the link becomes trusted
- **THEN** the baby unit sends a noise event with its level to the parent right away, and the parent alerts
