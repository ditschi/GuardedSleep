# Tasks

Alpha (T0), after `alpha-connection-model`. Each task is a brief for the `implementer`: layer rules from AGENTS.md §5.1 and §7.8, commit after each task, and run `just test-affected` after each task.

## 1. Relay and core

- [ ] 1.1 LAN admission without a room id (`gs-relay-admit/2`), with a constant-time key loop over at most 4 keys on the PC relay. Vectors and relay tests. Verify: `scripts/cargo.sh test -p guardedsleep-relay`.
- [ ] 1.2 Core `lan` mailbox: the LAN address in sealed `here` and in the mailbox update, and NSD candidates as inputs with the retry timing in the core. Verify: sim scenario "baby gets a new DHCP address offline → the parent recovers without re-pairing".

## 2. Android

- [ ] 2.1 Listener on the `relay` crate: lifecycle per active session, locks, the fixed default port with a fallback, the opaque NSD token, admission, closing unauthenticated sockets within 5 s. Verify: JVM tests, `just verify-android`.
- [ ] 2.2 The private-address cleartext rule: the network security config plus one platform check. Update the release-APK assertion so the base config stays restrictive. Verify the permission set for the pinned target SDK and update `add-android-release` 1.4 if needed. Verify: JVM tests, `just build-android` with the release check.
- [ ] 2.3 Parent NSD browse and admission tries. Copy for offline use and for a blocked LAN, in `locales/*.json`. Verify: JVM tests, Roborazzi in the pinned image.

## 3. Server local mode and web

- [ ] 3.1 Complete the server local mode (`--serve-web`, `--local` from W2-B): static UI and loopback API on 127.0.0.1 and ::1, `Origin`/`Host` checks, the LAN signaling port with Required admission, opening the default browser. Verify: `scripts/cargo.sh test -p guardedsleep-server`, `just smoke`, a security test for a foreign `Origin`.
- [ ] 3.2 The web client detects the local mode and adds the `lan` mailbox to its invite; honest offline copy. Verify: Vitest, Playwright against the local binary.

## 4. Verification

- [ ] 4.1 Host simulation and gate scenarios: Android ↔ Android without internet, Android ↔ PC in local mode with a browser parent (mDNS candidates), the uplink lost during LAN monitoring, and a DHCP change. Add `docs/ci-gates.md` rows. Verify: `scripts/cargo.sh test -p guardedsleep-sim`, `DOZE_MINUTES=2 just gate`.
- [ ] 4.2 Hotspot guide in `docs/` and the hardware-test round "offline LAN". Verify: `just lint-text`.

## Workflow follow-up

- Critical Reviewer pass on the LAN admission change and the cleartext rule.
- Add second-PC forwarding to `docs/backlog.md` as `IDEA`.
- `mise exec -- openspec archive add-offline-lan --yes`, then `openspec validate --all --strict`.
