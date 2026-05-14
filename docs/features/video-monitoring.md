# Feature: Video Monitoring

> STATUS: IDEA -- This entire feature is planned for a future version, not MVP.

## Overview

Extends the audio monitoring with video capabilities. Video is transmitted only when triggered by motion detection, keeping data usage low. Frame rate and resolution are configurable to balance quality against bandwidth.

## Requirements

- `[STATUS: IDEA]` Video transmission triggered by motion detection on baby unit camera
- `[STATUS: IDEA]` Configurable frame rate (e.g., 1 fps for low-bandwidth, 15+ fps for real-time)
- `[STATUS: IDEA]` Configurable resolution (e.g., 240p for data saving, 720p for clarity)
- `[STATUS: IDEA]` Screen wake on parent device when significant movement detected
- `[STATUS: IDEA]` End-to-end encrypted video stream
- `[STATUS: IDEA]` Night vision / infrared if device camera supports it

## Sub-Features

### Motion Detection

- On-device image analysis to detect significant movement
- Configurable sensitivity (avoid false triggers from shadows, light changes)
- Runs with minimal battery impact (low frame rate analysis)

### Adaptive Streaming

- Frame rate and resolution adjust based on activity level and network quality
- More movement = higher frame rate
- Poor network = lower resolution
- Parent can override to force higher/lower quality

### Night Vision

- Use device infrared camera if available
- Software-enhanced low-light mode as fallback
- Not all devices have IR capabilities -- graceful degradation

## Dependencies

- [Audio Monitoring](audio-monitoring.md) -- video and audio work together
- [Communication](communication.md) -- video streaming uses the same encrypted channel
- [Connectivity](connectivity-pairing.md) -- bandwidth requirements are much higher than audio
- [Device Management](device-management.md) -- significant battery impact

## Open Questions

- Which video codec provides the best compression for this use case?
- How to handle the significantly higher bandwidth requirements?
- Can motion detection run efficiently enough on mobile devices?
- How to deal with privacy concerns of video monitoring?
- Should video be opt-in (disabled by default) for privacy reasons?

## Discarded Alternatives

_None yet -- this feature area has not been deeply analyzed._
