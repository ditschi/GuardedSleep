# Design

## Context

Source: ADR-020 §4 (WebRTC), §9 (Renovate: 7 days for libwebrtc and Gradle plugins).

## Decisions

- **Vendor behind an interface**: `MediaEngine` hides the SDK so a vendor switch is local to the media layer.
- **CVE-based policy instead of a milestone SLA**: a milestone-based SLA would block every vendor; the trigger is a critical or exploited CVE in a used path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) unpatched 30 days after the Chrome fix, or six months without a vendor release.
- **Discarded alternatives:** GetStream m125 (older milestone); building libwebrtc from source now (cost; `DEFERRED` with F-Droid main).

- **Renovate and milestones**: webrtc-sdk versions are `<milestone>.<build>.<patch>`, so every milestone bump is a semver major. A dedicated rule (`io.github.webrtc-sdk:android`, `major`) overrides the generic monthly Majors schedule with `before 6am on monday`, group `libwebrtc`, still 7-day release age and never automerged, so a new milestone is proposed within a week instead of up to five.
- **Repository content filters** (`apps/android/settings.gradle.kts`): `io.github.webrtc-sdk` resolves from Maven Central only (`exclusiveContent`); Google Maven serves only `com.android`, `com.google` and `androidx` groups; no JitPack. Follow-up, not in this change: `gradle/verification-metadata.xml`.
- **Capture callback**: `AudioBufferCallback` returns the capture timestamp that libwebrtc then uses; the callback must return the incoming value (`CaptureTap`, unit-tested).

## Risks / Trade-offs

- Vendor lag on security fixes: the policy forces an ADR-level decision (switch vendor, patch-build or disable the feature).
- **Patch-policy check at adoption (2026-10-22, owner: project owner).** `150.7871.01` (released 2026-08-31) is the newest webrtc-sdk Android release; the vendor ships no per-milestone security backports and has published nothing newer. Chrome stable is M156 (156.0.8078.47, 2026-10-20), six milestones ahead. Findings from public Chrome security notes (secondary reports, CVE ids not independently verified): Chrome 156 lists a critical memory-corruption bug in WebRTC (reported 2026-10-15, fixed 2026-10-20); Chrome 155 (2026-10-15) lists no WebRTC-path CVE. Backported in `150.7871.01`: **no**, it predates the fix. The 30-day clock of the patch policy runs until 2026-11-19; the project owner re-checks webrtc-sdk releases then. If the bug is in a path we use (the report does not say which component) and no vendor release carries the fix, the policy opens a change (switch vendor, patch-build or disable). Audio-only, sendonly baby and receive-only parent reduce but do not remove exposure (SDP, ICE, DTLS, SCTP and audio codecs are still parsed from the peer, which is authenticated by the pairing proof).
- **Monthly watch**: the libwebrtc CVE watch row in `docs/ci-gates.md` (owner: project owner, first week of each month) repeats this comparison.
- **Commit history note**: commit 31a3453c already uses the webrtc-sdk-only `AudioBufferCallback` API while the build still depends on stream-webrtc, so it does not compile alone (bisect caveat). Keep API moves in the commit that swaps the dependency.

## Measurements

Unsigned release APK (`app-release-unsigned.apk`), all ABIs the build produces:

| Build                                                                      | Size (bytes) |
| -------------------------------------------------------------------------- | -----------: |
| Before: GetStream `stream-webrtc-android`, previous artifact of 2026-10-20 |   43,949,980 |
| After: `io.github.webrtc-sdk:android` 150.7871.01 (this change)            |   49,733,352 |

The ABI filter is arm64-v8a and x86_64. The "before" figure is the previous artifact from the main checkout, not a fresh build of the base commit (too costly), so it also contains unrelated changes since 2026-10-20. The Rust core accounts for 1,684,192 bytes (`size-check.sh`), so the roughly 5.8 MB increase is dominated by the newer libwebrtc milestone and its native libraries.
