# Spec Delta

## MODIFIED Requirements

### Requirement: Weak connection

When the link is connected but rated `poor`, the parent SHALL show a "Weak connection — sound may break up" chip without changing the bar's connected state. When it is rated `slow`, the parent SHALL show "Slow connection — alerts may arrive up to {seconds} s later" with the cause when known (mobile data, relay server), also without changing the connected state.

#### Scenario: Poor quality while connected

- **WHEN** the link quality drops to poor while the attempt stays verified
- **THEN** the weak-connection chip appears and the bar stays green

#### Scenario: Slow link

- **WHEN** the link is rated `slow` with an expected delay of 3 s over mobile data
- **THEN** the parent shows that alerts may arrive up to 3 s later over mobile data, and the bar stays green

## ADDED Requirements

### Requirement: Liveness line

While connected, the parent SHALL show how long ago the last message from the baby unit arrived ("Connected · checked {seconds} s ago"), updated at least every second, so a quiet night still shows a live link.

#### Scenario: Quiet room

- **WHEN** no audio has flowed for an hour and heartbeats keep arriving
- **THEN** the liveness line keeps counting up from 0 after every heartbeat
