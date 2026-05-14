# ADR-004: Encryption, Transport, and Key Storage (MVP Baseline)

- Status: DECIDED
- Date: 2026-04-15
- Deciders: Security-informed architecture

## Context

Audio and control data must be protected in transit; pairing must resist casual MITM; keys must not live in plaintext on disk.

## Decision

1. **Layer 1:** WebRTC **DTLS-SRTP** for media and **DTLS** for DataChannel (standard WebRTC).

2. **Layer 2:** **DTLS certificate fingerprint** bound to material from **QR pairing** (PSK / pairing exchange) -- automatic verification, no manual user fingerprint compare in normal flow.

3. **Layer 3:** Application-level E2E with HKDF-derived keys -- **Phase 2**, not MVP.

4. **Signaling:** **WSS** (TLS) for WebSocket signaling.

5. **Key storage:** **PSK and sensitive pairing material** in **Android Keystore** via Secure Storage API; non-sensitive settings in Preferences. **No app-level PIN/biometrics in MVP** (ADR-001) -- device encryption and OS lock are the user's responsibility.

6. **Protocol version:** DataChannel messages must carry a **protocol version** for compatibility (exact schema in implementation spec).

## Consequences

### Positive

- Aligns with industry practice; no custom crypto for transport.

### Negative

- Users who never lock their phone have higher physical-access risk -- mitigated by education, not in-app lock in MVP.

### Risks

- Compromised WebRTC stack -- Layer 3 deferred; accepted for MVP scope.

## Discarded alternatives

- Noise Protocol as replacement for DTLS-SRTP -- rejected for media path.
- Plain localStorage for PSK -- rejected.
