# connection-status Specification

## Purpose

Show both people involved, the parent and anyone walking past the baby unit, whether monitoring works right now, with colour, symbol and plain words, and without light or sound that could wake the child. Migrated from ADR-013 and ADR-018 §2 (connection visibility), as built on Android from the core's link state.

## Requirements

### Requirement: Parent ambient bar

While monitoring is started, the parent screen SHALL always show a full-width bar at the top that renders the core's link state and escalation level, so the parent can see at a glance whether the baby is monitored right now.

#### Scenario: Normal night

- **WHEN** the link is connected
- **THEN** the bar is green with a link symbol and names the baby unit

### Requirement: Ambient bar per level

The bar SHALL show: connecting or level 0 in blue with a refresh symbol; level 1 in orange with a broken-link symbol and "still trying"; level 2 and above in red, slowly pulsing, with a broken-link and exclamation symbol and the outage duration; a stopped peer in grey with a pause symbol; a link that needs the user in red with the reason.

#### Scenario: Outage passes level 1

- **WHEN** the link has been down for 25 seconds
- **THEN** the bar is orange with a broken-link symbol and says the baby unit cannot be reached and the app is still trying

#### Scenario: Outage passes level 2

- **WHEN** the link has been down for 70 seconds
- **THEN** the bar is red, pulses slowly and shows how long there has been no connection

### Requirement: Status is never colour alone

Every status SHALL combine colour with a symbol and a text label. Pulsing SHALL become static when the system asks for reduced motion. Changes to level 2 or above, a stopped peer and a link that needs the user SHALL be announced assertively to screen readers.

#### Scenario: Colour-blind parent

- **WHEN** a parent who cannot tell red from green looks at the bar
- **THEN** the symbol and text alone tell whether the link is connected or lost

### Requirement: Plain copy

User-facing status copy SHALL NOT contain IP addresses, port numbers or protocol terms (such as WebSocket, DTLS, ICE or socket errors). A link that needs the user SHALL say what to do in plain words: pair again, or update one of the units.

#### Scenario: Protocol mismatch

- **WHEN** the link fails with `update_required`
- **THEN** the parent reads that one of the units needs a GuardedSleep update, without any technical term

### Requirement: Restored notice

After recovery from level 1 or above, the parent SHALL show for 10 seconds that the link is connected again together with how long the baby was not monitored, and SHALL post a notification with the same duration.

#### Scenario: Recovery after 90 seconds

- **WHEN** the link recovers after a 90-second outage
- **THEN** the parent sees "connected again" with an unmonitored duration of 1:30

### Requirement: Weak connection

When the link is connected but its quality is poor, the parent SHALL show a "Weak connection — sound may break up" chip without changing the bar's connected state.

#### Scenario: Poor quality while connected

- **WHEN** the link quality drops to poor while the attempt stays verified
- **THEN** the weak-connection chip appears and the bar stays green

### Requirement: Monitoring notification text

The parent's persistent monitoring notification SHALL say whether it is connected, connecting or reconnecting to the baby unit.

#### Scenario: Reconnecting in the background

- **WHEN** the link is reconnecting while the app is in the background
- **THEN** the notification says it is reconnecting to the baby unit

### Requirement: Baby unit edge indicator

The baby unit SHALL show the link state only as a small indicator at the screen edge, with colour and symbol: orange at level 1 and a very dim red from level 2. In the dark kiosk the indicator SHALL NOT light up the screen, and the baby unit SHALL never use sound or vibration for any connection state.

#### Scenario: Parent phone goes away at night

- **WHEN** the parent unit has been unreachable for 2 minutes
- **THEN** the baby unit shows a dim red edge indicator and stays silent

### Requirement: Baby unit status on touch-wake

When someone wakes the baby unit's screen, it SHALL show the link status in plain words: connected, waiting for the parent unit, reconnecting, parent unit not responding — still trying, or parent unit stopped monitoring.

#### Scenario: Adult checks the baby phone

- **WHEN** an adult touches the baby phone while the parent unit is unreachable
- **THEN** the screen says the parent unit is not responding and that the app is still trying
