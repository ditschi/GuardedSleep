# Backlog

Everything that is not current behaviour: decided work not started yet, ideas, deferred and rejected items. Current behaviour lives in [`openspec/specs/`](../openspec/specs/); work in progress lives in `openspec/changes/`.

- **STATUS tags:** `PLANNED` (decided, not started; has a target), `IDEA` (captured, not evaluated; no target unless the owner sets one), `DEFERRED` (reason + target phase), `REJECTED` (reason; never deleted).
- **Picking an item up:** start an OpenSpec change (`/opsx:propose <change-id>`), link the item from its proposal, and delete the item here in the same commit.
- **New ideas** go here as `IDEA`, under the matching capability.
- **Migration:** while ADRs and feature docs are migrated (see [`openspec-onboarding.md`](openspec-onboarding.md) §6), each capability's `IDEA`/`DEFERRED`/`REJECTED` items move here in the same commit. Until then, the feature docs still hold the ones not moved yet. Agent guardrails for off-limits features stay in `AGENTS.md` §9.

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

Concept and goals of ADR-013 are decided. Re-evaluate the UI realisation and detailed implementation on Android and the new web client, including a UX review (ambient bar, copy, baby-side indicator, onboarding on indicator loss).

## By capability

### pairing

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

### parent-alerts

- `DEFERRED` (Phase 2) With multi-parent: failover banner and escalation to another parent when the primary parent stops acknowledging heartbeats.
- `IDEA` Configurable alert behaviour beyond the built chain (which combination of sound, vibration and screen per alert type), vibration strength or pattern, and a cool-down between repeated alerts.
- `IDEA` Connection-context alert policy: full alert chain on unmetered Wi-Fi, a lighter chain on metered or cellular data.
- `IDEA` Opt-out of the Do Not Disturb override with a clear warning that alerts will be silent when the phone is muted.
- `IDEA` Re-prompt the test alert after an app update and before the first use on mobile data (alpha-ux-flows §3.9; built: readiness warns after 30 days).

### link-supervision

- `IDEA` Recovery by ICE restart on the existing peer connection, to save the DTLS handshake; measure new-attempt recovery over TURN on hardware first (ADR-017 Option A).
- `IDEA` Per-peer rate limit for `request_offer`: a ghost room member can keep replacing the baby's unverified attempt (verified attempts are unaffected) (ADR-017 §3a, open).

### offline-lan

- `IDEA` Chrome Local Network Access as a way for an HTTPS page to reach a LAN relay (ADR-021 §1 keeps HTTPS web ↔ phone listener `REJECTED`).

### i18n

- `IDEA` A "translation feedback" link next to machine-translated copy.

### dev-toolchain

- `IDEA` Kani proof for "alarm within T after link loss".
- `IDEA` `examples/` folder with PoC reproductions.
- `UNDER DISCUSSION` refactor-scout items: a shared `docker run` helper; YAML anchors for the duplicated CI path filters; one source for the NDK and cargo-ndk pins (Renovate regex manager); whether `stack-up` prints `SKIPPED` without Docker.
- `PLANNED` (wave 2/3) The generated Dev Container from ADR-020 §2; complete the ktfmt and uv pins once `reuse` ships a wheel.
