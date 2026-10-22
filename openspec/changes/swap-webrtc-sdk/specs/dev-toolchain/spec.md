# Spec Delta

## ADDED Requirements

### Requirement: libwebrtc patch policy

A critical or exploited CVE in a used WebRTC path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) that the vendor has not patched 30 days after the Chrome fix, or six months without a vendor release, SHALL trigger a change that switches the vendor, builds a patched version or disables the feature.

#### Scenario: Unpatched SRTP CVE

- **WHEN** a critical SRTP CVE is still unpatched by the vendor 30 days after Chrome fixed it
- **THEN** a change is opened to switch vendor, patch-build or disable the affected feature
