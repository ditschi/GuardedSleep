# OpenSpec onboarding: GuardedSleep

Archived 2026-10-22: the onboarding note written for the switch to OpenSpec, kept as the design record of the toolchain review of 2026-10-20/21 and of the ADR migration.

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

## 5. Open points

Moved to [`backlog.md`](../../../../docs/backlog.md) (2026-10-21). How OpenSpec and ADRs coexist is decided: the ADRs are migrated and then removed (section 6).

---

## 6. ADR → OpenSpec migration (in progress)

Goal: one system. `docs/decisions/` disappears; every ADR ends up as an archived change, and its outcome lives in `openspec/specs/`. Work locally, **one capability per commit**, after wave 1 is merged.

### Decisions on the approach (owner, 2026-10-21)

- **Specs are the current truth; the archive is history.** Planned work runs propose → apply → `/opsx:archive`, which merges the delta specs and moves the change to `changes/archive/<date>-<id>/`. Migrated ADRs reach the archive by hand (verbatim `git mv`, decision date, no `tasks.md`); their specs are written as `ADDED` deltas of a migration change.
- **Wave 1 is not ported.** Its outcome reaches the specs when ADR-020 is migrated into `dev-toolchain`; git history and ADR-020 keep the rest.
- **Waves 2 and 3 become OpenSpec changes**, smaller than a wave (wave 2: e.g. `add-ffi-wasm-facades`, `add-rust-signaling-server`, `swap-webrtc-sdk`, `split-android-modules`). A change can only modify requirements that exist, so the capabilities a wave touches are migrated before its changes are proposed (wave 2: `pairing`, `signaling-relay`).
- **Remaining capabilities are migrated with the skills.** The planner runs `/opsx:explore` on the source ADRs and feature docs, then `/opsx:propose migrate-<capability>`. Everything clear is answered without asking; every real doubt (contradiction, code drift, unclear status, scope) is asked interactively and the answer is recorded in the change's `design.md`. Then `/opsx:apply`, `/opsx:archive` and one commit. The `link-supervision` pilot is not redone; its spec is reviewed in the `pairing` + `signaling-relay` run.
- **Backlog:** `IDEA`, `DEFERRED`, `REJECTED` and decided-but-not-started (`PLANNED`) items live in [`backlog.md`](../../../../docs/backlog.md); GitHub issues once outside contributors arrive.
- **Formerly `PROPOSED` ADRs are decided:** 004 amendment, 009, 010 (amended), 011, 012 (goals), 013 (concept), 015, 016. The re-evaluations attached to 010–013 and the security audit for 004 are `PLANNED` in the backlog.

### Procedure per capability (one commit)

1. Write `openspec/specs/<capability>/spec.md` from the ADR sections and the `DECIDED` feature-doc items listed below. Resolve contradictions in favour of the newest ADR, check numbers against the code, and note what changed in the commit message.
2. Add a `Migrated to OpenSpec` note under the header of each source ADR, naming the sections it now covers. Those sections become normative in the spec; the rest of the ADR stays binding.
3. In the feature doc, replace the migrated `DECIDED` text with a link to the spec, and move the capability's `IDEA`, `DEFERRED` and `REJECTED` items to [`backlog.md`](../../../../docs/backlog.md).
4. When **every** section of an ADR is migrated (or the ADR is archive-only), archive it in the same commit:
   - `git mv` the ADR to `openspec/changes/archive/<decided-date>-adr-NNN-<slug>/design.md` (verbatim, so options, discarded alternatives and the `§` numbers cited in code comments stay valid);
   - add `proposal.md` (Why = the ADR's Context in two or three sentences; What Changes = the decision; Capabilities = the specs it fed);
   - update `docs/decisions/README.md`.
5. Run `just lint-text` (includes `openspec validate --all --strict`) and `openspec validate --archived`.

Finish: once the tracker is empty, delete `docs/decisions/`, rewrite the precedence list in `AGENTS.md`, `.cursor/rules/decision-workflow.mdc` and `docs-conventions.mdc`, and delete this note.

### Tracker

`DECIDED*`: decided by the owner on 2026-10-21 with a `PLANNED` re-evaluation or follow-up in the backlog; migrate the decided part now and leave the follow-up in the backlog.

Mapping is a proposal; adjust it while migrating. "Archive-only" means the outcome is a project rule already held in `AGENTS.md` or `openspec/config.yaml`, not a behaviour spec.

| ADR                                    | Status    | Goes to                                                                                       | Migrated                        |
| -------------------------------------- | --------- | --------------------------------------------------------------------------------------------- | ------------------------------- |
| 001 MVP scope, PoC gate, auth          | DECIDED   | `pairing` (auth); scope archive-only (ADR-018)                                                |                                 |
| 002 client platform, repo structure    | partly S  | archive-only (superseded by 015/019/020)                                                      |                                 |
| 003 WebRTC, signaling, QR pairing      | DECIDED   | `pairing`, `signaling-relay`                                                                  |                                 |
| 004 encryption, key storage            | DECIDED\* | `pairing`                                                                                     |                                 |
| 005 parent alerts, naming              | DECIDED   | `parent-alerts`; naming/ASO archive-only; distribution → `distribution`                       |                                 |
| 006 release, changelog                 | DECIDED   | `distribution`                                                                                |                                 |
| 007 browser E2E strategy               | DECIDED   | `dev-toolchain`                                                                               |                                 |
| 008 container images                   | DECIDED   | `distribution`                                                                                |                                 |
| 009 personas, failure model            | DECIDED   | archive-only (personas → `config.yaml` context)                                               |                                 |
| 010 baby-unit hardware                 | DECIDED\* | `audio-monitoring` (self-tests)                                                               |                                 |
| 011 parent audio routing               | DECIDED\* | `audio-monitoring`                                                                            |                                 |
| 012 connection model, LAN, handoff     | DECIDED\* | `offline-lan`; §1 retries superseded by `link-supervision`                                    |                                 |
| 013 connection-status copy             | DECIDED\* | `connection-status`                                                                           |                                 |
| 014 AGPL, trademark                    | DECIDED   | archive-only                                                                                  |                                 |
| 015 native clients, shared Rust core   | DECIDED   | archive-only (layer rules in `AGENTS.md` §5.1)                                                |                                 |
| 016 test strategy                      | DECIDED   | `dev-toolchain`                                                                               |                                 |
| 017 link recovery, supervision         | DECIDED   | `link-supervision` (§2–§9, §11), `pairing` (§1), `signaling-relay` (§10), §12 into scenarios  | §2–§9, §11 → `link-supervision` |
| 018 alpha scope, trust features        | DECIDED   | `parent-alerts`, `audio-monitoring`, `connection-status`; scope archive-only                  |                                 |
| 019 language, repo, client policy      | DECIDED   | `web-client`; rest archive-only                                                               |                                 |
| 020 toolchain                          | DECIDED   | `dev-toolchain`; §5 → `web-client`, §6 → `signaling-relay`, §7 → `i18n`, §10 → `distribution` |                                 |
| 021 offline LAN, locales, distribution | DECIDED   | `offline-lan` (§1), `i18n` (§2), `distribution` (§3)                                          |                                 |

Suggested order (closes ADRs early): `pairing` + `signaling-relay` (closes 003, 004, 017), `parent-alerts`, `connection-status`, `audio-monitoring`, `offline-lan`, `i18n`, `distribution`, `web-client`, `dev-toolchain`, then the archive-only ADRs.

### Findings from the `link-supervision` pilot

- **ADRs drift from the code.** ADR-017 §11 lists the sans-IO surface, but the core gained `attempt_dtls_fingerprint`, `set_noise_gate` and `send_probe_tone`. The spec points at `LinkInput`/`LinkAction` instead of repeating the list.
- **Feature docs contradicted the ADR.** `connectivity-pairing.md` still said "heartbeat 5–30 s, loss after 2 missed beats"; ADR-017 says 5–15 s and `2 × interval + 5 s`. Migrating forces these out.
- **`--strict` caps a requirement at 500 characters**, which forces one behaviour per requirement. ADR-017 §2–§11 became 25 requirements with 31 scenarios; the §12 simulation cases became scenarios.
- **Most ADRs feed several capabilities**, so an ADR is archived only when its last capability is migrated. The section-level `Migrated to OpenSpec` note keeps precedence unambiguous meanwhile.
- **Archive format works:** an entry with only `proposal.md` and `design.md` passes `openspec validate --archived`, and `--all` ignores the archive.
- **Seven ADRs were still `PROPOSED`** although in force (015 is the architecture). Resolved by the owner on 2026-10-21, see "Decisions on the approach".
