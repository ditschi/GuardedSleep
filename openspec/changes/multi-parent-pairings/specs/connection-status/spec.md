# Spec Delta

## MODIFIED Requirements

### Requirement: Baby unit status on touch-wake

When someone wakes the baby unit's screen, it SHALL show the link status in plain words: connected, waiting for the parent unit, reconnecting, parent unit not responding — still trying, or parent unit stopped monitoring. With two saved parent pairings it SHALL name how many parent devices are connected and the state of each by its connection name.

#### Scenario: Adult checks the baby phone

- **WHEN** an adult touches the baby phone while the parent unit is unreachable
- **THEN** the screen says the parent unit is not responding and that the app is still trying

#### Scenario: One of two connected

- **WHEN** an adult touches the baby phone while one of two parent devices is connected
- **THEN** the screen says 1 of 2 parent devices is connected and names both with their state

## ADDED Requirements

### Requirement: Other parent status

When the baby unit has two saved parent pairings, the parent monitoring screen SHALL show a quiet line saying whether the other parent device is connected, from the status the baby unit sends, and SHALL hide it while its own link is not connected. It SHALL NOT change the ambient bar, which reflects only this parent's own link.

#### Scenario: Other parent leaves

- **WHEN** the other parent device's link stops
- **THEN** this parent's screen says the other parent device is not connected and its bar stays green

#### Scenario: Own link down

- **WHEN** this parent's own link is lost
- **THEN** the other-parent line is hidden until the link is connected again
