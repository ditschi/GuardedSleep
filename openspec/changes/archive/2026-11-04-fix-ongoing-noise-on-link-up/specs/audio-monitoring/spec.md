# Spec Delta

## ADDED Requirements

### Requirement: Ongoing noise when the link comes up

When a link to the parent becomes trusted (the first link or a recovery) while the noise gate is loud, the baby unit SHALL send a noise event at once, in addition to the missed-events report.

#### Scenario: Baby still crying when the link comes up

- **WHEN** the noise gate turned loud while no trusted link existed and the level is still above the threshold when the link becomes trusted
- **THEN** the baby unit sends a noise event with its level to the parent right away, and the parent alerts
