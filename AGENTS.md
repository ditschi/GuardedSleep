# AGENTS.md — GuardedSleep

> **What is this file?** A tool-agnostic briefing for AI coding agents (Claude Code, Cursor, Codex, Cline, Aider, Copilot, …) working in this repository. It holds **rules, conventions and links**, not prose. Human onboarding lives in [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).
>
> **Source of truth precedence** (highest first):
>
> 1. [`openspec/specs/`](openspec/specs/) — what the product and toolchain do today (built behaviour, `SHALL` requirements with scenarios). Binding.
> 2. Active changes in [`openspec/changes/`](openspec/changes/) — decided work that is not built yet (prepared wave changes, offline LAN, Android release). Binding for that work once applied.
> 3. This file, then [`docs/backlog.md`](docs/backlog.md) (`PLANNED`, `IDEA`, `DEFERRED`, `REJECTED`).
> 4. [`docs/concept/`](docs/concept/vision.md) and [`docs/technical/`](docs/technical/sw-architecture.md) — non-normative background (vision, personas, failure model, architecture overview, screens).
> 5. [`.cursor/rules/`](.cursor/rules/) (Cursor only; mirrors this file).
>
> History: the 21 former ADRs and every archived change are in the [decision archive](openspec/changes/archive/README.md); references such as "ADR-017 §6" resolve there. **If this file disagrees with a spec, the spec wins. File a `chore(docs): sync AGENTS.md` change.**

---

## 1. Project snapshot

- **Product:** GuardedSleep – Privacy Baby Monitor. Open-source, privacy-first, **fully local-capable** (no cloud dependency for media).
- **Names:** npm scope `@guardedsleep/*`, Android app id `org.guardedsleep.app` (alpha: `org.guardedsleep.app.alpha`), Docker images `guardedsleep-*`.
- **Phase:** alpha build-out ([ADR-018](openspec/changes/archive/2026-10-20-adr-018-alpha-milestone-scope-and-trust-features/design.md)). The tooling is **in transition** to ADR-020, implemented in three waves: **wave 1** guardrails and cleanup (done), **wave 2** structure (crates, server, Android modules), **wave 3** web client and quality gates. Items marked "(planned, wave N)" do not exist yet: do not look for them or invent them.
- **Platforms:** native Android (Kotlin + Compose) and a web client (Svelte 5 on the WASM core), both on one Rust core ([ADR-015](openspec/changes/archive/2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md), [ADR-019](openspec/changes/archive/2026-10-20-adr-019-language-repository-and-client-policy/design.md)). iOS, desktop apps and video are deferred (§9).
- **License:** AGPL-3.0-or-later ([`LICENSE`](LICENSE), [ADR-014](openspec/changes/archive/2026-07-07-adr-014-open-source-license-agpl-and-trademark/design.md)); trademark policy in [`docs/legal/trademark.md`](docs/legal/trademark.md).

## 2. Tech stack (target per ADR-020; see §3 for what exists today)

| Area             | Choice                                                                                                                                        |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared core      | **Rust**, sans-IO, edition 2024, `forbid(unsafe_code)`, RustCrypto only. Façades: `ffi` (UniFFI, exact pin) and `wasm` (wasm-bindgen + tsify) |
| Android          | Kotlin + Jetpack Compose, latest stable AGP/Kotlin/Compose, `io.github.webrtc-sdk:android` behind a `MediaEngine` interface                   |
| Web client       | `apps/web`: **Vite 8 + Svelte 5 + vite-plugin-pwa**, TypeScript strict, on the WASM core. **No SvelteKit.** Full client for both roles        |
| Signaling server | **Rust axum** on the sans-IO `relay` crate; built-in STUN; can serve the static web UI (offline PC mode)                                      |
| Protocol types   | Rust core is the source of truth; TypeScript types are generated into `packages/core-wasm`                                                    |
| Tooling          | `just` (entry point), mise (tool pins), lefthook (hooks), Renovate, cargo-deny; Rust tooling where it shares protocol logic                   |
| Tests            | nextest, proptest, insta, golden + negative vectors through every binding; Robolectric/Roborazzi; Vitest; Playwright; Maestro; emulator gate  |
| Crypto / media   | WebRTC DTLS-SRTP + DataChannel; direction-bound pairing proof (ADR-004 amendment). No custom transport crypto                                 |

**Gone:** the legacy SvelteKit + Capacitor web client and `docker/android-build` (the Capacitor image); do not look for them or recreate them.

## 3. Repository map

```
crates/
  guardedsleep-core/    Rust sans-IO core (protocol v3, pairing, Link/Attempt); vectors + traces      [exists; short name `core`]
  guardedsleep-sim/     Host simulation (gs-scenario), trace recording                                [exists; short name `sim`]
  i18n-gen/ , uniffi-bindgen/                                                                         [exist]
  public-lists/         Validator and generator for data/*.json (curated relays and STUN) -> web module, Kotlin, CSP origins [exists]
  guardedsleep-ffi/     UniFFI façade (libguardedsleep_ffi.so, Kotlin bindings); api.txt snapshot      [exists; short name `ffi`]
  guardedsleep-wasm/    wasm-bindgen + tsify façade for the web client; api.txt snapshot                [exists; short name `wasm`]
  guardedsleep-relay/   Sans-IO signaling relay: rooms, limits, LAN admission                          [exists]
  guardedsleep-server/  Rust signaling server (axum), built-in STUN, optional local web UI            [exists]
apps/
  android/              Native client (Kotlin + Compose); modules app, ui-contract, platform, core-bindings [exists; build-logic holds the Gradle conventions]
  web/                  Vite 8 + Svelte 5 web client (PWA)                                             [exists]
  e2e/                  Playwright browser E2E
packages/
  core-wasm/            WASM build of the core + generated TS types
  protocol/             LEGACY, deleted in wave 2
data/                   Curated public Nostr relays and STUN servers (nostr-relays.json, stun-servers.json); `just lint` checks them
locales/                UI copy catalogue (en master), ~20 locales planned, wave 2 (4 today; §5.2a)
docker/                 signaling-server, web, stack, android-native-build, android-emulator
scripts/                core-build.sh, cargo.sh, poc/ (emulator gate), lint-ui-literals.sh, …
docs/                   backlog.md, ci-gates.md, concept, technical (reference only), roles, reviews, contributing, legal
openspec/               OpenSpec: config.yaml, specs/ (current behaviour), changes/ (prepared work), changes/archive/ (history incl. ADRs), §6
                        docs/ci-gates.md (gate owners, runbooks, last-catch dates)
mise.toml, mise.lock, rust-toolchain.toml, lefthook.yml, renovate.json, justfile
```

Per-package `AGENTS.md` files may exist later; they inherit this file and document only deltas.

## 4. Commands

**`just` is the single entry point**, with the fixed vocabulary `setup fmt lint test gen build e2e gate ci` (sub-recipes such as `fmt-check`, `lint-text`, `build-android` are allowed). Recipes call the native tools and never re-implement them; `just ci` gives the same verdict as CI (lint + test + build + e2e; the PR title check and the emulator gate are CI-only). Check `just --list`.

- Tasks needing Docker, KVM, the Android SDK or Playwright browsers print `SKIPPED: <task> needs <x>` and exit 0. A SKIPPED is not a pass. CI sets `GS_STRICT=1`, which turns every skip into a failure.
- `just test` builds no Android ABI (host Rust, wasm, Vitest, Android JVM tests against the host build of the core). Cross-compiled ABIs belong to `just build-android`, `just gate` and release builds.
- The core graph checks (`just deny`, dependency count) run on the all-features graph of `guardedsleep-core`. `just lint` also diffs the binding surface (`just api-check`; regenerate with `just api-snapshot` after an intended change): `crates/guardedsleep-{ffi,wasm}/api.txt` and `packages/core-wasm/api/core.d.ts`, using the nightly pinned in `tools/nightly/rust-toolchain.toml`. Only the façades may allow `unsafe`; the core forbids it and carries no binding attributes (its optional `tsify` feature is enabled by `guardedsleep-wasm` alone).

| Task                       | Command                                                                                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Setup (tools, deps, hooks) | `just setup` (mise, rustup toolchain, `pnpm install --frozen-lockfile`, lefthook)                                                                |
| Format                     | `just fmt` · `just fmt-check`                                                                                                                    |
| Lint                       | `just lint` (= `lint-text` + `lint-workflows` + `lint-code`; clippy runs with `--all-features`, plus `cargo check` for `aarch64-apple-ios`)      |
| Test                       | `just test` (cargo workspace, WASM conformance, Vitest, Android JVM tests) · `scripts/cargo.sh test -p guardedsleep-sim` (host simulation)       |
| Generate                   | `just gen` (i18n output) · `scripts/core-build.sh host` (host core library + Kotlin bindings) · `scripts/core-build.sh wasm`                     |
| Build                      | `just build` (web + Android) · `just build-web` · `just build-android` · `just verify-android` (adds JVM tests and Roborazzi)                    |
| Browser E2E                | `just e2e` (Playwright; starts `just dev`)                                                                                                       |
| Emulator gate (KVM)        | `just gate` (`DOZE_MINUTES=2 just gate` for a quick run; run `just build-android` first)                                                         |
| Local dev                  | `just dev` (Vite 5173 + Rust signaling server 8787)                                                                                              |
| Docker stack               | `just stack-lan` · `just stack-up`                                                                                                               |
| Supply chain, secrets      | `just deny` · `just secrets` · `just audit`                                                                                                      |
| Server smoke test          | `just smoke` (built `guardedsleep-server` binary, hello + signal relay over a WebSocket)                                                         |
| Delta tests, timings       | `just test-affected [base]` (tests for the changed paths only; `node scripts/lib/affected.mjs --dry-run` shows the plan) · `just timings [days]` |

Before a commit lefthook runs Prettier, cargo fmt, typos, hygiene and Betterleaks on staged files; CI re-runs everything. Never bypass hooks; re-run `just lint` manually before declaring "done".

## 5. Coding conventions

### 5.1 Layers (DECIDED, ADR-015, ADR-019, ADR-020)

Imports flow **only downward**:

```
UI         Compose (apps/android …/ui) · Svelte components (apps/web)
  ↓
State      StateFlow models (UiContract.kt) · Svelte stores of the same shape
  ↓
Platform   Kotlin service/media/signaling · browser link driver — all I/O lives here
  ↓
Bindings   UniFFI (ffi) · wasm-bindgen (wasm)
  ↓
Core       Rust, sans-IO (core, relay): no I/O, threads or clocks; emits message keys, never prose
```

- **Android:** `ui-contract` is the port between State and Platform: `platform` implements `AppController` and emits contract models, `app` only consumes them; `ui-contract` depends on neither.
- **Safety logic lives once, in the Rust core** (pairing, link supervision, escalation, noise gate, validation). Platform reconnect loops or timer-based alarms are bugs.
- **Protocol shapes come from the Rust core.** Never hand-edit a second schema.
- **UI renders contract models only** (`UiContract.kt`; web stores mirror it). UI code never imports UniFFI/WASM types. In Android only `platform` may depend on `core-bindings`; `app` reaches platform components through `ui-contract` models and the wiring interfaces.
- **`relay` is shared** by the server and the Android LAN listener; the platform owns the sockets.
- **cargo-deny** bans tokio, mio, getrandom and socket crates from the `core` and `relay` graphs. This keeps them sans-IO: do not work around it. Binding attributes never appear in `core`.
- **No SvelteKit:** `$app/*` and `+page` files do not exist in `apps/web`. Capacitor was removed in wave 1.
- Cross-cutting new modules → propose an OpenSpec change first (`/opsx:propose`).

### 5.2 Style and tooling rules

- `.editorconfig`: LF, UTF-8, 2 spaces. Prettier `printWidth: 100`, `proseWrap: preserve`; no per-file overrides. Filenames `kebab-case.ts`/`.svelte` for modules, `PascalCase.svelte` for components. Named exports (default only for Svelte components). Comments explain **why**, not what.
- **Pins live in one place each:** `rust-toolchain.toml` (exact stable), `mise.toml` + `mise.lock` (node, pnpm 11, java, just, lefthook, typos, ktfmt, …), Gradle version catalog + wrapper. **Do not align the fuzz toolchain** (`fuzz/rust-toolchain.toml`, pinned nightly for cargo-fuzz and `cargo public-api`) **with the main pin.** pnpm comes from mise; no corepack.
- **Every version-specific config setting carries a comment with a doc link** (settings go stale, e.g. pnpm 11 `allowBuilds`).
- **Commit nothing generated** (bindings, generated TS, i18n output). API-surface snapshots (`cargo public-api` on `ffi`/`wasm`, the generated `.d.ts`) are committed instead; CI generates twice and compares hashes.
- **Never hand-edit screenshot baselines.** They are recorded only in the pinned build image.
- `svelte-ignore` and `@ts-expect-error` need an inline reason. No `@ts-ignore`, no `any` in production code, no ESLint/Biome (Prettier + `svelte-check --fail-on-warnings`).
- **Commit messages:** Conventional Commits are checked on the **PR title only**, in CI (squash merges). There is no local commit-msg hook.
- **Dependencies:** Renovate (weekly, grouped, `minimumReleaseAge` 3 days; UniFFI and wasm-bindgen get own PRs; no automerge for runtime/crypto deps). Never invent versions; add via the package manager. Justify new dependencies in the PR. Do not silence audit findings.
- **Tooling budget:** maintenance ceiling is **12 h/month**. Every gate has an owner, runbook and last-catch date in `docs/ci-gates.md`; do not add a gate without an entry there.

### 5.2a Human language and i18n (ADR-021)

- Repository language is **English**: docs, comments, identifiers, hard-coded strings in source.
- **All user-visible copy lives only in `locales/*.json`** (en master) and is generated by `crates/i18n-gen` into Android resources and a typed web module. Never write literals in UI code or paste translations into source; the literal lint enforces it.
- **Locale set:** about 20 AI-translated locales (en, de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar, tr, uk, id). **Adding locales is allowed** the same way, without a policy change.
- **Critical-key completeness:** a locale is generated only if every key tagged `critical` (alerts, link loss, connection, safety) is present, so alarms are never mixed-language. Other keys may fall back to English.
- Each non-English value carries a `machineTranslated` or `reviewed` marker. Review improves a locale; it does not gate shipping. Keep placeholders and plural categories intact; the generator validates them.
- Spellcheck: `typos` (English), CSpell for non-English catalogues. Maestro and Playwright select by stable ids, never by copy text.

### 5.3 Language rules

- **Rust:** `cargo fmt`, `clippy -D warnings`; `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` are denied in parser modules; no `unsafe` outside generated FFI glue; every module has unit tests; behaviour changes update vectors/traces (bless, review the diff by hand).
- **Kotlin:** official style (ktfmt), no `!!` in production code, I/O off the main thread, stable `testTag` ids (snake_case, shared with the web), manual constructor DI.
- **TypeScript:** strict mode; Svelte 5 runes; focus moves to the screen heading on route change.

### 5.4 Tests

- Next to the code: cargo test/nextest (+ proptest, insta, `guardedsleep-sim` scenarios), JUnit/Robolectric/Roborazzi, Vitest (logic only), Playwright with axe.
- Device behaviour (Doze, FGS, alerts, recovery) is proven by the emulator gate before hardware tests. A test that flakes twice in seven days is quarantined with an issue.

## 6. Documentation and decision conventions

- **Specs and changes (OpenSpec):** every behaviour, scope, security-model or architecture change is an OpenSpec change: `/opsx:propose` (proposal, design, delta specs, tasks) → review → `/opsx:apply` → `/opsx:archive`, which merges the deltas into `openspec/specs/`. Specs describe what is **built**; decided but unbuilt work lives in a change. Tooling-only changes set `skip_specs: true`. `openspec validate --all --strict` runs in `just lint-text`. Files under `.claude/*/openspec-*`, `.claude/commands/opsx` and `.cursor/*/opsx*` are generated by `openspec update`, never hand-edited.
- **Design and rejected options:** a change's `design.md` lists options as Pros / Cons / Recommendation and keeps rejected options under "Discarded alternatives"; never delete them (archived changes keep them).
- **Backlog and STATUS tags:** [`docs/backlog.md`](docs/backlog.md) holds `PLANNED` (decided, not started, with a target), `IDEA`, `DEFERRED` (reason + target phase) and `REJECTED` (reason) items by capability. User ideas go there as `IDEA`; assign no phase unless the user decides one. Picking an item up means proposing a change and deleting the item. `docs/concept/` and `docs/technical/` keep their STATUS tags but are reference only.
- **Presenting options:** a checklist with Pros / Cons / Recommendation ([`decision-workflow.mdc`](.cursor/rules/decision-workflow.mdc)); skip it when there is one reasonable approach.
- **Role coordination** ([`role-coordination.mdc`](.cursor/rules/role-coordination.mdc), [`docs/roles/`](docs/roles/README.md)): for non-trivial work act as Coordinator. New feature: Product Owner + Technical Architect (+ Security/Privacy for security, + UX for user-facing). Architecture decision: Technical Architect + Security Architect. MVP scoping: Product Owner + Critical Reviewer. **Critical Review Gate:** if a role's output changes architecture, scope, security model, a prior decision or a core concept, the Critical Reviewer is mandatory and the originator answers every `CRITICAL`/`IMPORTANT` finding. Unresolved disagreements go to the user as a §6.3-style checklist; the user has final authority. Agents without sub-agents list the role perspectives they applied in the PR description.

## 7. Privacy and security guardrails (NON-NEGOTIABLE)

1. **No third-party services in the media or alert path.** Media, alerts and heartbeats never transit a third party, except an encrypted, user-entered TURN. Signaling may use the vetted EU/EEA/CH mailboxes and STUN servers in `data/` (criteria: `openspec/specs/signaling-mailboxes`, "Curated public services"; being built in `alpha-connection-model`), only as sealed envelopes, so operators see only the items in "Relay-visible data", with disclosure and an off switch on each device. Only STUN hosts in `data/stun-servers.json` are allowed, enforced by lint.
2. **No cleartext PSKs or pairing material on disk.** Native: hardware-backed key (Android Keystore/StrongBox). Web: memory only, never `localStorage`, `sessionStorage`, IndexedDB or the Cache API (a test will enforce it, planned, wave 2). Never in Intents, Bundles, URLs or logs.
3. **No custom transport crypto.** Media and DataChannel use DTLS-SRTP/DTLS. HKDF-SHA256 with one info string per purpose derives every key from the PSK; HMAC-SHA256 only for the pairing, parent and baby proofs, the pin tags, admission and the Nostr tag. XChaCha20-Poly1305 only for the signaling envelope, with 192-bit nonces from the in-core CSPRNG seeded freshly by the platform at every start; a nonce never repeats under a key. BIP-340 only to sign Nostr events, never to authenticate the peer. Secret types use `secrecy`/`zeroize`.
4. **Validate all signaling payloads**; the server treats every message as untrusted. Room ids are HKDF-derived, never reversible to the PSK.
5. **No telemetry, crash reporting or analytics** without an OpenSpec change and an opt-in UX.
6. **No raw PII in logs** (IPs, device names, fingerprints). The server logs no IPs by default. The **log-canary test** (simulation and gate runs pair with a known PSK; CI fails if it appears in any log in any encoding) must pass once added (planned, wave 2).
7. **No new `*.key`, `*.pem`, `.env`, `credentials.json`**; never commit signing keys. The direct-distribution APK key is generated offline and never stored in CI.
8. **LAN listener rules (ADR-021 §1, phone and PC):** bind only while a session is active; the browser talks only to its own local binary over loopback (UI and loopback API bind to `127.0.0.1`/`::1`, `Origin` and `Host` are checked), only the LAN signaling port binds to the LAN; admit a joiner only after a nonce challenge (relay sends a fresh nonce, joiner answers `HMAC(HKDF(PSK, "guardedsleep relay admission v1"), nonce ‖ direction ‖ room id)`, planned, wave 2); close unauthenticated sockets within 5 s; never advertise anything derived from the PSK; Android cleartext is allowed toward private LAN addresses by the network security config (planned, wave 2; today debug allows cleartext and release none), and one platform-layer check rejects any non-private destination before connecting. LAN signaling is an untrusted relay: end-to-end security comes from the pairing proof and DTLS fingerprint binding.
9. **Supply chain:** cargo-deny (advisories, AGPL-compatible licences, crates.io only), dependency-count guard on `core`, `pnpm audit`, SHA-pinned Actions with `permissions: {}`. Release-APK assertions: no trackers, expected permissions, backup exclusion, `debuggable=false`.

## 8. Git, PRs and releases

- **Conventional Commits** are enforced on the **PR title** in CI (types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `security`, `style`, `test`). Breaking changes: `<type>!:` plus a `BREAKING CHANGE: <description>` footer in the PR description.
- **Branch protection on `main`:** required `ci-ok` status check + linear history, squash merges. **No required reviews, no signed-commit requirement, no separate agent identity** (ADR-020 §9). Target `main`; never push to it directly, never force-push, never `--no-verify`, never `git config` in an agent run.
- **Releases (ADR-020 §10):** at alpha, a **manual `android-v*` tag**; `versionCode` is derived from `versionName` in `version.properties`, never edited by hand. Release Please is **T1** (beta) with components `app` (one product version for Android and web), `server` and `core`. Do not edit released sections of `CHANGELOG.md`; only `## [Unreleased]`. The old npm-centric components (protocol, signaling-server, mobile) are gone.
- **Images** ([ADR-008](openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md), amended by ADR-020): `guardedsleep-signaling` and `guardedsleep-web` on GHCR, built on native amd64 and arm64 runners (no QEMU); `:dev` for every `main` push.
- **PR checklist:** Conventional title; lint/test green via `just ci`; `openspec validate` green; behaviour, architecture, scope or security changes go through an OpenSpec change (link it in the PR).

## 9. Not-yet-implemented features — agent guardrails

Off-limits unless the task explicitly references the matching OpenSpec change or backlog item:

| Feature                                              | Status                                                                                                    | Agent rule                                                                                                                                                                                                                                               |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| In-app App-Lock (PIN / biometrics)                   | `DEFERRED` Phase 2 (ADR-001)                                                                              | MVP relies on OS screen lock.                                                                                                                                                                                                                            |
| iOS build                                            | `DEFERRED` Phase 2                                                                                        | No Swift/Xcode code until the iOS gate; only the `cargo check --target aarch64-apple-ios` CI check and a weekly bindings smoke test.                                                                                                                     |
| Talk-back (parent → baby audio)                      | `DEFERRED` Phase 2                                                                                        | Track stays muted; no UI.                                                                                                                                                                                                                                |
| Multi-parent (>1 parent device)                      | `DEFERRED` Phase 2 (max 4)                                                                                | Hard-cap at 1 baby + 1 parent.                                                                                                                                                                                                                           |
| Application-level E2E layer (HKDF)                   | `DEFERRED` Phase 2 (ADR-004)                                                                              | DTLS + fingerprint binding is the baseline.                                                                                                                                                                                                              |
| Key rotation                                         | `DEFERRED` Phase 2                                                                                        | Pairing key valid until manual re-pair.                                                                                                                                                                                                                  |
| Offline LAN                                          | `DECIDED`, target **alpha** (ADR-021; owner 2026-10-23, `add-offline-lan` after `alpha-connection-model`) | Topologies: Android↔Android (embedded listener) and Android↔PC via local PC mode (server binary on `http://localhost`). PC↔PC offline (second-PC forwarding) is `IDEA`. HTTPS web ↔ phone listener is `REJECTED`; Chrome Local Network Access is `IDEA`. |
| LAN ↔ Internet automatic handoff                     | `DECIDED`, alpha (`alpha-connection-model`)                                                               | No mode switch: the core uses all mailboxes in parallel. Do not build platform-side failover.                                                                                                                                                            |
| Remote pairing (invite code)                         | `DEFERRED` Phase 2                                                                                        | MVP requires a physical QR scan.                                                                                                                                                                                                                         |
| i18n                                                 | `DECIDED` (ADR-021): about 20 AI-translated locales, en master                                            | See §5.2a. Adding locales is allowed through the catalogue and generator only.                                                                                                                                                                           |
| Distribution                                         | Play + GitHub APK at alpha; IzzyOnDroid at beta; **F-Droid main `DEFERRED`**                              | No promise of F-Droid main (prebuilt libwebrtc and JNA); say "via IzzyOnDroid".                                                                                                                                                                          |
| TURN, cargo-vet, SBOM/attest/cosign, CodeQL, Weblate | T1 (beta); alpha allows only a user-entered TURN                                                          | No default or project-run TURN before beta.                                                                                                                                                                                                              |
| Style Dictionary, OSS-Fuzz, sccache in CI            | T2 (local sccache is T0)                                                                                  | Not before 1.0.                                                                                                                                                                                                                                          |
| Video monitoring                                     | `IDEA` Phase 3+                                                                                           | No camera capture or video pipeline.                                                                                                                                                                                                                     |
| On-device AI / sound categorization                  | `IDEA` Phase 3+                                                                                           | No TFLite / YAMNet / MFCC code.                                                                                                                                                                                                                          |
| Desktop installable client (Tauri/Electron)          | `IDEA`                                                                                                    | PWA + local PC mode is the desktop story.                                                                                                                                                                                                                |
| Audio sensitivity calibration / sleep log            | `DEFERRED` Phase 2                                                                                        | Out of MVP.                                                                                                                                                                                                                                              |
| White noise / lullaby playback                       | `REJECTED`                                                                                                | Permanently out of scope.                                                                                                                                                                                                                                |
| Signaling rate limits beyond ADR-020 §6              | `IDEA`                                                                                                    | Design first as an OpenSpec change (backlog: `signaling-relay`).                                                                                                                                                                                         |
| Telemetry / crash reporting                          | not in docs                                                                                               | Requires an OpenSpec change + opt-in UX.                                                                                                                                                                                                                 |

When in doubt, read the matching spec in `openspec/specs/` and the backlog entry, and ask instead of inventing.

## 10. Tips for agents

- **Read before you write.** Open the relevant spec, change (or its archived design) and module first; matching the layer matters more than matching surface syntax.
- **Tasks print SKIPPED** without Docker/KVM; a SKIPPED is not a pass. Say so in your report.
- **Never invent dependency versions**; query the registry or lockfile.
- **Model routing (plan strong, implement cheap):** the session model does planning, architecture, OpenSpec proposals, role reviews and final verification. **Implementation is delegated** to the `implementer` sub-agent ([`.claude/agents/implementer.md`](.claude/agents/implementer.md), Sonnet); this rule is the user's standing request to spawn it. Hand over a self-contained brief (goal, files, layer rules, acceptance checks); the planner reviews the diff and runs lint/tests. Exceptions: trivial one-line edits and docs/ADR wording stay with the planner.
- **Cleanup pass (per task):** after implementation and green checks, before the final commit/PR, the planner runs [`refactor-scout`](.claude/agents/refactor-scout.md) in diff mode against `origin/main`; accepted findings go to the `implementer`, then re-run checks. Once per task; skip for docs-only, config-only or diffs under ~50 changed code lines.
- **Refactor sweep (weekly):** the [`refactor-sweep`](.claude/skills/refactor-sweep/SKILL.md) procedure: the scout proposes up to 5 behavior-preserving improvements, the planner triages, the `implementer` works each in its own worktree off `origin/main`, the planner verifies and opens one **draft PR** per finding (label `refactor-sweep`, max 3 open). Needs-an-ADR items are reported, not implemented. Sweeps never merge or push to `main`.
- **Cross-IDE etiquette:** tool-specific files complement, never replace, this file; mirror their intent here.

### 10.1 Machine load and parallel agents

Load comes from what an agent runs, not from how many agents run. Budgets are per machine and derived, never hard-coded here.

- **Machine profile:** `just machine-profile` measures cores, RAM and KVM once and writes the git-ignored `.cache/machine-profile.env` (`just setup` runs it; re-run after a hardware change). Rule of thumb: `GS_MEDIUM_SLOTS = max(1, min(cores/4, RAM_GB/6))`, `GS_HEAVY_SLOTS = max(1, min(cores/8, RAM_GB/12))`. Override with the same variables in the environment. Read the file once per session; do not probe the machine again.
- **Weight classes:**
  - _light_: reading, reviews, docs, OpenSpec artifacts, small edits checked with targeted commands (no full builds);
  - _medium_: building or testing one area, `just test`, `just lint`, `just build`;
  - _heavy_: `just gate` (emulators), `just e2e`, Docker image builds, full Android ABI builds.
- **Slots enforce it:** medium and heavy recipes run through `scripts/lib/slot.sh <class>`, a machine-wide `flock` semaphore in `/tmp` shared by all worktrees and sessions. Without a free slot a job waits (bounded by `GS_SLOT_WAIT`, default 3600 s) instead of overloading the machine. Slots are re-entrant for nested recipes.
- **Coordinator rule:** start light agents freely (practical limit about 6, set by coordination effort, not by the machine). Start at most `GS_MEDIUM_SLOTS` agents that build or test at the same time; heavy phases queue on their own. Prefer one agent per large work package over splitting it into agents that rebuild the same tree.
- **Targeted checks per step, full checks once:** after each step run `just test-affected` (maps the changed paths through `.github/path-filters.yml`, the same map CI uses, and falls back to the full `just test` for global or unmapped paths) or the affected crate/module/package check directly; run the full `just lint` / `just test` (plus any `just e2e` / `just gate` the planner names) once at the end of a work package. `just timings` shows where time goes (every slot-held run is recorded).
- **Shared caches:** the build-image scripts share the Cargo registry and the Gradle home across worktrees (`${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}`); each worktree keeps its own `target/`. Local container builds also share a size-capped sccache (`GS_SCCACHE_SIZE`, default 10G; `GS_SCCACHE=0` turns it off), so a new worktree reuses compiled dependencies; it is off in CI and for the generate-twice check. The first build in a new worktree is still cold (several minutes); that is expected, not a hang.

### 10.2 Commands that must finish

- **Every long command gets a deadline:** wrap it in `timeout N`, below the tool limit (10 min for a foreground call). Longer runs go to the background and the harness completion notice is the only signal.
- **Never write polling loops** (`until …; do sleep …; done`, `while ! …`) and never use `pgrep -f <pattern>` as a wait condition: the pattern matches the polling shell's own command line, so the loop never ends. A waited-for output file also stays empty forever when its producer was killed (for example by a rate limit).
- **Leave nothing running:** servers start only with a trap that stops them (Playwright manages `just dev` itself); the gate stops its emulator containers. Before reporting, an agent checks that no process or container it started is still running. `just reap` lists leftovers (dev servers, stuck shells, project containers) and stops them on request. In Claude Code a Stop/SubagentStop hook (`.claude/settings.json`) runs the listing after every turn and shows leftovers to the user; it never stops anything. Finished agent worktrees: `just reap --worktrees` lists the ones that are clean and contained in a pushed branch, `just reap --prune-worktrees` removes exactly those (never forced, never one in use).
- **Report progress through commits:** commit after each step so a stalled or cut-off agent loses nothing, and the coordinator can check progress with `git log` instead of waiting blind.
- **Deadlines start after the slot:** set `GS_RUN_TIMEOUT` instead of an outer `timeout` for medium/heavy recipes; `slot.sh` starts it once the slot is held and exits 124 when the run exceeds it, while `GS_SLOT_WAIT` bounds the queue (exit 75 = machine busy, rerun or background it, not a test failure). Foreground: `GS_SLOT_WAIT=60 GS_RUN_TIMEOUT=480 just test`; background: a long `GS_RUN_TIMEOUT` and no outer `timeout`. An outer `timeout` still counts slot waits.

### 10.3 Agent worktrees, interruptions and hooks

- **Rebase first:** agent worktrees can start from an old base. The first command in every worktree is `git rebase <wave branch>`; the brief names the expected `git log -1`.
- **Interrupted agents:** usage limits cut agents off mid-run. Commits survive, uncommitted edits stay in the worktree. Before resuming, check `git -C <worktree> status` and `just reap`, then resume the same agent (it keeps its context) instead of starting a new one.
- **Hooks must be active:** after `just setup`, `git config --get core.hooksPath` must not point at `.husky`; lefthook runs fmt, typos and the secret scan on staged files. With hooks off, broken commits slip through; `just lint` warns locally (`scripts/lib/hooks-check.sh`), so act on that warning by running `just setup`.
- **Merge hotspots:** `docs/ci-gates.md` (one large table), `justfile` and AGENTS.md conflict when parallel changes add rows or recipes. Resolve row by row and keep both sides; never take one side wholesale.
- **Verify claims:** the coordinator checks an agent's report against the tree (`git log`, the change's `tasks.md`, the check logs) before merging or archiving; "passed" in a report is not evidence.

## 11. Maintenance of this file

Update it when the stack, repository layout, commands, spec set, role process, guardrails or the §9 matrix change, and when a wave lands (drop the "(planned, wave N)" marker and the matching "current commands" row). Keep it under ~300 lines; move detail to `docs/contributing/*.md` and link it.
