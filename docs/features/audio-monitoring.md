# Feature: Audio Monitoring

## Overview

The core feature of GuardedSleep. The baby unit continuously listens for sounds and analyzes audio locally on the device. When noise exceeds configured thresholds, it triggers alerts and optionally streams audio to the parent unit. All audio processing happens on-device -- no audio data is sent to external servers.

## Requirements

> STATUS: DECIDED -- moved to [`openspec/specs/audio-monitoring`](../../openspec/specs/audio-monitoring/spec.md) (noise gate, sensitivity, audio on demand). Sound categorisation, on-device AI and community data are `IDEA` in the [backlog](../backlog.md) (AGENTS.md §9: no TFLite/YAMNet/MFCC code).

## Dependencies

- [Device Management](device-management.md) -- background execution and battery optimization directly affect audio monitoring capability
- [Communication](communication.md) -- audio streaming when parent listens
- [Notifications](notifications-alerts.md) -- alert delivery when thresholds exceeded

## Open Questions

Moved to the [backlog](../backlog.md) under `audio-monitoring`.

## Discarded Alternatives

| Alternative                    | Reason for Rejection                                                |
| ------------------------------ | ------------------------------------------------------------------- |
| Cloud-based audio analysis     | Violates privacy-first principle; adds latency and cloud dependency |
| Continuous streaming to parent | Wastes bandwidth; contradicts data-efficiency goal                  |
