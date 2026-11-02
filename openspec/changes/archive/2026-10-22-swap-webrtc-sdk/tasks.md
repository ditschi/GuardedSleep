# Tasks

Refined 2026-10-22. Implemented as W2-C on branch `worktree-agent-a409f09da55bdd44c` (a77ef792, c186ea70, 67a68f8f); ticked after the rebase onto `chore/toolchain-wave-2` and the checks below.

- [x] 1.1 `MediaEngine` interface in the platform layer; all vendor WebRTC imports in one implementation package (`media/libwebrtc`); a test fails if a file outside it imports `org.webrtc`
- [x] 1.2 Swap `io.getstream:stream-webrtc-android` for `io.github.webrtc-sdk:android` (150.7871.01, verified on Maven Central); audio-only behaviour, DTLS fingerprint handling and link-quality stats unchanged
- [x] 1.3 Renovate rule: own PR, no automerge and a 7-day release age for libwebrtc
- [x] 1.4 `just lint`, `just test`, release-check and size-check green; APK size before/after recorded in `design.md`
- [x] 1.5 Emulator gate AC-1…AC-4 and AC-10 green (`DOZE_MINUTES=2 just gate`)
- [x] 1.6 Review fixes: capture timestamp passed through (`CaptureTap`), seam moved to `media/libwebrtc`, Gradle repository content filters, spec delta completed, M150 CVE check recorded, Renovate milestone rule, libwebrtc CVE watch row in `docs/ci-gates.md`
