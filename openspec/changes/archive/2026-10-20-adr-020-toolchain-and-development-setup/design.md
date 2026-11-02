# ADR-020: Toolchain and development setup for the Rust core and native clients

- Status: DECIDED (owner, 2026-10-20)
- Date: 2026-10-20
- Deciders: Project owner. Team review 2026-10-20: Technical Architect, Security/Privacy Architect, DevEx & Release Engineer (ad-hoc role), Product Owner, UX/Usability Expert, Critical Reviewer.
- Supersedes: the tool and process rows of [ADR-019](../2026-10-20-adr-019-language-repository-and-client-policy/design.md) §2 (signaling "after the alpha"), §4 (pnpm as task host) and §5 (rename of `apps/mobile`); the release component layout of [ADR-006](../2026-05-13-adr-006-release-and-changelog-automation/design.md); the QEMU build and image-tag details of [ADR-008](../2026-05-13-adr-008-container-image-build-and-distribution/design.md) where they conflict; the hybrid (Capacitor) part of [ADR-007](../2026-05-13-adr-007-browser-e2e-test-strategy/design.md); partly [ADR-001](../2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md), [ADR-002](../2026-04-15-adr-002-client-platform-and-repository-structure/design.md), [ADR-014](../2026-07-07-adr-014-open-source-license-agpl-and-trademark/design.md), [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md) and [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md) where their notes say so
- Related: [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md), [ADR-016](../2026-10-21-adr-016-test-strategy-local-virtual-physical/design.md), [ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md)

> **Migrated to OpenSpec (2026-10-22):** the built part of §7 (catalogue, generator, literal lint) is normative in [`openspec/specs/i18n`](../../../specs/i18n/spec.md); the rest of §7 (critical tags, markers, pseudo-RTL, tokens) goes into the prepared change `expand-locales`. §10 goes into the prepared change `add-android-release` (T0) and the backlog (T1); the image part of §6 and §9 is in [`openspec/specs/distribution`](../../../specs/distribution/spec.md). §1–§5, §8, §9, §11 and §12 as built in wave 1 are normative in [`openspec/specs/dev-toolchain`](../../../specs/dev-toolchain/spec.md); their unbuilt wave-2/3 parts go into the prepared wave changes.

## Context

Most of the tooling was chosen for a single SvelteKit app wrapped by Capacitor: Husky + Python pre-commit + Node commitlint, an npm-centred Release Please setup, a Node signaling server, a hand-written TypeScript protocol package and a pnpm root that runs most tasks. The product now is a sans-IO Rust core with a native Android client, a WASM web client and a self-hosted signaling server ([ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md), [ADR-019](../2026-10-20-adr-019-language-repository-and-client-policy/design.md)).

The owner asked for a review that ignored all existing ADRs: design the development setup as if the project started today, then compare it with the repository and decide what to keep, adjust, rework or remove. Five roles wrote independent proposals (tool status verified on the web, October 2026). A Critical Reviewer challenged the merged draft with 18 findings, and every role answered each finding addressed to it. No disagreement between roles remained. The team's records are in the review notes summarised below.

## Decision

The setup is tiered: **T0** must exist for the alpha, **T1** by beta, **T2** at 1.0 or later. A gate that is not listed for a tier is not built before it.

### 1. Repository and orchestration (T0)

- One polyglot monorepo with each ecosystem's own workspace: Cargo, Gradle, pnpm.
- **`just` is the only entry point**, with a fixed task vocabulary: `setup fmt lint test gen build e2e gate ci`. Recipes call the native tools; they never re-implement them. `just ci` locally gives the same verdict as CI.
- **Tool pins live in exactly one place each**:
  - `rust-toolchain.toml` (exact stable version, edition 2024).
  - A separate pinned nightly used only for fuzzing and `cargo public-api` (`fuzz/rust-toolchain.toml`).
  - `mise.toml` + `mise.lock` for every other tool (node, pnpm, java, just, lefthook, typos, ktfmt, …).
  - The Gradle version catalog and wrapper.
- The task vocabulary is fixed (`setup fmt lint test gen build e2e gate ci`); namespaced sub-recipes such as `fmt-check` and `build-android` are allowed.
- Tasks that need Docker, KVM or the Android SDK print `SKIPPED: <task> needs <x>` and exit 0. CI sets `GS_STRICT=1`, which turns every skip into a failure.
- `just test` builds **no Android ABI**: host Rust, wasm, Vitest and Android JVM tests against the host-built core library. Cross-compiled ABIs belong to `just build-android`, `just gate` and release builds.

### 2. Dev environment (T0)

- Inner loop on the host: mise + rustup. Host prerequisites: git, mise, rustup; plus Docker and KVM for the gate.
- **One digest-pinned build image** is the only source of release APKs and screenshot baselines.
- The Dev Container is generated from that image and never edited by hand. Nix was rejected (cost for agents, Android SDK packaging).

### 3. Rust (T0 unless noted)

- **Crates** (this ADR uses short names; the real crate names carry the `guardedsleep-` prefix, e.g. `guardedsleep-core`, `guardedsleep-relay`, except the existing tool crates `i18n-gen` and `uniffi-bindgen`, which keep their names):
  - `core`: sans-IO, `#![forbid(unsafe_code)]`, RustCrypto only, no C dependencies.
  - `ffi`: UniFFI façade, pinned exactly (`=0.32.x`), with the in-repo bindgen.
  - `wasm`: wasm-bindgen + tsify façade.
  - `relay`: sans-IO room/admission logic shared by the server and the Android LAN listener.
  - `server`: axum binary.
  - `sim`, `i18n-gen`.
  - Binding attributes never appear in `core`.
- **cargo-deny** is the supply-chain gate:
  - Advisories.
  - Licences: an AGPL-3.0-compatible allow-list.
  - Sources: crates.io only.
  - Bans: tokio, mio, getrandom and socket crates are forbidden in the `core` and `relay` graphs, which enforces the sans-IO layering mechanically.
- Two further supply-chain checks: a dependency-count guard on `core`, and a daily scheduled advisory scan. cargo-vet on the core's runtime closure is **T1**.
- **Tests:** nextest, proptest, insta, and golden vectors including **negative vectors** that run through every binding (Rust, Kotlin, TS). One negative vector covers a replayed first message against the relay admission challenge ([ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §1). The simulator stays.
- **Fuzzing:**
  - T0: two cargo-fuzz targets run nightly (signaling envelope decode, QR/invite parse). The pairing-proof verify gets proptest properties at T0.
  - T1: the full fuzz set plus ClusterFuzzLite.
- **API-surface snapshots** (`cargo public-api` on `ffi` and `wasm`, plus the generated `.d.ts`) are committed. Generated bindings are not; CI generates twice and compares hashes.
- `cargo check --target aarch64-apple-ios` for `core`, `relay` and `ffi` runs on every PR. A Swift-bindings generation smoke test runs weekly.
- Clippy denies `unwrap_used`, `expect_used`, `panic` and `indexing_slicing` in parser modules through workspace `[lints]`.

### 4. Android (T0 unless noted)

- **Build:**
  - Latest stable AGP/Kotlin/Compose, targetSdk ≥ 36, version catalog.
  - A `build-logic` included build with convention plugins; configuration cache and build cache on.
  - Modules `app`, `ui-contract`, `platform`, `core-bindings`. Only `platform` may depend on `core-bindings`, so UI code importing FFI types is a compile error.
  - Manual constructor DI (no Hilt).
- **Formatting and lint:** ktfmt as a pinned standalone jar; Android Lint with `warningsAsErrors`.
- **Release build:**
  - R8 on.
  - **versionCode derived from versionName** in `version.properties`, never edited by hand.
  - Gradle wrapper `distributionSha256Sum`; repository content filters (`google()` for AndroidX/`com.android`, everything else `mavenCentral()`, no JitPack).
  - Gradle `verification-metadata.xml` is **T1**.
- **WebRTC:** `io.github.webrtc-sdk:android` behind a Kotlin `MediaEngine` interface. Patch policy is CVE-based:
  - A critical or exploited CVE in a used path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) that is still unpatched by the vendor 30 days after the Chrome fix triggers an ADR: switch vendor, patch-build, or disable the feature.
  - Six months without a vendor release also opens that ADR.
  - Building libwebrtc from source is DEFERRED with F-Droid main ([ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md)).
- **Tests:**
  - Robolectric and JVM tests against the host-built core.
  - Roborazzi + `roborazzi-accessibility-check` on **11 goldens**: 5 critical screens (connected, alert, reconnecting, link lost, mic denied) × {en 100 %, de 200 % font on a small phone} + 1 light-mode monitor screen.
  - A baseline-free text-overflow check.
  - Compose accessibility checks and Maestro in the emulator gate. Baselines are recorded only in the pinned image; agents never hand-edit them.

### 5. Web client (T0)

- `apps/web` is rebuilt on **Vite 8 + Svelte 5 + vite-plugin-pwa**, TypeScript strict, on the WASM core. No SvelteKit; revisit at beta.
  - `apps/mobile` stays frozen as a reference and is deleted when `apps/web` reaches parity.
  - Route changes move focus to the screen heading and update `document.title`.
  - The dark theme applies before first paint.
- **pnpm 11** is pinned through mise, with these settings:
  - `allowBuilds` as the install-script allow-list.
  - `minimumReleaseAge: 4320` (minutes, = 3 days).
  - `blockExoticSubdeps: true`.
  - `--frozen-lockfile` in CI.
  - corepack is not used.
- **Lint, format and tests:**
  - Prettier with `prettier-plugin-svelte` for formatting.
  - `svelte-check --fail-on-warnings` for types and compiler accessibility warnings. A suppression needs an inline `svelte-ignore` with a reason. No ESLint and no Biome at T0.
  - Vitest for logic only.
  - Playwright with fake media devices, `@axe-core/playwright` and an overflow check.
  - A test that pairing material never reaches `localStorage`, `sessionStorage`, IndexedDB or the Cache API.
- **Browser security:** strict CSP and Trusted Types; no third-party origins.
- **Protocol types are generated from the Rust core** into the WASM package. `packages/protocol` is deleted.

### 6. Signaling server (T0 unless noted)

- **Implementation and image:**
  - Rust axum on the sans-IO `relay` crate, **now** rather than after the alpha. It replaces `apps/signaling-server`.
  - Distroless, non-root, read-only root filesystem.
  - Built on native amd64 and arm64 runners and merged into one manifest (no QEMU, no Zig).
- **Behaviour:**
  - Per-connection size and rate limits.
  - No IP logging by default.
  - Room IDs derived by HKDF, never anything reversible to the PSK.
- **Built-in STUN** answers Binding requests only: response no larger than the request, per-IP rate limit, UDP 3478 published beside the reverse proxy. Public STUN is opt-in only, with UI copy that names who sees the IP address.
- The binary can also **serve the static web UI**, which is the offline PC mode in [ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md).
- **T1:**
  - A hardened TURN profile (coturn ≥ 4.15 or eturnal) with HMAC credentials, TTL ≤ 10 min, issued only after the pairing proof.
  - Trivy image scanning.
  - A Caddy + signaling (+ TURN) compose bundle with a self-hosting guide.

### 7. i18n and design tokens (T0)

- **i18n catalogue and generator:**
  - One JSON catalogue: en is the master, values use the ICU plural/select subset with named placeholders.
  - The Rust generator writes Android XML with plurals and a typed web module, plus `.xcstrings` later.
  - The generator fails on placeholder, plural or missing-key errors.
- **Shipping rules for locales:**
  - A shipped locale must be complete for every key tagged `critical` (alert, link loss, connection), so there is never a mixed-language alarm.
  - Each key carries a `machineTranslated` / `reviewed` marker, which tracks native-speaker review. Review improves a locale; it does not gate shipping ([ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md)).
  - `androidResources.localeFilters` lists exactly the generated locales.
  - A pseudo-locale and an RTL pseudo-locale exist, and a lint rejects hard-coded UI literals.
  - Weblate is T1.
- **Design tokens:** a DTCG-shaped `tokens.json`, generated for Compose and CSS by the same Rust generator.
  - A contrast test covers the dark, light and dim baby-unit themes (WCAG AA: 4.5:1 for text, 3:1 for status glyphs).
  - Every status token has an icon id and a message key, because status is never shown by colour alone.
  - Style Dictionary is T2.

### 8. Repository hygiene (T0)

- **Hooks:** lefthook replaces Husky, Python pre-commit and the commitlint hook. Pre-commit runs formatting and typos on staged files in under 5 s. Hooks are a convenience; CI re-runs everything.
- **Commit convention:** Conventional Commits are checked on the **PR title only**, in CI. With squash merges only the title reaches `main`.
- **Formatting:** `just fmt` / `just fmt-check` call rustfmt, ktfmt, Prettier and taplo directly. treefmt was rejected while there are fewer than six formatters.
- **Other checks:**
  - typos for the repository; CSpell with native dictionaries for non-English catalogues.
  - REUSE (`reuse lint`) for licence headers.
  - actionlint and zizmor for workflows.

### 9. CI, dependencies and branch protection (T0 unless noted)

- **GitHub Actions hardening:**
  - Every action pinned by full commit SHA.
  - `permissions: {}` at workflow level, granted per job.
  - `persist-credentials: false`; no caches written from PR workflows.
  - Release work runs in a protected `release` environment.
- **Job layout:**
  - One `ci-ok` aggregator job is the only required status check.
  - Path filters keep docs-only PRs fast.
  - Caching: rust-cache and setup-gradle; sccache is T2.
- **Emulator gate:**
  - It runs on PRs touching Android or core, nightly, and is required on release builds.
  - A test that flakes twice in seven days is quarantined with an issue; jobs are never retried as a whole.
- **Renovate ≥ 44.3.1:**
  - Runs weekly, one group per ecosystem, majors monthly.
  - `minimumReleaseAge` is 3 days, 7 days for libwebrtc and Gradle plugins.
  - UniFFI and wasm-bindgen get their own PRs, never grouped.
  - Automerge only for dev-only tools and Action digests; never for runtime or crypto dependencies.
- **Branch protection on `main`:** the `ci-ok` check plus linear history and squash merges. **No required reviews, no CODEOWNERS gate, no signed-commit requirement, and no separate agent identity**: the owner is the only maintainer and admin, and squash merges are signed by GitHub.

### 10. Releases and signing

- **T0:**
  - A manual `android-v*` tag with the derived versionCode.
  - The alpha signing key lives in the protected environment.
  - The key for directly distributed APKs is **generated offline now**, used from beta, and never stored in CI, because it cannot be rotated for existing installs. A v3 rotation-lineage plan is written down.
- **T1:**
  - Release Please with components `app` (one product version for Android and web), `server` and `core`.
  - The fastlane metadata tree serves Play and F-Droid clients.
  - CycloneDX SBOMs, `actions/attest` provenance, cosign-signed images.
  - A build-twice reproducibility check (apksigcopier/diffoscope).

### 11. Security and privacy tooling

- **T0:**
  - GitHub push protection plus Betterleaks with validation off and custom rules for PSK, invite and pairing-URI shapes.
  - A **log-canary test**: simulation and gate runs pair with a known PSK, and CI fails if it appears in any log in any encoding.
  - `secrecy` and `zeroize` for secret types.
  - Release-APK assertions: no tracker classes, expected permission set, backup exclusion, `debuggable=false`.
  - Size budgets for the core `.so`, the WASM bundle and the APK.
  - `SECURITY.md`; the AGPL §13 source link on server and web.
- **T1:** CodeQL, a threat-model document, OpenSSF Scorecard. MobSF is used ad hoc only, never as a gate.

### 12. Cost control (T0)

- Tooling maintenance has a **ceiling of 12 hours per month**; the team estimate for T0 is 7–13 h.
- Every gate has an owner, a failure runbook and a last-catch date in `docs/ci-gates.md`.
- A gate with no catch in six months is removed or demoted to nightly. When the ceiling is exceeded, the most expensive such gate goes first.

## Options Considered

The full option comparison per area is summarised here; rejected options keep their reason.

| Area               | Chosen                          | Discarded (reason)                                                                                                                                                                                                           |
| ------------------ | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Repository         | Monorepo, native workspaces     | Bazel/Buck2 (hostile to agents and F-Droid recipes); Nx/moon (second build-definition layer); multi-repo (protocol changes become cross-repo releases)                                                                       |
| Task runner        | `just`                          | mise tasks (task runner still changes monthly, caching experimental); make (portability); cargo xtask only (forces web/Android through cargo)                                                                                |
| Dev environment    | mise + pinned image             | Dev Container as primary (heavy, KVM passthrough fragile); Nix (agent error rate, Android SDK packaging); Docker-only (slow inner loop)                                                                                      |
| FFI                | UniFFI                          | Hand-written JNI/C ABI (two unsafe boundaries); Diplomat, swift-bridge (small user base); UniFFI JS backend (third-party, unstable flag)                                                                                     |
| Web framework      | Vite + Svelte 5                 | SvelteKit 3 (GA three weeks before this decision, agents write 1.x/2.x patterns); Leptos/Dioxus (browser APIs through `web-sys`, thin a11y ecosystem); Compose for Web (canvas rendering, Beta); React (heavier, no benefit) |
| Web lint           | Prettier + svelte-check         | Biome (full Svelte support experimental); ESLint (a11y rules already come from the Svelte compiler)                                                                                                                          |
| Signaling          | Rust axum                       | Node + ws (second validator, npm surface on the internet-facing service); Go (third language)                                                                                                                                |
| STUN               | Built into the server           | Public STUN by default (third party sees IPs); coturn as STUN (C parser with 2026 CVEs)                                                                                                                                      |
| WebRTC (Android)   | webrtc-sdk M144                 | GetStream (documented on m125); build from source (deferred with F-Droid main); webrtc-rs/str0m (rebuild audio stack)                                                                                                        |
| Screenshots        | Roborazzi, Playwright           | Paparazzi 2 and Compose Preview Screenshot Testing (alpha); full theme × locale × font matrix (hundreds of baselines get rubber-stamped)                                                                                     |
| i18n               | JSON + ICU + Rust generator     | Fluent (no mobile runtime); Android XML as source (format leaks into web/iOS); gettext (msgid keys)                                                                                                                          |
| Tokens             | DTCG JSON via Rust generator    | Style Dictionary now (Node build dependency; kept for iOS at T2)                                                                                                                                                             |
| Hooks              | lefthook                        | pre-commit (Python), prek (hook catalogue not needed), Husky (Node before hooks work), commit-msg hook (redundant with squash)                                                                                               |
| Formatting         | `just fmt`                      | treefmt (one more tool for four formatters)                                                                                                                                                                                  |
| Releases           | Manual tag, then Release Please | cocogitto, release-plz (Rust registry only), semantic-release (no review gate), knope (pre-1.0)                                                                                                                              |
| Dependency updates | Renovate                        | Dependabot (weak cross-ecosystem grouping, no mise/toolchain files)                                                                                                                                                          |
| Supply chain       | cargo-deny now, cargo-vet T1    | cargo-vet at T0 (a decision per update); cargo-crev (low adoption)                                                                                                                                                           |
| Branch protection  | `ci-ok` + linear history        | Required review and CODEOWNERS (owner is sole admin and would bypass); signed commits (web-flow signs squash merges); separate agent bot identity (owner is sole admin; no required reviews to satisfy)                      |

## Consequences

### Positive

- Every task has one command, and CI runs exactly that command.
- Safety logic, protocol types and validation exist once, in Rust, and the layering is checked by cargo-deny and Gradle modules rather than by review.
- Node leaves the server path and the git hooks; JavaScript remains only where the browser requires it.
- The alpha carries the guardrails that are expensive to add later (pins, signing key, REUSE, versionCode, Renovate), and defers the rest with a written tier.

### Negative

- Four languages remain (Rust, Kotlin, TypeScript, later Swift), and the setup still needs Docker and KVM for the gate.
- The web client is rebuilt rather than migrated in place.
- Roughly 10 hours per month of tooling maintenance for the owner.

### Risks

- UniFFI is pre-1.0 and breaks in minor releases. Mitigation: exact pin, coarse API, API-surface snapshot, separate Renovate PR.
- The webrtc-sdk build lags Chrome. Mitigation: CVE-based SLA and the `MediaEngine` seam.
- Version-specific settings go stale (for example, pnpm 11 renamed `onlyBuiltDependencies`). Every version-specific setting in configuration carries a link to its documentation.

## Follow-ups

- `STATUS: DECIDED` Implementation in three waves (guardrails and cleanup; structure; web and quality gates), each followed by a review from the affected roles.
- `STATUS: DECIDED` Wave 1 also disables `.github/workflows/release-please.yml` (npm components are legacy) until Release Please returns at T1.
- `STATUS: DECIDED` AGENTS.md, `.cursor/rules` and `.claude/agents` updated to this ADR.
- `STATUS: DECIDED` Status notes on ADR-002, ADR-006, ADR-007, ADR-008, ADR-015, ADR-016, ADR-019 pointing here (ADR-012 points to ADR-021) for superseded points.
