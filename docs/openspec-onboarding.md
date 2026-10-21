# OpenSpec onboarding: GuardedSleep

STATUS: UNDER DISCUSSION (temporary onboarding note, 2026-10-21; delete once `openspec/` is set up).

Purpose: everything OpenSpec needs for the initial setup (`openspec init` → `openspec/project.md`, baseline specs, open changes), plus the decisions and reasoning from the toolchain-review session of 2026-10-20/21.

**Sources of truth for the details:** the ADRs win; this note only summarises them. Read `docs/decisions/ADR-020-toolchain-and-development-setup.md`, `ADR-021-offline-lan-topologies-locales-and-distribution.md`, ADR-001…019 (with their 2026-10-20 supersession notes), `AGENTS.md`, `docs/features/*.md`, and `docs/concept/vision.md`.

---

## 1. Content for `openspec/project.md`

### Product

- **GuardedSleep**: an open-source, privacy-first baby monitor that can run fully locally. Media never goes through third-party servers.
- **License:** AGPL-3.0-or-later. Trademark policy is in `docs/legal/trademark.md`.
- **Phase:** alpha build-out (ADR-018).
  - The native PoC passed the emulator gate AC-1…AC-4 on 2026-10-20.
  - AC-10 (link recovery, ADR-017) is next.
- **Users:**
  - Parents, typically sleep-deprived and using the app at 3 AM.
  - A baby unit, often an old spare Android phone or a laptop with its internal microphone.
  - A parent unit on Android or in a browser.

### Architecture

Imports flow downward only:

```
UI        Compose (apps/android) · Svelte 5 (apps/web)
State     UiContract models (Kotlin) · Svelte stores of the same shape
Platform  all I/O: Kotlin service/media/signaling · browser link driver
Bindings  UniFFI (guardedsleep-ffi) · wasm-bindgen + tsify (guardedsleep-wasm)
Core      Rust, sans-IO (guardedsleep-core): no I/O, threads, clocks; emits message keys, never prose
```

- **Crates:**
  - `guardedsleep-core`: protocol v3, pairing, Link/Attempt state machines, noise gate, escalation.
  - `guardedsleep-relay`: sans-IO rooms and admission. Shared by the server and the Android LAN listener.
  - `guardedsleep-server`: axum signaling server, built-in STUN, optional static web UI.
  - `guardedsleep-ffi`, `guardedsleep-wasm`: façade crates.
  - `guardedsleep-sim`: host simulation and traces.
  - `i18n-gen`, `uniffi-bindgen`.
- **Apps:**
  - `apps/android`: Kotlin + Compose. Modules app / ui-contract / platform / core-bindings.
  - `apps/web`: Vite 8 + Svelte 5 + vite-plugin-pwa, **no SvelteKit**.
  - `apps/e2e`: Playwright.
- **Legacy, frozen until deleted:**
  - `apps/mobile` (SvelteKit web client)
  - `apps/signaling-server` (Node)
  - `packages/protocol` (hand-written TS)
- **Hard rules:**
  - Safety logic lives once, in Rust.
  - Protocol types come only from Rust, and clients never parse the wire themselves.
  - UI never imports binding types. Gradle modules enforce this on Android; cargo-deny bans enforce it in Rust.

### Tech stack and tools (ADR-020)

- **Rust:**
  - 1.99.1 pinned exactly in `rust-toolchain.toml`, edition 2024.
  - A separate pinned nightly is used only for fuzzing and `cargo public-api`.
  - Tests: nextest, proptest, insta, golden and negative vectors through every binding.
  - Supply chain: cargo-deny.
- **Android:**
  - Latest stable AGP/Kotlin/Compose, targetSdk ≥ 36, manual DI.
  - Formatting and lint: ktfmt, Android Lint with warningsAsErrors.
  - Tests: Robolectric, Roborazzi + ATF (11 goldens), Maestro, KVM emulator gate.
  - WebRTC: `io.github.webrtc-sdk:android` behind a `MediaEngine` interface.
- **Web:**
  - pnpm 11 via mise, with `allowBuilds`, `minimumReleaseAge: 4320` and `blockExoticSubdeps`.
  - Prettier, `svelte-check --fail-on-warnings`, Vitest (logic), Playwright + axe.
- **Tooling:**
  - `just` is the single entry point: `setup fmt fmt-check lint test gen build build-android e2e gate ci dev`.
  - mise pins every non-Rust tool.
  - lefthook pre-commit (fmt + typos on staged files).
  - typos, CSpell for non-English locales, REUSE, actionlint, zizmor, betterleaks.
- **CI:**
  - GitHub Actions, every action SHA-pinned, `permissions: {}`.
  - `ci-ok` aggregator is the only required check.
  - Renovate: weekly, grouped, with a minimum release age.
- **Tiers:** T0 = alpha, T1 = beta, T2 = 1.0+. Don't build a gate before its tier.

### Conventions

- **Language:** English everywhere. User copy lives only in `locales/*.json`, with en as master.
- **Docs:** every doc item carries a STATUS tag: `IDEA | UNDER DISCUSSION | DECIDED | DEFERRED | REJECTED`.
- **ADRs:** status goes PROPOSED → DECIDED → SUPERSEDED. Rejected options are never deleted; they move to "Discarded alternatives".
- **Commits:** Conventional Commits, checked on the PR title only (squash merge). Types: build, chore, ci, docs, feat, fix, perf, refactor, revert, security, style, test.
- **Branch protection:** required `ci-ok` + linear history.
  - No required reviews, no signed-commit requirement, no agent bot identity.
  - Owner decision: the sole maintainer is admin.
- **Generated code:** commit nothing generated. Commit API-surface snapshots instead.
- **Config settings:** every version-specific setting carries a link to its documentation.
- **Tasks:** they print `SKIPPED: <task> needs <x>` when Docker, KVM or the SDK is missing. `GS_STRICT=1` (CI) turns a skip into a failure.
- **Tooling budget:** ≤ 12 h/month. Gates are owned in `docs/ci-gates.md`. A gate with no catch in 6 months is cut or demoted.

### Privacy and security guardrails (non-negotiable)

- **Media and alerts:** no third-party services in the media or alert path.
- **Telemetry:** none, unless an ADR and opt-in UX exist.
- **Crypto:** no custom transport crypto. WebRTC DTLS-SRTP is used, and HMAC/HKDF only for the pairing proof, room derivation and relay admission.
- **Pairing secrets:**
  - Never on disk in cleartext: on Android they are hardware-backed (Keystore/StrongBox); on the web they live only in memory.
  - Never in logs, Intents, URLs or backups.
  - A log-canary test proves it.
- **Validation:** all signaling payloads are validated in the core.
- **LAN relays:**
  - The phone listener and the PC relay use nonce + `HMAC(HKDF(PSK, "guardedsleep relay admission v1"), nonce ‖ direction ‖ room id)` admission.
  - A slot is assigned only after verification.
  - Unauthenticated sockets close within 5 s.
  - Nothing derived from the PSK is advertised.
- **Local PC mode:** the browser talks only to loopback. The binary checks Origin/Host, and only the signaling port binds to the LAN.
- **Android cleartext:** allowed only to private LAN addresses, checked in one place in code.

---

## 2. Proposed baseline capability specs (`openspec/specs/<capability>/spec.md`)

Derive these from the existing feature docs and ADRs. Write requirements in SHALL form, with WHEN/THEN scenarios.

| Capability                                                             | Main sources                               |
| ---------------------------------------------------------------------- | ------------------------------------------ |
| `pairing` (QR invite, direction-bound pairing proof, PSK storage)      | ADR-003, ADR-004, connectivity-pairing.md  |
| `signaling-relay` (rooms, admission, limits, STUN Binding, no IP logs) | ADR-003, ADR-020 §6, ADR-021 §1            |
| `link-supervision` (Link/Attempt state machines, recovery, escalation) | ADR-017, ADR-013                           |
| `offline-lan` (Android↔Android listener, local PC mode)                | ADR-012 (partially superseded), ADR-021 §1 |
| `audio-monitoring` (noise gate, listen, alerts)                        | audio-monitoring.md, ADR-011               |
| `parent-alerts` (escalation, alarm on silent/DND, wake)                | ADR-005, notifications-alerts.md           |
| `connection-status` (ambient glow, copy, observability)                | ADR-013, user-interface.md                 |
| `i18n` (catalogue, ICU subset, critical-key completeness, MT markers)  | ADR-021 §2, ADR-020 §7                     |
| `web-client` (full baby + parent roles, keep-awake guidance, limits)   | ADR-019 §7, ADR-020 §5                     |
| `distribution` (Play, GitHub APK, IzzyOnDroid, versioning)             | ADR-021 §3, ADR-020 §10                    |
| `dev-toolchain` (just vocabulary, pins, gates, tiers)                  | ADR-020                                    |

---

## 3. Decisions from this session, with reasoning

### Process

- **How the review ran:**
  - Five roles (Technical Architect, Security/Privacy Architect, DevEx & Release Engineer, Product Owner, UX) each designed a greenfield setup while ignoring the existing ADRs.
  - A Critical Reviewer raised 18 findings.
  - Every role responded.
  - A gap analysis compared the result with the repo.
  - The owner decided.
- **Report:** https://claude.ai/artifact/6tnaUtu9CrwNsv7PRwAuaW
- **Recorded in:** ADR-020 and ADR-021 (2026-10-20). Superseded points in ADR-001…019 are marked inline.

### Owner decisions (final)

| #   | Decision                                                                                                                                                             | Reasoning                                                                                                                                                                                                                  |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | Offline LAN covers Android↔Android **and** Android↔PC / PC↔PC. The PC runs the local GuardedSleep binary (web UI on localhost + LAN signaling). Target: beta.        | An HTTPS page cannot reach a phone without a self-signed certificate warning, and that would break pairing as the trust anchor. `http://localhost` is a secure context, so the local-binary design works in every browser. |
| D2  | Ship about 20 AI-translated locales: en, de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar, tr, uk, id. Native-speaker review improves them later. | Reach over perfection. A shipped locale must be complete for every critical key, so an alarm is never in mixed languages. Arabic is included to prove RTL early.                                                           |
| D3  | Use a Play organisation account (D-U-N-S) if easy; otherwise the 14-day closed test is the beta. Register developer verification now.                                | The 12-tester rule applies to personal accounts. Developer verification goes global in 2027 and affects sideloading.                                                                                                       |
| D4  | Alpha: Play + GitHub APK. Beta: IzzyOnDroid. F-Droid main is DEFERRED.                                                                                               | F-Droid main builds from source, and both the libwebrtc AAR and the JNA natives are prebuilt.                                                                                                                              |
| D5  | Rebuild the web client as `apps/web` on plain Vite + Svelte 5; `apps/mobile` is reference only.                                                                      | SvelteKit 3 had been GA for only three weeks and agents write 1.x/2.x patterns. The app has about 5 screens and no SSR.                                                                                                    |
| D6  | Rust signaling server now, not after the alpha.                                                                                                                      | One fuzzed validator shared with the clients, and the relay logic is shared with the Android listener.                                                                                                                     |
| D7  | Tooling budget ceiling of 12 h/month.                                                                                                                                | Estimated T0 cost is 7–13 h/month for one owner.                                                                                                                                                                           |
| D8  | No required reviews, no agent identity, no signed commits.                                                                                                           | The owner is sole admin and would bypass them. Squash merges are signed by GitHub.                                                                                                                                         |

### Notable tool choices (rejected alternatives in brackets)

- `just` [mise tasks, make]
- mise + pinned build image [Nix, Dev Container-first]
- UniFFI [hand-written JNI, Diplomat]
- Svelte 5 [SvelteKit 3, Leptos/Dioxus, Compose for Web]
- Prettier + svelte-check [Biome (Svelte support experimental), ESLint]
- Built-in STUN [public STUN by default]
- webrtc-sdk [GetStream m125]
- Roborazzi with 11 goldens [a full screenshot matrix]
- JSON + ICU + Rust generator [Fluent, Android XML as source]
- DTCG tokens via the Rust generator [Style Dictionary now]
- lefthook [pre-commit, Husky, treefmt]
- Manual tag at alpha, then release-please [cocogitto, semantic-release]
- Renovate [Dependabot]
- cargo-deny now, cargo-vet at beta

### Lessons and facts found during implementation

- **typos corrupted test data.** The `typos` pre-commit hook had "corrected" a base64 room id in a committed trace. Generated vectors, traces and scenarios are now excluded from typos.
- **pnpm 11:**
  - `onlyBuiltDependencies` became `allowBuilds`.
  - `minimumReleaseAge` is in **minutes**.
  - corepack is not used; pnpm is pinned via mise.
- **WebRTC update rule:** a libwebrtc SLA based on milestones would block every vendor. It is CVE-based instead: an unpatched critical CVE 30 days after the Chrome fix triggers an ADR.
- **Agent worktrees** are created from an older base. Agents must `git rebase <wave branch>` first.

---

## 4. Work in flight, to model as OpenSpec changes

Branches: `docs/toolchain-realignment` → `chore/toolchain-wave-1` (→ `-wave-2`, `-wave-3`). Nothing pushed yet.

- **`toolchain-wave-1`: guardrails and cleanup.** DONE, reviewed, closing GO.
  - Pins: rust-toolchain, edition 2024, workspace lints, panic-free parsers.
  - Tooling: mise, just, lefthook, pnpm 11, cargo-deny, REUSE, betterleaks.
  - CI: SHA pins, ci-ok, Renovate.
  - Removed: Capacitor shell and hybrid E2E.
  - A final cleanup batch (refactor-scout + closing findings, shellcheck) is being merged.
- **`toolchain-wave-2`: structure.** Planned (`wave2-plan.md` in the session scratchpad).
  - Group 1, in parallel:
    - ffi/wasm façade crates, tsify types, API snapshots
    - relay crate + Rust server (admission, STUN, static UI), deleting the Node server
    - WebRTC swap to webrtc-sdk
  - Group 2, after group 1: Gradle build-logic and modules, ktfmt, Lint, R8, versionCode from versionName.
- **`toolchain-wave-3`: web and quality gates.** Planned:
  - Web: `apps/web` rebuild; delete `apps/mobile` and `packages/protocol`.
  - i18n: about 20 locales, MT markers, critical-key completeness.
  - Design: DTCG tokens + contrast test.
  - Visual and a11y tests: Roborazzi a11y + 11 goldens, Playwright axe + overflow.
  - Security tests: log canary, 2 fuzz targets.
- **Product changes:** `offline-lan` (Kotlin listener on the relay crate, local PC mode) and the T1 items:
  - release-please, SBOM/attest/cosign
  - CodeQL, Trivy, cargo-vet
  - TURN profile, Weblate, OEM soak, diagnostics export
  - IzzyOnDroid

### Owner to-dos

- Run `just setup` once. Git hooks are off until then: the stale Husky `core.hooksPath` is unset by the recipe.
- Start Play account / D-U-N-S and developer verification.
- Generate the offline direct-APK signing key (do not store it in CI).

---

## 5. Open points to decide or capture in OpenSpec

- **refactor-scout items needing a decision:**
  - a shared `docker run` helper
  - YAML anchors for the duplicated path filters
  - a single source for the NDK/cargo-ndk pins (Renovate regex manager)
  - whether `stack-up` should SKIP without Docker
- **ADR-020 §2 "generated Dev Container"** is not done yet. The ktfmt and uv pins are incomplete (reuse 6.2.0 has no wheel).
- **IDEAs:**
  - Chrome Local Network Access path
  - Kani for "alarm within T after link loss"
  - a "translation feedback" link
  - `examples/` folder for PoC reproductions
- **Choose how OpenSpec and ADRs coexist.** Suggestion: ADRs stay the record of decisions with their alternatives; OpenSpec specs hold the requirements; changes reference the ADR they implement.
