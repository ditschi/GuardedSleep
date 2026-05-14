# Feature: Audio Monitoring

## Overview

The core feature of GuardedSleep. The baby unit continuously listens for sounds and analyzes audio locally on the device. When noise exceeds configured thresholds, it triggers alerts and optionally streams audio to the parent unit. All audio processing happens on-device -- no audio data is sent to external servers.

## Requirements

- `[STATUS: DECIDED]` Continuous local audio capture on the baby unit while the app is active -- **MVP**
- `[STATUS: DECIDED]` Configurable noise detection threshold (volume level, slider with live preview) -- **MVP**
- `[STATUS: DECIDED]` Configurable duration threshold (3s/5s/10s/15s, default 5s) -- **MVP**
- `[STATUS: DECIDED]` On-device audio analysis (no raw audio leaves the device unless parent is listening) -- **MVP**
- `[STATUS: DECIDED]` Noise event triggers alert to parent unit (see [notifications-alerts.md](notifications-alerts.md)) -- **MVP**
- `[STATUS: DECIDED]` Audio stream to parent unit when parent activates listen mode (see [communication.md](communication.md)) -- **MVP**

## Sub-Features

### Noise Detection Engine

The core detection component that runs on the baby unit:

- Monitors ambient sound levels continuously
- Compares against user-configured thresholds
- Distinguishes between brief spikes (door closing) and sustained noise (crying)
- Must be battery-efficient (see [device-management.md](device-management.md))

### Threshold Configuration

Users can configure:

- Volume sensitivity (how loud before triggering)
- Duration sensitivity (how long before triggering)
- Possibly multiple alert levels (e.g., "soft alert" at low threshold, "urgent alert" at high threshold)

### Future: Sound Categorization

> STATUS: IDEA -- Future feature, not for MVP

Local ML-based classification of detected sounds:

- Crying vs. coughing vs. babbling vs. environmental noise
- Runs entirely on-device for privacy
- Could enable smarter alerts (e.g., "crying detected" vs. "noise detected")

### Future: On-Device AI Audio/Video Analysis

> STATUS: IDEA -- Feasibility confirmed by research. Phase 3+.

Local AI models running on the baby device for intelligent audio (and later video) analysis:

**Feasibility assessment**:

- **YES, this is realistic on modern Android devices.** Research shows lightweight CNN models (YAMNet transfer learning + compact classifier) achieve ~95% accuracy for baby cry detection with <0.8s latency on resource-constrained hardware (even Raspberry Pi).
- **Model size**: Quantized TFLite models can be as small as 1-5 MB.
- **Framework**: TensorFlow Lite for Android, optionally Edge Impulse for training pipeline.
- **Feature extraction**: MFCCs / Mel-spectrograms from audio frames.
- **Temporal smoothing**: Exponential Moving Average (EMA) reduces false positives.
- **Battery**: Should only run when charging (configurable), or with very efficient duty cycling.
- **Privacy**: All processing on-device, no audio sent to any server.

**Possible classifications**:

- Baby crying (urgent alert)
- Coughing (health alert)
- Babbling / talking (low-priority notification)
- Environmental noise (ignore / filter out)
- Silence (all quiet)

**Challenges**:

- Training data: diverse home environments, different babies
- False positives from TV, pets, household appliances
- Battery consumption (mitigate: only when charging, or low-duty-cycle)

### Future: Community Data Contribution for Model Training

> STATUS: IDEA -- Privacy and security concerns to be investigated before implementation.

Users can optionally contribute anonymized audio snippets to improve the AI model:

**How it would work**:

1. App keeps a local history of recent noise detection events (short audio snippets, ~3-5 seconds)
2. User reviews snippets in-app: mark as "correctly detected" or "false positive"
3. User can cut/trim snippets, add suggested correct categorization
4. User explicitly chooses to share selected snippets (opt-in, never automatic)
5. Snippets are anonymized (no metadata, no device ID, no timestamps beyond duration)
6. Uploaded to a contribution server for model training

**Privacy and security concerns (MUST be resolved before implementation)**:

- Snippets may contain identifiable voices (children, parents) -- anonymization insufficient?
- Risk of deliberate misuse (poisoning training data with wrong labels)
- Risk of too much data from one user (data diversity, DDoS)
- GDPR implications: audio snippets are personal data even if "anonymized"
- Who operates the contribution server? Trust model?
- Minimum age of audio data before contribution allowed?
- Review/moderation process for contributed data?
- Consent model: per-snippet consent or blanket opt-in?

**Status**: This idea requires significant privacy and legal analysis before it can be considered for implementation. Document here for future reference.

## Dependencies

- [Device Management](device-management.md) -- background execution and battery optimization directly affect audio monitoring capability
- [Communication](communication.md) -- audio streaming when parent listens
- [Notifications](notifications-alerts.md) -- alert delivery when thresholds exceeded

## Open Questions

- What audio processing library/API to use? (platform-dependent)
- What is the minimum sampling rate needed for reliable detection?
- How to handle audio processing in the background on iOS vs Android?
- Should threshold configuration be simple (single slider) or advanced (multiple parameters)?

## Discarded Alternatives

| Alternative                    | Reason for Rejection                                                |
| ------------------------------ | ------------------------------------------------------------------- |
| Cloud-based audio analysis     | Violates privacy-first principle; adds latency and cloud dependency |
| Continuous streaming to parent | Wastes bandwidth; contradicts data-efficiency goal                  |
