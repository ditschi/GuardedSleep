# AGENTS.md — GuardedSleep

> **What is this file?** A tool-agnostic briefing for AI coding agents (Cursor, Claude Code, OpenAI Codex CLI, Cline, Aider, Continue, Copilot Workspace, …) working in this repository. It is intentionally short on prose and high on **rules, conventions, and links** so that any agent can scan it and act correctly. Human-oriented onboarding lives in [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).
>
> **Source of truth precedence** (highest first):
>
> 1. [Architecture Decision Records](docs/decisions/README.md) (ADRs) — `DECIDED` items are binding.
> 2. [`docs/concept/vision.md`](docs/concept/vision.md), [`docs/features/`](docs/features/README.md), [`docs/technical/`](docs/technical/sw-architecture.md).
> 3. This file.
> 4. Cursor-specific rules in [`.cursor/rules/`](.cursor/rules/) (apply only inside Cursor; the same intent is mirrored here for other agents).
>
> **If this file disagrees with an ADR, the ADR wins. File a `chore(docs): sync AGENTS.md` change.**

---

## 1. Project snapshot

- **Product:** GuardedSleep – Privacy Baby Monitor. Open-source, privacy-first, **fully local-capable** (no cloud dependency for media).
- **Repository / engineering name:** GuardedSleep. **npm scope `@guardedsleep/*`**, **Android app id `org.guardedsleep.app`**, **Docker images `guardedsleep-*`**.
- **Phase:** **Concept Phase with scaffolded code.** Documentation and developer tooling are complete; an MVP feature implementation has **not** started. A **Proof-of-Concept gate** ([ADR-001](docs/decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md)) must pass before broad MVP build-out.
- **Primary platform (MVP):** Android via Capacitor. iOS, desktop, and video are **explicitly deferred or ideas only** (see §7).
- **License:** TBD before any first-party code release.

## 2. Tech stack (DECIDED, see [ADR-002](docs/decisions/ADR-002-client-platform-and-repository-structure.md))

| Area             | Choice                                              | Notes                                                                                                                                                                            |
| ---------------- | --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language         | **TypeScript everywhere**                           | No JavaScript files in new code.                                                                                                                                                 |
| Repo layout      | **pnpm workspaces monorepo**                        | See `pnpm-workspace.yaml`.                                                                                                                                                       |
| Web framework    | **Svelte 5 + SvelteKit (SPA / static adapter)**     | SSR features are not used (Capacitor target).                                                                                                                                    |
| UI               | **Tailwind CSS + shadcn-svelte (planned)**          | Currently Tailwind v4 via `@tailwindcss/vite`; shadcn-svelte components are added on demand.                                                                                     |
| Mobile shell     | **Capacitor 7 (Android)**                           | iOS is Phase 2; do **not** add iOS code without an ADR update.                                                                                                                   |
| Signaling server | **Node.js 22 + `ws` + TypeScript**                  | WSS in production, plain WS only for local dev.                                                                                                                                  |
| Build tool       | **Vite (via SvelteKit) / `tsc` for libraries**      | `packages/protocol` emits to `dist/`.                                                                                                                                            |
| Package manager  | **pnpm 9.15.4** (pinned via `packageManager`)       | Use `corepack` to activate.                                                                                                                                                      |
| Lint / format    | **Prettier + `prettier-plugin-svelte` + `typos`**   | No ESLint yet; Prettier + `svelte-check` cover style. Adding ESLint requires an ADR or `docs` note.                                                                              |
| Tests            | **Vitest** + **Playwright** (`apps/e2e`, Chromium)  | Vitest in every workspace; Playwright for two-role WebRTC smoke ([ADR-007](docs/decisions/ADR-007-browser-e2e-test-strategy.md)). `@testing-library/svelte` for component tests. |
| Crypto / media   | **WebRTC DTLS-SRTP + DataChannel** (browser-native) | No custom crypto for transport. PSK / fingerprint binding planned per [ADR-004](docs/decisions/ADR-004-encryption-transport-and-key-storage.md).                                 |

## 3. Repository map

```
.
├── apps/
│   ├── mobile/              # SvelteKit + Capacitor (Android tracked under apps/mobile/android)
│   ├── e2e/                 # Playwright browser E2E (baby + parent, local dev stack)
│   └── signaling-server/    # Node + ws WebSocket relay
├── packages/
│   └── protocol/            # Shared TS types & helpers (DataChannel msgs, heartbeat, parse)
├── docker/
│   ├── signaling-server/    # Multi-stage image + Compose (local build + GHCR + Traefik example)
│   ├── mobile-web/          # nginx serving the static PWA build
│   ├── stack/               # Combined PWA + signaling local stack (TLS via self-signed)
│   └── android-build/       # CI / host-only Gradle image (alt. to dev image)
├── .devcontainer/           # Dev Container (Node 22 + JDK 21 + Android SDK 35)
├── .github/workflows/       # CI: pre-commit, test+build, browser E2E, optional docker-signaling
├── .husky/                  # Git hooks (pre-commit, commit-msg)
├── .cursor/rules/           # Cursor-specific *.mdc rules (also captured here for other agents); includes `english-source-locale.mdc` (English source + i18n)
├── docs/
│   ├── concept/             # Vision, brainstorm, competitive analysis
│   ├── features/            # Feature specs — each file uses STATUS tags
│   ├── decisions/           # ADRs (authoritative MVP decisions)
│   ├── technical/           # Architecture, UI design, analysis
│   ├── roles/               # Role definitions for multi-agent reviews
│   ├── reviews/             # Rolling review checklists
│   ├── contributing/        # Dev setup, Docker, releases
│   └── legal/               # Privacy policy
├── scripts/                 # devcontainer-run.sh, dev-shell.sh
├── CHANGELOG.md             # Keep a Changelog, manual until Release Please
├── CONTRIBUTING.md
└── README.md
```

**Per-package agent context:** for now, this single root `AGENTS.md` applies to the whole monorepo. If a package gains package-specific rules (e.g. `apps/mobile/AGENTS.md`), it inherits from this file and **only documents deltas**.

| Spellcheck locale message JSON only | `pnpm run lint:cspell-i18n` (same as the `cspell-i18n-messages` pre-commit hook) |

## 4. Setup, build, test (always copy-paste from here)

> Prefer the Dev Container; it pins Node 22, pnpm 9.15.4, JDK 21, and the Android SDK so an agent never has to guess versions.

```bash
# One-time (host or Dev Container):
corepack enable && corepack prepare pnpm@9.15.4 --activate

# Install all workspaces:
pnpm install --frozen-lockfile

# Wire Husky (only after install on a fresh clone):
pnpm run prepare   # do NOT run `pre-commit install` (conflicts with Husky)
```

> **Rootless Docker on Linux:** if `docker info` shows `rootless: true`, use `./scripts/devcontainer-run.sh …` (auto `DEV_UID`/`DEV_GID`) or copy `.devcontainer/.env.example` → `.devcontainer/.env` with `DEV_UID=0` / `DEV_GID=0`. Raw `docker compose -f .devcontainer/…` must also pass `--project-directory .devcontainer` so that `.env` is loaded; otherwise `node` (uid 1000) hits `EACCES` on `pnpm install`. Details: [docs/contributing/development.md](docs/contributing/development.md#rootless-docker--eacces-on-workspace_tmp_).

| Task                            | Command                                                                                                                                                                                                  |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Run all CI-equivalent checks    | `pnpm run lint` (= `pre-commit run --all-files`; includes **typos** for English + **cspell** for `de`/`fr`/`es` message JSON)                                                                            |
| Spellcheck locale message JSON  | `pnpm run lint:cspell-i18n` (same checks as the `cspell-i18n-messages` pre-commit hook)                                                                                                                  |
| Format the repo                 | `pnpm run format`                                                                                                                                                                                        |
| Format check only               | `pnpm run format:check`                                                                                                                                                                                  |
| Type check Svelte               | `pnpm --filter @guardedsleep/mobile check`                                                                                                                                                               |
| Unit + integration tests        | `pnpm test` (builds `@guardedsleep/protocol` first, then runs Vitest in all)                                                                                                                             |
| Build all packages              | `pnpm run build`                                                                                                                                                                                         |
| Build mobile (static SvelteKit) | `pnpm --filter @guardedsleep/mobile build`                                                                                                                                                               |
| Build signaling-server          | `pnpm --filter @guardedsleep/signaling-server build`                                                                                                                                                     |
| Signaling smoke (WS relay)      | `pnpm --filter @guardedsleep/signaling-server smoke`                                                                                                                                                     |
| Local dev (PWA + signaling)     | `pnpm run dev:local` (Vite on `5173`, signaling on `8787`)                                                                                                                                               |
| Browser E2E (Chromium)          | `pnpm run e2e` (Playwright; starts `dev:local` via `webServer` — see `apps/e2e/README.md`)                                                                                                               |
| Dependency audit                | `pnpm run audit` (`--audit-level=high`)                                                                                                                                                                  |
| Capacitor sync (Android)        | `pnpm --filter @guardedsleep/mobile run cap:sync`                                                                                                                                                        |
| Open Android Studio project     | `pnpm --filter @guardedsleep/mobile run cap:open:android`                                                                                                                                                |
| Android debug APK (CI)          | Workflow [`.github/workflows/android.yml`](.github/workflows/android.yml); artifact on PR / `main` / `mobile-v*` tags — [android-apk-build.md](docs/contributing/android-apk-build.md#ci-github-actions) |
| One-shot full stack (Docker)    | `docker compose -f docker/stack/compose.yaml up --build`                                                                                                                                                 |
| Validate branch commits         | `pnpm run release:lint` (Release Please pre-check)                                                                                                                                                       |
| Preview Release Please output   | `GH_TOKEN=… GITHUB_REPOSITORY=…/… pnpm run release:preview` (maintainer)                                                                                                                                 |

**Before every commit** the Husky `pre-commit` hook runs `pre-commit run` (trailing whitespace, EOF, YAML/JSON, typos, CSpell on locale message JSON, Prettier) and `commit-msg` runs `commitlint`. Agents that bypass hooks must re-run `pnpm run lint` manually before declaring "done".

## 5. Coding conventions

### 5.1 Source layout & layers (DECIDED, see [`sw-architecture.md`](docs/technical/sw-architecture.md))

The mobile app uses a **strict layered architecture**. Agents adding code MUST keep imports flowing **only downward**:

```
UI (routes/, components/)
  ↓
State (Svelte stores)
  ↓
Platform (Capacitor ↔ browser abstraction in lib/platform/*)
  ↓
Core  (pure TypeScript, framework-free) — lib/core/{webrtc,audio,crypto,protocol,discovery}
```

Hard rules:

- **`lib/core/` is framework-free.** No `import` of Svelte, SvelteKit, Capacitor, DOM-only APIs that have no test shim, or any UI library. Core code must run under plain Vitest (jsdom only when truly DOM-dependent).
- **`lib/platform/` is the only place that touches Capacitor plugins.** It exposes a stable interface so the same UI/core works in browser (PWA fallback) and on Android.
- **Signaling protocol types live in `@guardedsleep/protocol`** and are imported by both mobile and signaling-server. Never duplicate the message schema; extend the shared package.
- New cross-cutting modules → propose in `docs/technical/sw-architecture.md` or open an ADR first.

### 5.2 Style

- Editor settings come from `.editorconfig` (LF, UTF-8, 2-space indent).
- Prettier config: `printWidth: 100`, `proseWrap: preserve`. **Do not** add ad-hoc Prettier overrides per file.
- Filenames: `kebab-case.ts` / `kebab-case.svelte` for modules, `PascalCase.svelte` for visible components.
- Imports: prefer **named exports**; only default-export Svelte components and SvelteKit page/layout files.
- Comments: explain **why**, not **what**. No narration comments (`// import X`, `// loop`). See repo-wide rule "no obvious comments".

### 5.2a Human language (English source, i18n for locales)

- **Repository default language: English.** All material under `docs/` (concept, features, ADRs, reviews, contributing), code comments, identifiers, and **hard-coded UI strings in source** must be written in English so reviews, CI (`typos`), and cross-team work stay consistent.
- **Localized end-user copy** lives **only** in locale bundles (today: `apps/mobile/src/lib/i18n/messages/*.json`). Add or change strings there; `en.json` is the parity master (`MASTER_LOCALE`). Do not paste German/French/etc. prose into `.svelte`/`.ts` for production UI—use `t()` / `tWithEnFallback()` with keys. **Spellcheck:** `typos` (English) covers the repo including `en.json`; **CSpell** with native dictionaries covers `de.json` / `fr.json` / `es.json` (`pnpm run lint:cspell-i18n`, configs `cspell-i18n-*.json`).
- **Quoted non-English** is allowed sparingly in docs when it is the product artifact being analyzed (e.g. a persona quote); prefer an English gloss in the same sentence.

### 5.3 TypeScript

- Strict mode is assumed across packages. Do not silence with `// @ts-ignore`; use `// @ts-expect-error <reason>` if absolutely required.
- Public functions/classes get JSDoc only when intent is non-obvious — the type system carries the rest.
- No `any` in production code; in tests prefer minimal local mock types.

### 5.4 Tests

- Place tests next to the file (`foo.ts` → `foo.test.ts`) for unit tests; integration tests live under `src/test/` (mobile) or `src/*.test.ts` (server) per the existing pattern.
- Every new pure-TS module in `lib/core/` **must** ship a Vitest file. UI components should at minimum have a render-smoke test.
- Use `@testing-library/svelte` for components; do not introduce a second component test framework.

## 6. Documentation & decision conventions

These rules apply **always**, not only when editing docs — agents producing features or PRs must follow them.

### 6.1 STATUS tags (mandatory in `docs/`)

Every requirement, feature item, proposal, or design note carries one of:

- `STATUS: IDEA` — captured but not yet evaluated.
- `STATUS: UNDER DISCUSSION` — actively analyzed.
- `STATUS: DECIDED` — consensus reached, rationale documented.
- `STATUS: DEFERRED` — postponed (include reason + target phase).
- `STATUS: REJECTED` — explicitly rejected (include reason).

User-suggested ideas → file under the matching feature doc as `STATUS: IDEA`. Do **not** assign a phase unless the user explicitly decides one.

### 6.2 ADR workflow

Any change that touches **architecture, scope, security model, prior decisions, or core concepts** requires an ADR in [`docs/decisions/`](docs/decisions/README.md) using the template at the top of that README. Naming: `ADR-NNN-short-title.md`. Status transitions: `PROPOSED → DECIDED → SUPERSEDED`. **Never delete rejected options** — they go to "Discarded alternatives" with the reason.

### 6.3 Presenting options (mirrors [`.cursor/rules/decision-workflow.mdc`](.cursor/rules/decision-workflow.mdc))

When suggesting multiple paths to the user, an agent MUST present them as a checklist with **Pros / Cons / Recommendation**:

```markdown
### Options for [Topic]

- [ ] **Option A: [Name]** — Pros: … Cons: …
- [ ] **Option B: [Name]** — Pros: … Cons: …

**Recommendation:** Option [X] because [reasoning].
```

Skip the checklist only if there is clearly one reasonable approach. Trivial implementation details never need a vote.

### 6.4 Role coordination (mirrors [`.cursor/rules/role-coordination.mdc`](.cursor/rules/role-coordination.mdc))

For non-trivial work, act as **Coordinator** and consult the project roles defined in [`docs/roles/`](docs/roles/README.md):

| Task                              | Minimum roles                            |
| --------------------------------- | ---------------------------------------- |
| New feature design                | Product Owner + Technical Architect      |
| + security implications           | + Security/Privacy Architect             |
| + user-facing changes             | + UX/Usability Expert                    |
| Any ADR being finalized           | + Critical Reviewer                      |
| Architecture / technical decision | Technical Architect + Security Architect |
| MVP scoping / prioritization      | Product Owner + Critical Reviewer        |

**Critical Review Gate:** if any role's output changes architecture, scope, security model, a prior decision, or a core concept, the **Critical Reviewer is mandatory** and the originating role must respond to each `CRITICAL` / `IMPORTANT` finding (accept, refute with evidence, or propose an alternative). Unresolved disagreements are presented to the user as a decision checklist (§6.3). The user has final authority.

**Agents without sub-agent support** (e.g. plain CLI tools, single-shot completions): adopt the same checklist mentally and **list which role perspectives you applied** in the PR description. Don't skip; just narrate.

## 7. Privacy and security guardrails (NON-NEGOTIABLE)

These are absolute lines. Code that crosses them must be rejected or rolled back, even if it compiles and tests pass.

1. **No third-party services in the media or alert path.** Audio, video, and per-event notifications never traverse cloud servers we don't operate. Signaling and STUN may transit our infrastructure but carry **only connection metadata**, never media.
2. **No cleartext PSKs or sensitive pairing material on disk.** Use Capacitor Secure Storage (Android Keystore / TEE). Plain `localStorage` is only for non-secret settings (UI prefs, signaling URL).
3. **No custom transport crypto.** Stick to WebRTC DTLS-SRTP + DTLS (DataChannel) plus the planned application-level HKDF layer in Phase 2. No homegrown ciphers, no XOR "obfuscation".
4. **Validate all signaling payloads** before use (the WebRTC client already does — keep it that way). The server treats every message as untrusted.
5. **No telemetry, crash reporting, or analytics** without an ADR and an opt-in UX. Default = silent, local only.
6. **No raw PII in logs** (no IPs, device names, fingerprints in production builds). Dev logs may be more verbose behind a debug flag.
7. **No new `*.key`, `*.pem`, `.env`, `credentials.json`** committed. `.gitignore` already blocks these — do not add exceptions.
8. **Adding dependencies:** justify the addition in the PR description; prefer well-maintained, low-transitive-dependency packages. CI gate is `pnpm audit --audit-level=high`. Do not silence audit findings; bump or override (`pnpm.overrides` is the documented escape hatch).

## 8. Git, commits, pull requests, and releases

- **Conventional Commits** are enforced by commitlint locally (Husky `commit-msg`) and in CI on both **PR title** (squash subject) and **all PR commits** (post-rebase safety). Allowed types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `security`, `style`, `test` (see [`commitlint.config.mjs`](commitlint.config.mjs)).
- **PR titles** must follow Conventional Commits because the default merge is **squash** and the squashed commit subject is derived from the PR title.
- **Breaking changes:** use `<type>!:` in the subject **and** add a `BREAKING CHANGE: <description>` footer at the bottom of the PR description so it survives the squash. Footer must be exactly that string (case + spacing) — Release Please ignores variants.
- **Release automation:** [Release Please](docs/contributing/release-and-changelog.md) is **wired** ([ADR-006](docs/decisions/ADR-006-release-and-changelog-automation.md)). It opens a release PR that bumps versions and updates `CHANGELOG.md` from Conventional Commits. Agents do not edit released-version sections of `CHANGELOG.md` by hand; only `## [Unreleased]` may receive manual bullets until the first release PR rolls them up.
- **Versioning posture:** `@guardedsleep/mobile`'s version **is** the user-facing product version. `@guardedsleep/protocol` and `@guardedsleep/signaling-server` are versioned **independently**. When `protocol` bumps, the `node-workspace` Release Please plugin **auto-patches** both consumers in the same release PR (with a `Dependencies` changelog entry) — do not bump dependents manually. Project milestones (MVP, PoC pass, Phase 2 entry, …) receive **manual meta-tags** like `mvp-v0.1.0` whose annotated message lists the exact `protocol-v…` / `signaling-server-v…` / `mobile-v…` triplet; see [release-and-changelog.md §16–§19](docs/contributing/release-and-changelog.md). Tag format is always `<component>-v<semver>` with a hyphen (Release Please default).
- **Container images** ([ADR-008](docs/decisions/ADR-008-container-image-build-and-distribution.md)): two images on GHCR — `ghcr.io/<owner>/guardedsleep-signaling` and `…/guardedsleep-web` — built for `linux/amd64` + `linux/arm64`. Tag scheme: `:latest` (= latest release tag), `:vX.Y.Z` / `:vX.Y` / `:vX` (post-1.0), `:dev` (= every `main` push, alias `:edge`), `:sha-<short>`. Workflows: [`docker-signaling.yml`](.github/workflows/docker-signaling.yml), [`docker-web.yml`](.github/workflows/docker-web.yml). Consumer drop-in stack: [`docker/stack/compose.ghcr.yaml`](docker/stack/compose.ghcr.yaml) + [`.env.ghcr.example`](docker/stack/.env.ghcr.example). Until the first Release Please release exists, `:latest` is not produced — consumers use `:dev`.
- **Commit → changelog mapping** (full table in [release-and-changelog.md §2](docs/contributing/release-and-changelog.md)):
  - `feat:` → `Added`, minor bump
  - `fix:` → `Fixed`, patch bump
  - `perf:` → `Changed`, patch bump
  - `revert:` → `Removed`, patch bump
  - `security:` → `Security`, patch bump
  - `feat!:` / `BREAKING CHANGE:` → ⚠ banner + minor (pre-1.0) / major (post-1.0) bump
  - all other types → hidden from changelog
- **Branching:** target `main`. Long-running features may use `develop`. Do not push to `main` directly except for Release Please's `chore(release)` PR merges.
- **PR checklist** (enforced in [`.github/pull_request_template.md`](.github/pull_request_template.md)):
  - Conventional PR title.
  - Breaking change footer if applicable.
  - `pnpm run lint` and `pnpm run release:lint` pass locally.
  - Docs changes respect `STATUS` tags and ADR conventions.
  - Architecture/scope/security-model change → ADR exists or is part of the PR.
- **Never** force-push to `main` / `master` / `develop`. Never use `--no-verify` to bypass hooks. Never `git config` changes inside an agent run.

## 9. Not-yet-implemented features — agent guardrails

The repo's documentation already commits to a feature roadmap. Agents must respect **what is not yet implemented** and must not speculate code into being. Treat the following as **off-limits unless the task explicitly references the matching ADR / feature file and updates `STATUS`**:

| Feature                                     | Doc status                                                                                         | Agent rule                                                                    |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| In-app App-Lock (PIN / biometrics)          | `DEFERRED` to Phase 2 ([ADR-001](docs/decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md)) | MVP relies on OS screen lock. Do not add biometric plugins or PIN flows.      |
| iOS build                                   | `DEFERRED` Phase 2                                                                                 | Do not generate Xcode/iOS code. Capacitor `ios` folder is ignored on purpose. |
| Talk-back (parent → baby audio)             | `DEFERRED` Phase 2                                                                                 | Track stays muted in MVP; no UI for talk-back.                                |
| Multi-parent (>1 parent device)             | `DEFERRED` Phase 2 (max 4)                                                                         | Hard-cap MVP at 1 baby + 1 parent. Same QR add-flow planned for Phase 2.      |
| Application-level E2E layer (Layer 3, HKDF) | `DEFERRED` Phase 2 ([ADR-004](docs/decisions/ADR-004-encryption-transport-and-key-storage.md))     | DTLS + fingerprint binding is the MVP baseline.                               |
| Key rotation                                | `DEFERRED` Phase 2                                                                                 | Pairing key valid until manual re-pair.                                       |
| LAN ↔ Internet automatic handoff            | `IDEA`                                                                                             | Both modes exist in MVP; switching is manual.                                 |
| Remote pairing (invite code)                | `DEFERRED` Phase 2                                                                                 | MVP requires physical QR scan; invite link UX is meeting-style only.          |
| i18n (multi-language)                       | `DEFERRED` Phase 2                                                                                 | English only in Phase 1. The `lib/i18n/` parity test is groundwork.           |
| Video monitoring                            | `IDEA` Phase 3+                                                                                    | Do not add camera capture or video pipeline.                                  |
| On-device AI / sound categorization         | `IDEA` Phase 3+                                                                                    | No TFLite / YAMNet / MFCC code yet.                                           |
| Desktop installable client (Tauri/Electron) | `IDEA`                                                                                             | PWA + browser fallback is the current desktop story.                          |
| Audio sensitivity calibration / sleep log   | `DEFERRED` Phase 2                                                                                 | Out of MVP.                                                                   |
| White noise / lullaby playback              | `REJECTED`                                                                                         | Permanently out of scope.                                                     |
| Signaling rate limits                       | `IDEA`                                                                                             | Design first (`docs/features/connectivity-pairing.md`) before code.           |
| Telemetry / crash reporting                 | not in docs                                                                                        | Requires explicit ADR + opt-in UX. Default = none.                            |

**When in doubt:** open the matching feature file in `docs/features/`, follow the `STATUS` tag, and surface a question instead of inventing implementation.

## 10. Tips for agents working in this repo

- **Read before you write.** Open the relevant ADR, feature doc, and existing module before generating new code. The codebase deliberately separates concerns; matching the layer is more important than matching surface syntax.
- **Prefer extending the shared protocol package** over inlining types. Drift between mobile and signaling-server is the most likely source of nasty bugs.
- **Run the smoke test** (`pnpm --filter @guardedsleep/signaling-server smoke`) when you touch signaling logic; it builds and exercises the WS relay end-to-end.
- **Capacitor sync is destructive** to `apps/mobile/android/app/src/main/assets/public`. Do not commit transient `cap sync` artifacts; CI / Docker workflows recreate them.
- **For multi-step tasks** maintain a short to-do list; do not declare success until lint, type-check, and tests pass.
- **Never invent versions** for new dependencies — query the registry / lockfile, or add via `pnpm add` so the lockfile updates correctly.
- **Model routing (plan strong, implement cheap):** the session model (strong, e.g. Opus) does planning, architecture, ADR/role reviews, and final verification. **Implementation work is delegated** to a sub-agent on a smaller, token-efficient model — in Claude Code the `implementer` agent ([`.claude/agents/implementer.md`](.claude/agents/implementer.md), Sonnet). This repo rule counts as the user's standing request to spawn sub-agents for implementation. Hand over a self-contained brief (goal, files, layer rules from §5, acceptance checks); the planner reviews the diff and runs lint/tests before declaring done. Exceptions: trivial one-line edits and docs/ADR wording stay with the planner. Tools without sub-agents: plan first, then switch to a cheaper model manually for the implementation step.
- **Cleanup pass (per task):** once a task's implementation is complete and checks pass, and **before the final commit / PR**, the planner runs [`refactor-scout`](.claude/agents/refactor-scout.md) in diff mode against `origin/main`. Accepted findings go to the `implementer` in the same branch, then lint/tests are re-run. Run it once per task, not per commit; skip it for docs-only, config-only, or diffs under ~50 changed code lines. Pre-existing issues outside the diff are left for the weekly sweep.
- **Refactor sweep (weekly):** a scheduled run executes the [`refactor-sweep`](.claude/skills/refactor-sweep/SKILL.md) procedure: the read-only [`refactor-scout`](.claude/agents/refactor-scout.md) proposes up to 5 behavior-preserving improvements (duplication, layering, dead code, consistency, simplification), the planner triages, the `implementer` implements each in its own worktree branch off `origin/main`, and the planner verifies lint/tests and opens one **draft PR** per finding (label `refactor-sweep`, max 3 open). Anything needing an ADR is reported, not implemented. Sweeps never merge or push to `main`.
- **Cross-IDE etiquette:** if you add automation specific to one agent (e.g. Cursor `.mdc` rule, Claude `CLAUDE.md` excerpt, Codex `~/.codex/instructions`), mirror the intent here so other agents stay aligned. Tool-specific files **complement**, never replace, `AGENTS.md`.

## 11. Maintenance of this file

- Update `AGENTS.md` whenever any of the following change: tech stack, repository layout, CI commands, ADR set, role coordination process, privacy/security guardrails, or the "not yet implemented" matrix.
- Keep this file under **~500 lines**; if it grows, factor sections out into `docs/contributing/*.md` and link from here.
- This file is plain Markdown — agents that read AGENTS.md should not need to parse front-matter or YAML.

## 12. Open follow-ups for this setup (`STATUS: IDEA`)

Captured so the next pass can prioritize them; they are not gating:

- `STATUS: IDEA` Add per-package `AGENTS.md` deltas under `apps/mobile/`, `apps/signaling-server/`, and `packages/protocol/` once those packages diverge meaningfully.
- `STATUS: IDEA` Introduce ESLint with a Svelte + TypeScript ruleset (or evaluate Biome) — requires an ADR-level discussion vs. current Prettier-only stance.
- `STATUS: IDEA` Document an explicit threat model and STRIDE/LINDDUN summary in `docs/legal/` or `docs/technical/security-model.md`; today the model is spread across ADR-004 and feature docs.
- `STATUS: IDEA` Add a CODEOWNERS file once contributor roles solidify.
- `STATUS: DECIDED` Release Please wiring landed ([ADR-006](docs/decisions/ADR-006-release-and-changelog-automation.md), [release-and-changelog.md](docs/contributing/release-and-changelog.md)). The workflow is active on `main` but produces no release until releasable commits accrue after `bootstrap-sha`. Phase 2 follow-up: scripted `versionCode` bump for `apps/mobile`.
- `STATUS: DECIDED` Browser E2E harness: `apps/e2e` + Playwright + dev-only `window.__guardedsleep_test` ([ADR-007](docs/decisions/ADR-007-browser-e2e-test-strategy.md)).
- `STATUS: IDEA` Add an `examples/` folder with minimal reproductions (e.g. standalone WebRTC PoC harness) so the PoC gate from ADR-001 has a permanent home **outside** Playwright app tests.
