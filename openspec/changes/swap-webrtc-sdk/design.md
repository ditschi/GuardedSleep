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
- **Patch-policy check at adoption (2026-10-22, owner: project owner).** `150.7871.01` (Maven Central, 2026-08-31) is the newest webrtc-sdk Android release; there are no GitHub releases, and the `m150_release` branch has received only vendor feature/bug commits since then (no visible Chromium security cherry-picks). Chrome stable is M156 (156.0.8078.47, 2026-10-20). Sources: the Chrome Releases desktop stable posts for every M151–M156 stable point release, cross-checked against NVD for CVE-2026-113303, CVE-2026-79187 and CVE-2026-103631. Chrome names only the component "WebRTC" and keeps the bug tracker restricted, so **which subcomponent (and therefore whether our audio/DTLS/SRTP/SCTP/ICE/SDP path is touched) is unknown for every item**; we treat each one as possibly in a used path. "In 150.7871.01" is **no** for every M151+ fix: the build is based on M150 and the vendor has published nothing newer. No WebRTC fix in M151–M156 is marked exploited in the wild (the in-the-wild CVEs in this window, CVE-2026-85046 and CVE-2026-87491, are in V8).
  - **M151** (first stable 151.0.7922.71, 2026-07-29): Medium CVE-2026-17884 (object lifecycle issue), Low CVE-2026-17951 (heap buffer overflow). No critical or exploited item: policy not triggered.
  - **M152** (152.0.7977.64, 2026-08-25; 152.0.7977.75, 2026-09-01): High CVE-2026-79187 and CVE-2026-79027 (use after free); Medium CVE-2026-78891 (buffer overflow), CVE-2026-79255 (improper input validation), CVE-2026-84347 (use after free, 2026-09-01); Low CVE-2026-78950 (integer overflow). No critical or exploited item: policy not triggered.
  - **M153** (153.0.8010.36, 2026-09-08): Medium CVE-2026-87630 (integer overflow), CVE-2026-87579 (buffer overflow); Low CVE-2026-87430 (buffer overflow). No critical or exploited item: policy not triggered.
  - **M154** (154.0.8037.57, 2026-09-22; 154.0.8037.97, 2026-10-01): High CVE-2026-103631 (buffer overflow, 2026-10-01). No critical or exploited item: policy not triggered.
  - **M155** (155.0.8059.39, 2026-10-06; 155.0.8059.79, 2026-10-13): High CVE-2026-106268 and CVE-2026-106357 (use after free, 2026-10-06), High CVE-2026-110137 (use after free, 2026-10-13). The critical CVE-2026-110151 of 2026-10-13 is in Chromium "Media", not libwebrtc. No critical or exploited WebRTC item: policy not triggered.
  - **M156** (156.0.8078.47, 2026-10-20): **Critical CVE-2026-113303** (memory corruption, NVD-confirmed) and High CVE-2026-113161 (use after free). The critical item starts the 30-day clock: **deadline 2026-11-19**, tracked in `docs/backlog.md` "libwebrtc CVE decision by 2026-11-19".
  - Result: the policy is **not** breached at adoption; the only open trigger is CVE-2026-113303. Not covered by this check: M150 point-release fixes after the vendor's branch point (e.g. High CVE-2026-15121, 150.0.7871.114, 2026-07-08), whose presence in `150.7871.01` is unknown. Seven High WebRTC fixes in M152–M156 are missing from `150.7871.01`; they do not trigger the policy, which counts only critical or exploited CVEs, but they show the size of the lag. Audio-only, sendonly baby and receive-only parent reduce but do not remove exposure (SDP, ICE, DTLS, SCTP and audio codecs are still parsed from the peer, which is authenticated by the pairing proof).
- **Monthly watch**: the libwebrtc CVE watch row in `docs/ci-gates.md` (owner: project owner, first week of each month) repeats this comparison and records results in the `docs/backlog.md` entry "libwebrtc CVE decision" (or its successor), which stays put when this change is archived.
- **Commit history note**: commit 31a3453c already uses the webrtc-sdk-only `AudioBufferCallback` API while the build still depends on stream-webrtc, so it does not compile alone (bisect caveat). Keep API moves in the commit that swaps the dependency.

## Measurements

Unsigned release APK (`app-release-unsigned.apk`), all ABIs the build produces:

| Build                                                                      | Size (bytes) |
| -------------------------------------------------------------------------- | -----------: |
| Before: GetStream `stream-webrtc-android`, previous artifact of 2026-10-20 |   43,949,980 |
| After: `io.github.webrtc-sdk:android` 150.7871.01 (this change)            |   49,733,352 |

The ABI filter is arm64-v8a and x86_64. The "before" figure is the previous artifact from the main checkout, not a fresh build of the base commit (too costly), so it also contains unrelated changes since 2026-10-20. The Rust core accounts for 1,684,192 bytes (`size-check.sh`), so the roughly 5.8 MB increase is dominated by the newer libwebrtc milestone and its native libraries.
