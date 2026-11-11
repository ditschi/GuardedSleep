# Spec Delta

## MODIFIED Requirements

### Requirement: Baby kiosk

While monitoring, the baby unit SHALL show a dark kiosk screen at minimum brightness that locks itself 30 seconds after the last touch. A touch SHALL wake it at low brightness (about 10 %) to show the status, and unlocking SHALL need a hold-and-slide gesture. The kiosk SHALL never use vibration or haptics, and SHALL use sound only for a granted talk (`talk-back`); a running talk SHALL show only the dim talk indicator, without waking the screen to normal brightness.

#### Scenario: Toddler taps the baby phone

- **WHEN** someone taps the locked baby unit
- **THEN** the screen wakes dimly and shows the status, and only hold-and-slide unlocks it

#### Scenario: Parent talks while the kiosk is dark

- **WHEN** a parent talks while the kiosk screen is dark
- **THEN** the dim talk indicator appears and the screen stays at minimum brightness

### Requirement: Setup checklist per role

Each role SHALL have one checklist screen whose rows show a state and open the matching system setting: on the baby unit the microphone, background operation (battery optimisation exemption), silence access and a talk-back row that says parent devices can speak through this device, with its switch and volume; on the parent unit notifications, lock-screen and full-screen alerts, Do Not Disturb bypass and background operation. Permissions SHALL be requested only when needed, with a plain reason.

#### Scenario: Battery optimisation still on

- **WHEN** the baby unit's app is still battery-optimised
- **THEN** the background row is marked to do and its button opens the system's battery optimisation setting

#### Scenario: Setting up the baby unit

- **WHEN** an adult goes through the baby unit's checklist
- **THEN** the talk-back row shows whether parent devices can speak through this device and lets the adult change it
