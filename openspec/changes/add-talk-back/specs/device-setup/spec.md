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
