# AGENTS.md — GuardedSleep

> **What is this file?** A tool-agnostic briefing for AI coding agents (Claude Code, Cursor, Codex, Cline, Aider, Copilot, …) working in this repository. It holds **rules, conventions and links**, not prose. Human onboarding lives in [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).
>
> **Source of truth precedence** (highest first):
>
> 1. [Architecture Decision Records](docs/decisions/README.md) — `DECIDED` items are binding. The newest ADRs win: [ADR-020](docs/decisions/ADR-020-toolchain-and-development-setup.md) (toolchain) and [ADR-021](docs/decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) (offline LAN, locales, distribution) supersede older tooling, locale and distribution statements.
> 2. [`docs/concept/vision.md`](docs/concept/vision.md), [`docs/features/`](docs/features/README.md), [`docs/technical/`](docs/technical/sw-architecture.md).
> 3. This file.
> 4. [`.cursor/rules/`](.cursor/rules/) (Cursor only; mirrors this file).
>
> **If this file disagrees with an ADR, the ADR wins. File a `chore(docs): sync AGENTS.md` change.**

---

## 1. Project snapshot

- **Product:** GuardedSleep – Privacy Baby Monitor. Open-source, privacy-first, **fully local-capable** (no cloud dependency for media).
- **Names:** npm scope `@guardedsleep/*`, Android app id `org.guardedsleep.app` (alpha: `org.guardedsleep.app.alpha`), Docker images `guardedsleep-*`.
- **Phase:** alpha build-out ([ADR-018](docs/decisions/ADR-018-alpha-milestone-scope-and-trust-features.md)). The tooling is **in transition** to ADR-020, implemented in three waves: **wave 1** guardrails and cleanup, **wave 2** structure (crates, server, Android modules), **wave 3** web client and quality gates. Items marked "(planned, wave N)" do not exist yet: do not look for them or invent them.
- **Platforms:** native Android (Kotlin + Compose) and a web client (Svelte 5 on the WASM core), both on one Rust core ([ADR-015](docs/decisions/ADR-015-native-clients-with-shared-rust-core.md), [ADR-019](docs/decisions/ADR-019-language-repository-and-client-policy.md)). iOS, desktop apps and video are deferred (§9).
- **License:** AGPL-3.0-or-later ([`LICENSE`](LICENSE), [ADR-014](docs/decisions/ADR-014-open-source-license-agpl-and-trademark.md)); trademark policy in [`docs/legal/trademark.md`](docs/legal/trademark.md).

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

**Legacy, frozen, never extend:** `apps/mobile` (SvelteKit + Capacitor; reference for the `apps/web` rebuild, deleted at parity), `apps/signaling-server` (Node, replaced by the Rust server), `packages/protocol` (hand-written TS types, to be deleted), `docker/android-build` (Capacitor image).

## 3. Repository map

```
crates/
  guardedsleep-core/    Rust sans-IO core (protocol v3, pairing, Link/Attempt); vectors + traces      [exists; short name `core`]
  guardedsleep-sim/     Host simulation (gs-scenario), trace recording                                [exists; short name `sim`]
  i18n-gen/ , uniffi-bindgen/                                                                         [exist]
  guardedsleep-{ffi,wasm,relay,server}/                                                              (planned, wave 2)
apps/
  android/              Native client (Kotlin + Compose); :core = UniFFI bindings + JNI libs          [exists; modules app/ui-contract/platform/core-bindings planned, wave 2]
  web/                  Vite 8 + Svelte 5 web client                                                   (planned, wave 3)
  e2e/                  Playwright browser E2E
  mobile/               LEGACY, frozen reference     signaling-server/   LEGACY Node relay, frozen
packages/
  core-wasm/            WASM build of the core + generated TS types
  protocol/             LEGACY, deleted in wave 2
locales/                UI copy catalogue (en master), ~20 locales planned, wave 2 (4 today; §5.2a)
docker/                 signaling-server, mobile-web, stack, android-native-build, android-emulator
scripts/                core-build.sh, cargo.sh, poc/ (emulator gate), lint-ui-literals.sh, …
docs/                   concept, features, decisions (ADRs), technical, roles, reviews, contributing, legal
                        docs/ci-gates.md (gate owners, runbooks, last-catch dates)                      (planned, wave 1)
mise.toml, mise.lock, rust-toolchain.toml, lefthook.yml, renovate.json                                  (planned, wave 1)
```

Per-package `AGENTS.md` files may exist later; they inherit this file and document only deltas.

## 4. Commands

**Target: `just` is the single entry point**, with the fixed vocabulary `setup fmt lint test gen build e2e gate ci` (planned, wave 1; sub-recipes such as `fmt-check` and `build-android` are allowed). Recipes call the native tools and never re-implement them; `just ci` gives the same verdict as CI. Prefer `just <task>` as soon as the recipe exists (a `justfile` is already in the repo; check `just --list`).

- (planned, wave 1) Tasks needing Docker, KVM or the Android SDK print `SKIPPED: <task> needs <x>` and exit 0. CI sets `GS_STRICT=1`, which turns every skip into a failure.
- Today only `just test|lint|build|gate|e2e` (plus `install`, `test-*`) exist; missing Docker makes them fail, not skip, and `just test` still builds the Android ABIs.
- (planned, wave 1) `just test` builds no Android ABI (host Rust, wasm, Vitest, Android JVM tests). Cross-compiled ABIs belong to `just build-android`, `just gate` and release builds.

**Current commands until wave 1 lands** (these keep working; do not delete them):

| Task                   | Command                                                                                                                                                                                                        |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Install JS deps        | `corepack enable && pnpm install --frozen-lockfile` (becomes pnpm 11 via mise, no corepack)                                                                                                                    |
| Lint / format          | `pnpm run lint` (= `pre-commit run --all-files`), `pnpm run format`, `pnpm run format:check`                                                                                                                   |
| Rust fmt, clippy, test | `scripts/core-build.sh test` (Docker image; cargo is not needed on the host)                                                                                                                                   |
| Core for Android / web | `scripts/core-build.sh android` · `scripts/core-build.sh wasm`                                                                                                                                                 |
| Host simulation        | `scripts/cargo.sh test -p guardedsleep-sim`                                                                                                                                                                    |
| Android unit + build   | in `apps/android`, in the `guardedsleep-android-native-build:local` image: `./gradlew :core:testDebugUnitTest :app:testDebugUnitTest :app:assembleDebug` (wrap concurrent runs in `flock target/.gradle.lock`) |
| Emulator gate (KVM)    | `scripts/poc/local-gate.sh` (`DOZE_MINUTES=2` for a quick run) · `scripts/poc/release-check.sh` · `scripts/poc/size-check.sh`                                                                                  |
| i18n                   | `pnpm run i18n:check` · `pnpm run i18n:gen`                                                                                                                                                                    |
| Local dev / E2E        | `pnpm run dev:local` (Vite 5173, signaling 8787) · `pnpm run e2e` (Playwright)                                                                                                                                 |
| JS tests / build       | `pnpm test` · `pnpm run build` · `pnpm --filter @guardedsleep/signaling-server smoke` (legacy server)                                                                                                          |
| Docker stack           | `pnpm run stack:lan` · `docker compose -f docker/stack/compose.yaml up --build`                                                                                                                                |
| Dependency audit       | `pnpm run audit`                                                                                                                                                                                               |

Before a commit the current Husky hook runs `pre-commit run` and commitlint; after wave 1 lefthook runs formatting + typos on staged files, and CI re-runs everything. Never bypass hooks; re-run the lint task manually before declaring "done".

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

- **Safety logic lives once, in the Rust core** (pairing, link supervision, escalation, noise gate, validation). Platform reconnect loops or timer-based alarms are bugs.
- **Protocol shapes come from the Rust core.** Never hand-edit a second schema; never extend `packages/protocol`.
- **UI renders contract models only** (`UiContract.kt`; web stores mirror it). UI code never imports UniFFI/WASM types. In Android only `platform` may depend on `core-bindings` (wave 2).
- **`relay` is shared** by the server and the Android LAN listener; the platform owns the sockets.
- **cargo-deny** bans tokio, mio, getrandom and socket crates from the `core` and `relay` graphs. This keeps them sans-IO: do not work around it. Binding attributes never appear in `core`.
- **No SvelteKit:** `$app/*` and `+page` files do not exist in `apps/web`. Capacitor is legacy and is removed in wave 1.
- Cross-cutting new modules → propose in `docs/technical/sw-architecture.md` or open an ADR first.

### 5.2 Style and tooling rules

- `.editorconfig`: LF, UTF-8, 2 spaces. Prettier `printWidth: 100`, `proseWrap: preserve`; no per-file overrides. Filenames `kebab-case.ts`/`.svelte` for modules, `PascalCase.svelte` for components. Named exports (default only for Svelte components). Comments explain **why**, not what.
- **Pins live in one place each:** `rust-toolchain.toml` (exact stable), `mise.toml` + `mise.lock` (node, pnpm 11, java, just, lefthook, typos, ktfmt, …), Gradle version catalog + wrapper. **Do not align the fuzz toolchain** (`fuzz/rust-toolchain.toml`, pinned nightly for cargo-fuzz and `cargo public-api`) **with the main pin.** pnpm comes from mise; no corepack.
- **Every version-specific config setting carries a comment with a doc link** (settings go stale, e.g. pnpm 11 `allowBuilds`).
- **Commit nothing generated** (bindings, generated TS, i18n output). API-surface snapshots (`cargo public-api` on `ffi`/`wasm`, the generated `.d.ts`) are committed instead; CI generates twice and compares hashes.
- **Never hand-edit screenshot baselines.** They are recorded only in the pinned build image.
- `svelte-ignore` and `@ts-expect-error` need an inline reason. No `@ts-ignore`, no `any` in production code, no ESLint/Biome (Prettier + `svelte-check --fail-on-warnings`).
- **Commit messages:** Conventional Commits are checked on the **PR title only**, in CI (squash merges). Local commit-msg hooks go away in wave 1.
- **Dependencies:** Renovate (weekly, grouped, `minimumReleaseAge` 3 days; UniFFI and wasm-bindgen get own PRs; no automerge for runtime/crypto deps). Never invent versions; add via the package manager. Justify new dependencies in the PR. Do not silence audit findings.
- **Tooling budget:** maintenance ceiling is **12 h/month**. Every gate has an owner, runbook and last-catch date in `docs/ci-gates.md`; do not add a gate without an entry there.

### 5.2a Human language and i18n (ADR-021)

- Repository language is **English**: docs, comments, identifiers, hard-coded strings in source.
- **All user-visible copy lives only in `locales/*.json`** (en master) and is generated by `crates/i18n-gen` into Android resources and a typed web module. Never write literals in UI code or paste translations into source; the literal lint enforces it.
- **Locale set:** about 20 AI-translated locales (en, de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar, tr, uk, id). **Adding locales is allowed** without a new ADR, in the same way.
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

- **STATUS tags** (mandatory in `docs/`): `IDEA`, `UNDER DISCUSSION`, `DECIDED`, `DEFERRED` (reason + target phase), `REJECTED` (reason). User ideas go under the matching feature doc as `IDEA`; assign no phase unless the user decides one.
- **ADR workflow:** architecture, scope, security-model or core-concept changes need an ADR in [`docs/decisions/`](docs/decisions/README.md) (`ADR-NNN-short-title.md`, `PROPOSED → DECIDED → SUPERSEDED`). Never delete rejected options.
- **Presenting options:** a checklist with Pros / Cons / Recommendation ([`decision-workflow.mdc`](.cursor/rules/decision-workflow.mdc)); skip it when there is one reasonable approach.
- **Role coordination** ([`role-coordination.mdc`](.cursor/rules/role-coordination.mdc), [`docs/roles/`](docs/roles/README.md)): for non-trivial work act as Coordinator. New feature: Product Owner + Technical Architect (+ Security/Privacy for security, + UX for user-facing). Architecture decision: Technical Architect + Security Architect. MVP scoping: Product Owner + Critical Reviewer. **Critical Review Gate:** if a role's output changes architecture, scope, security model, a prior decision or a core concept, the Critical Reviewer is mandatory and the originator answers every `CRITICAL`/`IMPORTANT` finding. Unresolved disagreements go to the user as a §6.3-style checklist; the user has final authority. Agents without sub-agents list the role perspectives they applied in the PR description.

## 7. Privacy and security guardrails (NON-NEGOTIABLE)

1. **No third-party services in the media or alert path.** Signaling and STUN carry only connection metadata. Public STUN is opt-in only, with UI copy naming who sees the IP.
2. **No cleartext PSKs or pairing material on disk.** Native: hardware-backed key (Android Keystore/StrongBox). Web: memory only, never `localStorage`, `sessionStorage`, IndexedDB or the Cache API (a test will enforce it, planned, wave 2). Never in Intents, Bundles, URLs or logs.
3. **No custom transport crypto.** DTLS-SRTP + DTLS; HMAC/HKDF only for the pairing proof and room derivation. Use `secrecy`/`zeroize` for secret types.
4. **Validate all signaling payloads**; the server treats every message as untrusted. Room ids are HKDF-derived, never reversible to the PSK.
5. **No telemetry, crash reporting or analytics** without an ADR and an opt-in UX.
6. **No raw PII in logs** (IPs, device names, fingerprints). The server logs no IPs by default. The **log-canary test** (simulation and gate runs pair with a known PSK; CI fails if it appears in any log in any encoding) must pass once added (planned, wave 2).
7. **No new `*.key`, `*.pem`, `.env`, `credentials.json`**; never commit signing keys. The direct-distribution APK key is generated offline and never stored in CI.
8. **LAN listener rules (ADR-021 §1, phone and PC):** bind only while a session is active; the browser talks only to its own local binary over loopback (UI and loopback API bind to `127.0.0.1`/`::1`, `Origin` and `Host` are checked), only the LAN signaling port binds to the LAN; admit a joiner only after a nonce challenge (relay sends a fresh nonce, joiner answers `HMAC(HKDF(PSK, "guardedsleep relay admission v1"), nonce ‖ direction ‖ room id)`, planned, wave 2); close unauthenticated sockets within 5 s; never advertise anything derived from the PSK; Android cleartext is allowed toward private LAN addresses by the network security config, and one platform-layer check rejects any non-private destination before connecting. LAN signaling is an untrusted relay: end-to-end security comes from the pairing proof and DTLS fingerprint binding.
9. **Supply chain:** cargo-deny (advisories, AGPL-compatible licences, crates.io only), dependency-count guard on `core`, `pnpm audit`, SHA-pinned Actions with `permissions: {}`. Release-APK assertions: no trackers, expected permissions, backup exclusion, `debuggable=false`.

## 8. Git, PRs and releases

- **Conventional Commits** are enforced on the **PR title** in CI (types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `security`, `style`, `test`). Breaking changes: `<type>!:` plus a `BREAKING CHANGE: <description>` footer in the PR description.
- **Branch protection on `main`:** required `ci-ok` status check + linear history, squash merges. **No required reviews, no signed-commit requirement, no separate agent identity** (ADR-020 §9). Target `main`; never push to it directly, never force-push, never `--no-verify`, never `git config` in an agent run.
- **Releases (ADR-020 §10):** at alpha, a **manual `android-v*` tag**; `versionCode` is derived from `versionName` in `version.properties`, never edited by hand. Release Please is **T1** (beta) with components `app` (one product version for Android and web), `server` and `core`. Do not edit released sections of `CHANGELOG.md`; only `## [Unreleased]`. The old npm-centric components (protocol, signaling-server, mobile) are legacy.
- **Images** ([ADR-008](docs/decisions/ADR-008-container-image-build-and-distribution.md), amended by ADR-020): `guardedsleep-signaling` and `guardedsleep-web` on GHCR, built on native amd64 and arm64 runners (no QEMU); `:dev` for every `main` push.
- **PR checklist:** Conventional title; lint/test green via `just ci` (planned, wave 1; until then `just lint` + `just test`, or the current commands in §4); docs respect STATUS tags; an ADR exists for architecture/scope/security changes.

## 9. Not-yet-implemented features — agent guardrails

Off-limits unless the task explicitly references the matching ADR / feature file and updates `STATUS`:

| Feature                                              | Status                                                                       | Agent rule                                                                                                                                                                                                                                      |
| ---------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| In-app App-Lock (PIN / biometrics)                   | `DEFERRED` Phase 2 (ADR-001)                                                 | MVP relies on OS screen lock.                                                                                                                                                                                                                   |
| iOS build                                            | `DEFERRED` Phase 2                                                           | No Swift/Xcode code until the iOS gate; only the `cargo check --target aarch64-apple-ios` CI check and a weekly bindings smoke test.                                                                                                            |
| Talk-back (parent → baby audio)                      | `DEFERRED` Phase 2                                                           | Track stays muted; no UI.                                                                                                                                                                                                                       |
| Multi-parent (>1 parent device)                      | `DEFERRED` Phase 2 (max 4)                                                   | Hard-cap at 1 baby + 1 parent.                                                                                                                                                                                                                  |
| Application-level E2E layer (HKDF)                   | `DEFERRED` Phase 2 (ADR-004)                                                 | DTLS + fingerprint binding is the baseline.                                                                                                                                                                                                     |
| Key rotation                                         | `DEFERRED` Phase 2                                                           | Pairing key valid until manual re-pair.                                                                                                                                                                                                         |
| Offline LAN                                          | `DECIDED`, target **beta** (ADR-021)                                         | Topologies: Android↔Android (embedded listener) and Android↔PC / PC↔PC via local PC mode (server binary on `http://localhost`). Alpha uses internet signaling. HTTPS web ↔ phone listener is `REJECTED`; Chrome Local Network Access is `IDEA`. |
| LAN ↔ Internet automatic handoff                     | `IDEA`                                                                       | Switching is manual.                                                                                                                                                                                                                            |
| Remote pairing (invite code)                         | `DEFERRED` Phase 2                                                           | MVP requires a physical QR scan.                                                                                                                                                                                                                |
| i18n                                                 | `DECIDED` (ADR-021): about 20 AI-translated locales, en master               | See §5.2a. Adding locales is allowed through the catalogue and generator only.                                                                                                                                                                  |
| Distribution                                         | Play + GitHub APK at alpha; IzzyOnDroid at beta; **F-Droid main `DEFERRED`** | No promise of F-Droid main (prebuilt libwebrtc and JNA); say "via IzzyOnDroid".                                                                                                                                                                 |
| TURN, cargo-vet, SBOM/attest/cosign, CodeQL, Weblate | T1 (beta)                                                                    | Do not build before beta.                                                                                                                                                                                                                       |
| Style Dictionary, OSS-Fuzz, sccache                  | T2                                                                           | Not before 1.0.                                                                                                                                                                                                                                 |
| Video monitoring                                     | `IDEA` Phase 3+                                                              | No camera capture or video pipeline.                                                                                                                                                                                                            |
| On-device AI / sound categorization                  | `IDEA` Phase 3+                                                              | No TFLite / YAMNet / MFCC code.                                                                                                                                                                                                                 |
| Desktop installable client (Tauri/Electron)          | `IDEA`                                                                       | PWA + local PC mode is the desktop story.                                                                                                                                                                                                       |
| Audio sensitivity calibration / sleep log            | `DEFERRED` Phase 2                                                           | Out of MVP.                                                                                                                                                                                                                                     |
| White noise / lullaby playback                       | `REJECTED`                                                                   | Permanently out of scope.                                                                                                                                                                                                                       |
| Signaling rate limits beyond ADR-020 §6              | `IDEA`                                                                       | Design first in `docs/features/connectivity-pairing.md`.                                                                                                                                                                                        |
| Telemetry / crash reporting                          | not in docs                                                                  | Requires an ADR + opt-in UX.                                                                                                                                                                                                                    |

When in doubt, open the matching file in `docs/features/`, follow its `STATUS`, and ask instead of inventing.

## 10. Tips for agents

- **Read before you write.** Open the relevant ADR, feature doc and module first; matching the layer matters more than matching surface syntax.
- **Tasks print SKIPPED** without Docker/KVM; a SKIPPED is not a pass. Say so in your report.
- **Never invent dependency versions**; query the registry or lockfile.
- **Model routing (plan strong, implement cheap):** the session model does planning, architecture, ADR/role reviews and final verification. **Implementation is delegated** to the `implementer` sub-agent ([`.claude/agents/implementer.md`](.claude/agents/implementer.md), Sonnet); this rule is the user's standing request to spawn it. Hand over a self-contained brief (goal, files, layer rules, acceptance checks); the planner reviews the diff and runs lint/tests. Exceptions: trivial one-line edits and docs/ADR wording stay with the planner.
- **Cleanup pass (per task):** after implementation and green checks, before the final commit/PR, the planner runs [`refactor-scout`](.claude/agents/refactor-scout.md) in diff mode against `origin/main`; accepted findings go to the `implementer`, then re-run checks. Once per task; skip for docs-only, config-only or diffs under ~50 changed code lines.
- **Refactor sweep (weekly):** the [`refactor-sweep`](.claude/skills/refactor-sweep/SKILL.md) procedure: the scout proposes up to 5 behavior-preserving improvements, the planner triages, the `implementer` works each in its own worktree off `origin/main`, the planner verifies and opens one **draft PR** per finding (label `refactor-sweep`, max 3 open). Needs-an-ADR items are reported, not implemented. Sweeps never merge or push to `main`.
- **Cross-IDE etiquette:** tool-specific files complement, never replace, this file; mirror their intent here.

## 11. Maintenance of this file

Update it when the stack, repository layout, commands, ADR set, role process, guardrails or the §9 matrix change, and when a wave lands (drop the "(planned, wave N)" marker and the matching "current commands" row). Keep it under ~300 lines; move detail to `docs/contributing/*.md` and link it.
