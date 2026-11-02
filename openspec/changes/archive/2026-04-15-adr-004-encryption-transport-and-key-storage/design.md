# ADR-004: Encryption, Transport, and Key Storage (MVP Baseline)

- Status: DECIDED
- Date: 2026-04-15
- Deciders: Security-informed architecture

> **Migrated to OpenSpec (2026-10-22):** §1–§3, §5, §6 and both amendments are normative in [`openspec/specs/pairing`](../../../specs/pairing/spec.md); §3 (application-level E2E) is `DEFERRED` in the [backlog](../../../../docs/backlog.md). §4 (WSS) is covered by the `pairing` requirement "Signaling transport security". Archived 2026-10-22.

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

## Amendment 2026-10-19: direction-bound pairing proof and PSK-derived room id

- Status: DECIDED (owner, 2026-10-21, with [ADR-017](../2026-10-20-adr-017-link-recovery-and-connection-supervision/design.md) §1). A deep security audit is required before the alpha release, see [backlog](../../../../docs/backlog.md) "Security audit before alpha"

The fingerprint-binding MAC as implemented (`HMAC(PSK, room|sorted fingerprints|version)`) is **symmetric**: both peers send the value they expect, so a peer that only knows the room id can reflect the other side's proof and pass verification (security review 2026-10-19, CRITICAL). Protocol v3 replaces it:

- `mac_role = HMAC-SHA256(PSK, "gs-pair-proof/3|" + role + "|" + room + "|" + fp_sender + "|" + fp_receiver)`; each side sends its own MAC and verifies the peer's with roles and fingerprints swapped. Comparison is constant-time.
- The room id is derived from the PSK (`HKDF-SHA256`, info `gs-room/3`, 128 bit, base64url) and is no longer a separate, guessable value.
- PSK, derived room id and TURN credentials are stored encrypted with an AES-GCM key in the Android Keystore (StrongBox when available); never in Intents, Bundles or logs. Web clients keep the PSK in memory only.

### Amendment addendum 2026-10-20: fingerprint source and DTLS cross-check (code review)

- The fingerprint used in the proof is parsed only from SDP lines that **start with** `a=fingerprint:`; exactly one distinct, well-formed SHA-256 value is accepted, otherwise the attempt fails (`fingerprint_invalid`). The earlier substring search let a relay hide the genuine fingerprint in another line while DTLS ran against its own certificate (security review 2026-10-20, CRITICAL).
- Defence in depth: platforms report the remote certificate fingerprint from WebRTC stats (`RTCCertificateStats`) after DTLS connects (`attempt_dtls_fingerprint`); the core verifies an attempt only if it equals the SDP fingerprint bound by the proof (`require_dtls_fingerprint`, default on).
