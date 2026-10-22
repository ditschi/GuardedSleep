# Backlog

Everything that is not current behaviour: decided work not started yet, ideas, deferred and rejected items. Current behaviour lives in [`openspec/specs/`](../openspec/specs/); work in progress lives in `openspec/changes/`.

- **STATUS tags:** `PLANNED` (decided, not started; has a target), `IDEA` (captured, not evaluated; no target unless the owner sets one), `DEFERRED` (reason + target phase), `REJECTED` (reason; never deleted).
- **Picking an item up:** start an OpenSpec change (`/opsx:propose <change-id>`), link the item from its proposal, and delete the item here in the same commit.
- **New ideas** go here as `IDEA`, under the matching capability.
- **Origin:** most items came from the former feature docs and ADRs when they were migrated to OpenSpec (2026-10-22, see the [decision archive](../openspec/changes/archive/README.md)). Agent guardrails for off-limits features stay in `AGENTS.md` §9.

## Planned

### Security audit before alpha

`PLANNED` — target: before the first `android-v*` alpha tag (owner, 2026-10-21; ADR-004 amendment).

A deep security audit of everything an attacker on the network, the relay or a nearby device can reach: pairing proof and PSK-derived room id, fingerprint parsing and the DTLS cross-check, relay admission, signaling validation and the parsers, key storage (Keystore/StrongBox, web memory-only), the LAN listener rules, the log canary, and the release APK surface. Open: who performs it (independent reviewer or agent-based review roles) and how findings gate the tag. Known gap to include: the Android client uses public STUN by default without telling the user who sees the IP address (AGENTS.md §7.1); fixed by `add-rust-signaling-server`.

### Baby-unit self-tests

`PLANNED` — target: detailed design after wave 3 (owner, 2026-10-21; amends ADR-010).

Trust the baby unit because the fundamentals keep working, not because of a hardware minimum:

- A gentle warning during the first few uses that an old phone is fine as long as these checks keep passing.
- Regular self-tests on the baby unit: the microphone still receives sound and is not disabled or muted (the core already reports `mic_silent` after 30 s without audio), the link and signaling work, battery and charging state where available.
- A safe alert on the parent for any potential issue; the parent checks manually and decides whether a phone is too old.
- Battery safety copy stays (ADR-010 Decision 3).

Design as an OpenSpec change touching `audio-monitoring` and `parent-alerts`.

### Alert audio routing UX

`PLANNED` — target: after wave 3 (owner, 2026-10-21; ADR-011).

ADR-011 is decided as the baseline. Re-evaluate speaker or headphone routing, call interruption and the volume settings with real users on the native app, to make sure the experience is good. Decided in ADR-011 but not built yet: the "Speaker + default device" dual route, a minimum gain during alerts, and automatic resume of alert or listen audio after a phone call with a banner if resume fails (ADR-018 T2).

### Connection model re-evaluation

`PLANNED` — target: after wave 3 (owner, 2026-10-21; ADR-012).

The goals of ADR-012 are fixed. During waves 1–3 the current logic stays as implemented. Afterwards, reconsider the state machine, path ranking and live handoff (ADR-012 §1, §2, §4) on the new base: the native app, the Rust `Link` (`link-supervision`) and the Rust relay, and fold the result into `link-supervision` and `offline-lan`.

### Connection-status UI review

`PLANNED` — target: after the rework (owner, 2026-10-21; ADR-013).

Concept and goals of ADR-013 are decided. Re-evaluate the UI realisation and detailed implementation on Android and the new web client, including a UX review (ambient bar, copy, baby-side indicator, onboarding on indicator loss). Decided but not built yet: the network-mode banner ("Now on mobile data", "Local — no data leaves your network") with a "Show connection details" setting (ADR-013 §4, ADR-018 T2); indicator-loss training in onboarding with the same components as in reality, followed by the test alert, and a reminder once per 30 days (ADR-013 §5, ADR-018 onboarding); a quality indicator with good/medium/poor (built: only a "weak connection" chip); a user-facing diagnostics view for technical details.

### Alpha exit criteria

`PLANNED` — target: before hardware tests (ADR-018 §5). Measured in the emulator gate and host simulation: noise → parent alert ≤ 2 s; outage ≤ 30 s → no level 2; outage ≥ 60 s → level 2 within 62 s of last contact and `link_restored` afterwards; one-tap start of a saved pairing → connected ≤ 15 s; wrong key and reflection attack rejected; automated baby-silence checks green (Android and web baby); role reviews (UX with screenshots, Security, Critical Reviewer) without open CRITICAL findings; hardware checklist written (`docs/contributing/alpha-hardware-test.md`). The first hardware round is a real-device smoke test (pairing, overnight foreground service, Wi-Fi flap, link-lost alarm).

## By capability

### pairing

- `IDEA` Saved connections as a list (up to 5, editable names, swipe to delete, auto-delete after 90 days without use with a 7-day warning) and a "Delete all connections & keys" panic button; today one pairing is stored and "Forget" deletes it.

- `DEFERRED` (Phase 2) Multi-parent: up to 4 parent units per baby (product cap); "Add device" on a paired baby or parent unit, the baby unit only after kiosk unlock and confirmation; a join request with device name after the invite is dismissed; failover of alerts to another parent when one stops responding (failure model F-1a).
- `DEFERRED` (Phase 2) Remote revocation of a device with re-keying, so a lost or stolen device cannot reconnect (failure model F-7).
- `DEFERRED` (Phase 2) In-app App-Lock (PIN or biometrics); the MVP relies on the OS screen lock (ADR-001).
- `DEFERRED` (Phase 2) Application-level end-to-end layer with HKDF-derived keys on top of DTLS (ADR-004 §3).
- `DEFERRED` (Phase 2) Remote pairing by invite code; the MVP requires a physical QR scan.
- `IDEA` Babysitter mode: a time-limited invite (6–24 h) whose pairing expires without manual revocation.
- `IDEA` Join notification to all paired devices; with more than two devices a second device confirms the join; audit log of joins and leaves.
- `IDEA` Key rotation of the pairing key (e.g. every 30 days), distinct from re-keying after revocation; `DEFERRED` per AGENTS.md §9.
- `IDEA` Forward secrecy with an ephemeral Diffie-Hellman exchange per connection.
- `IDEA` Emoji fingerprint shown on both units for an optional visual check (ADR-018 T2).
- `IDEA` Move the invite payload codec from Kotlin into the core, so the web client and Android share one parser (found during migration, fits the wave-2 façades).

### signaling-relay

- `UNDER DISCUSSION` Operate a project-hosted public signaling instance as the default in release builds (today: no default URL; owner, 2026-10-21: open). Needs an operator, a privacy statement and abuse handling.
- `PLANNED` (T1) Hardened TURN profile (coturn ≥ 4.15 or eturnal), HMAC credentials with TTL ≤ 10 min issued only after the pairing proof; Caddy + signaling (+ TURN) compose bundle with a self-hosting guide; Trivy image scanning (ADR-020 §6).
- `IDEA` Signaling rate limits beyond ADR-020 §6 (per room, per message; reverse proxy or in-process; mobile IP churn). Design first.
- `REJECTED` Signaling-free VPN mode (Tailscale/Headscale treated like LAN): cleartext signaling toward 100.64/10 is refused (ADR-021 §1); VPN works transparently through the normal relay (owner, 2026-10-21).

### audio-monitoring

- `DEFERRED` (Phase 2) Talk-back: the parent speaks through the baby unit's speaker (high parent expectation compared to hardware monitors); open: full or half duplex; only one child device at a time with several children. Until then the track stays muted and there is no UI (AGENTS.md §9).
- `IDEA` (Phase 3+) Sound categorisation on the device (crying, coughing, babbling, environmental noise, silence) for smarter alerts. Research notes (2026-04): YAMNet transfer learning with a compact classifier reaches about 95 % on cry detection with under 0.8 s latency; quantised TFLite models are 1–5 MB; MFCC or mel-spectrogram features with EMA smoothing; run only while charging or duty-cycled; risks are false positives from TV, pets and appliances. No TFLite, YAMNet or MFCC code before this is decided (AGENTS.md §9).
- `IDEA` (Phase 3+) Community data contribution for model training; privacy and legal questions first.
- `IDEA` Several alert levels (soft alert at a low threshold, urgent alert at a high one).
- `IDEA` Roaming cost warning when the internet path is first used on cellular data while roaming (communication.md "MVP", ADR-018 T2; not built).
- `IDEA` Session data estimate (heartbeat, silence frames, listen bursts) and, later, an optional per-session data ceiling that keeps the safety path.
- `IDEA` User settings for audio quality, bitrate or codec; show the audio delay in the parent UI. Open: which codec settings give the best quality per bandwidth for voice.
- `IDEA` Fall back from a muted track (Opus DTX) to `replaceTrack(null)` on mobile data if measurements show high silence-frame traffic.
- `IDEA` Nightlight mode on the baby unit (very dim warm glow), only if it cannot wake the child.
- `IDEA` Temperature and humidity display when the baby phone has such sensors.

### parent-alerts

- `DEFERRED` (Phase 2) With multi-parent: failover banner and escalation to another parent when the primary parent stops acknowledging heartbeats.
- `IDEA` Configurable alert behaviour beyond the built chain (which combination of sound, vibration and screen per alert type), vibration strength or pattern, and a cool-down between repeated alerts.
- `IDEA` Connection-context alert policy: full alert chain on unmetered Wi-Fi, a lighter chain on metered or cellular data.
- `IDEA` Opt-out of the Do Not Disturb override with a clear warning that alerts will be silent when the phone is muted.
- `IDEA` Change the baby unit's sensitivity remotely from the parent (ADR-018 T2).
- `IDEA` Re-prompt the test alert after an app update and before the first use on mobile data (alpha-ux-flows §3.9; built: readiness warns after 30 days).

### device-setup

- `IDEA` Role switching in the UI: switching to baby mode warns that the microphone will be active and asks for confirmation; switching to parent is immediate; "Start as [last role]?" on start. Today the role is chosen once (a different role deletes the pairing) and there is no switch UI.
- `DEFERRED` Auto-resume baby monitoring after a reboot: Android 14+ does not let a microphone foreground service start from the background (ADR-017 §9, ADR-018); "baby phone reboots overnight" stays a must-pass hardware scenario where the parent reaches level 2. `IDEA` (Phase 2) the same for the parent, more conservative.
- `IDEA` Tell the parent when the baby kiosk is unlocked ("Baby unit was unlocked"); with multi-parent, show "Add device" on the baby unit only after kiosk unlock and confirmation.
- `IDEA` OEM check: count heartbeats for about 30 minutes after battery-optimisation setup and warn if the OS seems to kill the app (ADR-018 T2 "30-minute heartbeat self-check").
- `IDEA` Estimated remaining runtime of the baby unit on the parent; tune screen-off and CPU throttling on the baby unit.
- `IDEA` Several baby units (one per child or room) with one parent app switching between them (use case UC-5).
- `IDEA` About/FAQ: hardware vs software baby monitors and range expectations (Wi-Fi, hotspot, internet).
- `IDEA` Responsive layout for tablets.

### link-supervision

- `IDEA` Recovery by ICE restart on the existing peer connection, to save the DTLS handshake; measure new-attempt recovery over TURN on hardware first (ADR-017 Option A).
- `IDEA` Per-peer rate limit for `request_offer`: a ghost room member can keep replacing the baby's unverified attempt (verified attempts are unaffected) (ADR-017 §3a, open).

### offline-lan

- `IDEA` Automatic LAN ↔ internet handoff in the same session (prefer the cheaper or local path again when it returns); switching is manual (AGENTS.md §9). Part of the "Connection model re-evaluation".
- `IDEA` Out-of-home and venue Wi-Fi: detect captive portals and client isolation after pairing or on reconnect, and suggest mobile data on both phones in plain words (use case UC-3).
- `IDEA` Exchange current address lists over the DataChannel on network changes (labels instead of full IPs, short TTL, never logged), and measure path quality to prefer the best path.
- `IDEA` Show "Secure local mode" when media stays on the local network (part of the connection-status UI review).

- `IDEA` Chrome Local Network Access as a way for an HTTPS page to reach a LAN relay (ADR-021 §1 keeps HTTPS web ↔ phone listener `REJECTED`).

### i18n

- `PLANNED` (T1) Weblate for volunteer translators (ADR-021 §2).
- `IDEA` A "translation feedback" link next to machine-translated copy.

### distribution

- `PLANNED` (T1) Release Please with the components `app` (one product version for Android and web), `server` and `core`; fastlane metadata for Play and F-Droid clients; CycloneDX SBOMs, `actions/attest` provenance and cosign-signed images; a build-twice reproducibility check (apksigcopier, diffoscope) (ADR-020 §10).
- `PLANNED` (beta) IzzyOnDroid (developer-signed, reproducibility-verified) (ADR-021 §3).
- `DEFERRED` F-Droid main repository until a from-source build of libwebrtc and JNA is sized; public wording says "via IzzyOnDroid" (ADR-021 §3).
- `PLANNED` (owner) Play organisation account with D-U-N-S if easy, otherwise a 14-day closed test with at least 15 testers as the beta; register developer verification now (ADR-021 §3).

### dev-toolchain

- `IDEA` Kani proof for "alarm within T after link loss".
- `IDEA` `examples/` folder with PoC reproductions.
- `IDEA` sccache for local agent worktrees (ADR-020 tier T2 → reconsider): the shared Cargo registry does not share compiled artefacts, so every new worktree still builds cold.
- `UNDER DISCUSSION` refactor-scout items: a shared `docker run` helper; YAML anchors for the duplicated CI path filters; one source for the NDK and cargo-ndk pins (Renovate regex manager); whether `stack-up` prints `SKIPPED` without Docker.
- `PLANNED` (wave 2/3) The generated Dev Container from ADR-020 §2; complete the ktfmt and uv pins once `reuse` ships a wheel.

### platforms

- `DEFERRED` (Phase 2) iOS: no Swift or Xcode code until the iOS gate; only `cargo check --target aarch64-apple-ios` in CI and a weekly bindings smoke test (AGENTS.md §9).
- `IDEA` Desktop installable client (Tauri or Electron around the web build): parent station on a PC; open questions are roles, auto-update and code signing. Today the desktop story is the PWA plus local PC mode.
- `IDEA` (Phase 3+) Video monitoring: motion-triggered video from the baby unit's camera, adaptive frame rate and resolution, night vision where the camera supports it, end-to-end encrypted. No camera capture or video pipeline before it is decided (AGENTS.md §9).

### later product ideas

- `DEFERRED` (Phase 2) Audio sensitivity calibration with a room baseline.
- `DEFERRED` (Phase 2) Event history and sleep log.
- `DEFERRED` Sleep and wake scheduling (start monitoring automatically at sleep times).

## Rejected

Never deleted; kept so the reasoning is not lost.

| Item                                                             | Reason                                                                                                   |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| White noise or lullaby playback                                  | Out of scope: GuardedSleep is a monitor, not a sound player.                                             |
| Audible alarm on the baby unit when the parent device fails      | Wakes the child; contradicts the core purpose (failure model, ADR-009).                                  |
| Cloud push, SMS or VoIP as an alarm when the parent phone is off | No cloud in the alarm path (vision non-goals).                                                           |
| Cloud-based audio analysis                                       | Violates privacy-first; adds latency and a cloud dependency.                                             |
| Continuous audio streaming to the parent                         | Wastes bandwidth; audio is sent only on demand.                                                          |
| Manual IP address entry for pairing                              | Poor UX, error-prone, exposes technical details.                                                         |
| Cloud-based device discovery                                     | Violates privacy-first; adds a cloud dependency.                                                         |
| Programmatic hotspot creation                                    | Not possible on mobile operating systems.                                                                |
| Full SDP in the QR code                                          | About 2500 bytes, beyond QR capacity; the v3 invite carries only the PSK, URLs and expiry.               |
| Tailscale-only internet mode                                     | VPN slot conflict, complex setup, privacy contradiction.                                                 |
| WireGuard embedded in the app                                    | VPN slot conflict remains; about 1000 lines of native code for little gain.                              |
| Signaling-free VPN mode                                          | See `signaling-relay` above (owner, 2026-10-21).                                                         |
| HTTPS web client ↔ phone listener with a self-signed certificate | Teaches users to accept certificate warnings, which undermines pairing as the trust anchor (ADR-021 §1). |
