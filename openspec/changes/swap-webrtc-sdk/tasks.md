# Tasks

Refined 2026-10-22. Implemented as W2-C on branch `worktree-agent-a409f09da55bdd44c` (a77ef792, c186ea70, 67a68f8f); ticked after the rebase onto `chore/toolchain-wave-2` and the checks below.

- [ ] 1.1 `MediaEngine` interface in the platform layer; all vendor WebRTC imports in one implementation class; a test fails if another file imports `org.webrtc`
- [ ] 1.2 Swap `io.getstream:stream-webrtc-android` for `io.github.webrtc-sdk:android` (150.7871.01, verified on Maven Central); audio-only behaviour, DTLS fingerprint handling and link-quality stats unchanged
- [ ] 1.3 Renovate rule: own PR, no automerge and a 7-day release age for libwebrtc
- [ ] 1.4 `just lint`, `just test`, release-check and size-check green; APK size before/after recorded in `design.md`
- [ ] 1.5 Emulator gate AC-1…AC-4 and AC-10 green (`DOZE_MINUTES=2 just gate`)
