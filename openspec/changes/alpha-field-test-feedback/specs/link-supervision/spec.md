## MODIFIED Requirements

### Requirement: Noise event lifecycle in the core

The parent core SHALL track its current noise event from non-probe noise alerts and loud peer heartbeats. A peer heartbeat SHALL count as loud when the baby's noise gate is loud at sending time, which the baby SHALL state in every heartbeat, or when its level is at or above the threshold. The event SHALL end after 2 minutes without a loud signal, and the next noise alert SHALL start a new event with a fresh count and no acknowledgment.

#### Scenario: New event after quiet

- **WHEN** an escalated event ended after 2 quiet minutes and noise returns
- **THEN** a new event starts and escalates only after the delay passes again

#### Scenario: Crying in bursts after a pause ends (failure guard)

- **WHEN** noise alerts were paused, the child keeps crying in bursts with the gate loud throughout, and the pause ends while a heartbeat falls into a breath between bursts
- **THEN** the parent still treats the noise as ongoing and alerts

## ADDED Requirements

### Requirement: Acknowledging a deliberate stop

The parent core SHALL accept an acknowledgment of a deliberate baby stop and SHALL stop monitoring for it only while the link is still `peer_stopped`. If the baby announced a new session and the link left `peer_stopped` before the acknowledgment, the acknowledgment SHALL be ignored and monitoring SHALL continue. No notification action SHALL send this acknowledgment.

#### Scenario: Baby stopped on purpose

- **WHEN** the link is `peer_stopped` and the parent acknowledges the stop
- **THEN** the parent stops monitoring

#### Scenario: Baby restarted before the tap (failure)

- **WHEN** the baby restarted monitoring and the link is connecting or connected again when the parent's acknowledgment arrives
- **THEN** the parent keeps monitoring
