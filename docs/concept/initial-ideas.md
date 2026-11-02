# Initial Ideas and Brainstorm

> This document preserves the original brainstorm and raw ideas that started the project.
> It serves as a reference for the thought process behind the concept.
> Refined versions of these ideas are tracked in the [feature documents](../backlog.md).

## Original Brainstorm

### Core Concept

A mobile-friendly, cross-OS app (Android, iOS phones/tablets, optionally PC) that works as a baby monitor. Security and privacy first, with encryption. Two modes: baby unit and parent unit.

### Audio Monitoring

- Configurable voice/noise detection with future local categorization (cough, cry, etc.)
- Transmission to parent when thresholds are exceeded (volume, duration of detected noise, etc.)
- Parent can "talk to child" (talk-back feature) -- only to one child device if multiple are connected

### Data Efficiency

- Transmission only when child appears awake / noise detected
- Minimal heartbeat signal (warn if connection lost)
- Parent explicitly activates "listen mode" (e.g., button that needs to be held)

### Connectivity

- Must work securely and encrypted over the internet
- Must also work without internet (e.g., create hotspot or similar local connection)
- Device pairing via QR codes for easy, secure connection setup

### Notifications

- Parent notification with ringtone + vibration + screen activation when thresholds exceeded

### Future Ideas

- Video support with movement detection
- High movement enables transmission of image + wakes parent screen
- Frame rate and resolution configurable to save data
- Multiple children and multiple parents support
- Local sound categorization (cry, cough, breathing patterns)

## Additional Considerations

These points were identified during concept development as important aspects not in the original brainstorm:

### Battery and Background Execution

Baby devices will potentially run for hours. Continuous audio monitoring drains battery significantly. Mobile operating systems (especially iOS) aggressively kill background apps. This is a critical technical challenge that directly affects which platform approach is viable.

### Network Resilience

Brief network disconnections are inevitable. The system needs strategies for:

- Buffering detected events during short outages
- Automatic reconnection without user intervention
- Clear indication to the parent when the connection is degraded

### Device Pairing and Trust

How do baby and parent devices securely discover and authenticate each other? QR code pairing was suggested as a user-friendly approach. The pairing mechanism must establish a cryptographic trust relationship.

### Platform Constraints

- PWAs (Progressive Web Apps) have limited background execution on iOS
- Native apps require separate codebases or cross-platform frameworks
- App store distribution (especially Apple) adds complexity and gatekeeping
- Camera and microphone access varies by platform and app type

### NAT Traversal

P2P connections between devices on different networks require NAT traversal (STUN/TURN servers). Even "serverless" architectures need some infrastructure for connection establishment.

### Push Notifications

Each mobile platform has its own push notification system. Waking a parent's screen and triggering sound/vibration requires platform-specific integration.

## Features Considered but Deferred or Rejected

During the initial brainstorm review, the following features were evaluated:

| Feature                                      | Status   | Reasoning                                       |
| -------------------------------------------- | -------- | ----------------------------------------------- |
| Audio sensitivity calibration                | DEFERRED | Nice-to-have, not essential for first version   |
| Event history / sleep log                    | DEFERRED | Valuable but adds significant complexity        |
| White noise / lullaby playback               | REJECTED | Out of scope -- this is a monitor, not a player |
| Device lock / kiosk mode                     | DEFERRED | Future feature for baby unit                    |
| Accessibility (screen reader, high contrast) | DEFERRED | Important but later phase                       |
| Data retention policy                        | DEFERRED | Needs definition during technical design        |
| Sleep/wake scheduling                        | DEFERRED | Automatic scheduling adds complexity            |
| Latency requirements definition              | DEFERRED | Needs analysis during technical design          |
