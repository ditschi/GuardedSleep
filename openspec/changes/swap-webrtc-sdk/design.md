# Design

## Context

Source: ADR-020 §4 (WebRTC), §9 (Renovate: 7 days for libwebrtc and Gradle plugins).

## Decisions

- **Vendor behind an interface**: `MediaEngine` hides the SDK so a vendor switch is local to the media layer.
- **CVE-based policy instead of a milestone SLA**: a milestone-based SLA would block every vendor; the trigger is a critical or exploited CVE in a used path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) unpatched 30 days after the Chrome fix, or six months without a vendor release.
- **Discarded alternatives:** GetStream m125 (older milestone); building libwebrtc from source now (cost; `DEFERRED` with F-Droid main).

## Risks / Trade-offs

- Vendor lag on security fixes: the policy forces an ADR-level decision (switch vendor, patch-build or disable the feature).

## Measurements

Unsigned release APK (`app-release-unsigned.apk`), all ABIs the build produces:

| Build                                                                      | Size (bytes) |
| -------------------------------------------------------------------------- | -----------: |
| Before: GetStream `stream-webrtc-android`, previous artifact of 2026-10-20 |   43,949,980 |
| After: `io.github.webrtc-sdk:android` 150.7871.01 (this change)            |   49,733,352 |

The "before" figure is the previous artifact from the main checkout, not a fresh build of the base commit (too costly), so it also contains unrelated changes since 2026-10-20. The Rust core accounts for 1,684,192 bytes (`size-check.sh`), so the roughly 5.8 MB increase is dominated by the newer libwebrtc milestone and its native libraries.
