# Design

## Context

Source: ADR-020 §4 (WebRTC), §9 (Renovate: 7 days for libwebrtc and Gradle plugins).

## Decisions

- **Vendor behind an interface**: `MediaEngine` hides the SDK so a vendor switch is local to the media layer.
- **CVE-based policy instead of a milestone SLA**: a milestone-based SLA would block every vendor; the trigger is a critical or exploited CVE in a used path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) unpatched 30 days after the Chrome fix, or six months without a vendor release.
- **Discarded alternatives:** GetStream m125 (older milestone); building libwebrtc from source now (cost; `DEFERRED` with F-Droid main).

## Risks / Trade-offs

- Vendor lag on security fixes: the policy forces an ADR-level decision (switch vendor, patch-build or disable the feature).
