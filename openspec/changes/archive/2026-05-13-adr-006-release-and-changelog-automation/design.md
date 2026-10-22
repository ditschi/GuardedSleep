# ADR-006: Release and Changelog Automation (Release Please)

- Status: DECIDED
- Date: 2026-05-13
- Deciders: User decision after option analysis (Technical Architect + Product Owner + Critical Reviewer perspectives synthesized in the analysis)
- Supersedes: nothing; supplements [docs/contributing/release-and-changelog.md](../../../../docs/contributing/release-and-changelog.md)

> **Partially superseded (2026-10-20):** the release component layout (`protocol`, `signaling-server`, `mobile` as npm packages with the `node` release type and `node-workspace` plugin) and the Husky + commitlint commit check → [ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md) §8 and §10: alpha uses a manual `android-v*` tag; at beta Release Please runs with components `app`, `server`, `core`; Conventional Commits are checked on the PR title only, in CI. `packages/protocol` and `apps/signaling-server` are deleted.

## Context

The repository already enforces **Conventional Commits** (via Husky + commitlint (superseded, see ADR-020 §8), and via PR-title check in CI for squash merges) and maintains a manual **Keep a Changelog**–style [`CHANGELOG.md`](../../../../CHANGELOG.md). We need a deterministic, low-friction path from those signals to:

1. A **SemVer version** per workspace package.
2. A **changelog** with explicit sections (Added / Fixed / Changed / Removed / Security) that maps from commit types.
3. A **release artifact trail** (git tag + GitHub Release) once the project ships its first build.

The repository is a **pnpm-workspaces monorepo** with three versionable units (`@guardedsleep/protocol`, `@guardedsleep/signaling-server`, `@guardedsleep/mobile`) (superseded, see ADR-020 §10: components `app`, `server`, `core`). The mobile app additionally has Android-specific version metadata (`versionName` SemVer + `versionCode` integer).

## Options Considered

### Option A: Release Please (Google)

A GitHub Action that scans Conventional Commits since the last tag, opens a "release PR" that bumps versions and updates the changelog, and on merge creates git tags + GitHub Releases.

- **Pros**
  - Native monorepo manifest mode with per-package versioning.
  - Updates the **existing** root `CHANGELOG.md` in Keep a Changelog–compatible form (no format migration).
  - Changelog **sections are configurable** — we can map Conventional types directly to our Added/Fixed/Changed/Removed/Security headers.
  - Release happens as a **PR** → goes through the existing review pipeline (PR template, CI checks, Critical Review Gate).
  - Recognizes `feat!:` / `BREAKING CHANGE:` footer and renders a `⚠ BREAKING CHANGES` section.
  - Mature, GitHub-maintained, large user base.
- **Cons**
  - First-time bootstrap on a long-lived branch needs a `bootstrap-sha` or an initial release PR that is closed/edited manually.
  - SemVer **pre-1.0**: a `feat!:` bumps minor (not major). Must be communicated to contributors.
  - Configuration for monorepo + custom sections is non-trivial.

### Option B: Changesets (Atlassian)

PR authors add an explicit `.changeset/<name>.md` per PR declaring bump type + summary.

- **Pros**
  - Explicit human authorship of release notes (better narrative than aggregated commit subjects).
  - Standard in pnpm/Svelte/Vite ecosystem.
  - Strong monorepo support including linked and fixed modes.
- **Cons**
  - **Duplicates** the discipline already enforced via Conventional Commits — authors must write both.
  - Generates **per-package CHANGELOG.md** files; we would either lose the central `CHANGELOG.md` or maintain a merger step.
  - Higher day-to-day friction for a small/solo maintainer team.

### Option C: semantic-release

Fully automated: every merge to main is evaluated; if release-worthy commits are present, a release is cut immediately.

- **Pros**
  - Zero manual step between commit and release.
  - Rich plugin ecosystem.
- **Cons**
  - Monorepo support is via third-party wrappers, less polished.
  - Encourages "release on every merge", which is incompatible with mobile app-store cadence (Play / F-Droid review windows).
  - Would **replace** the manually curated `CHANGELOG.md` rather than incrementally update it.

### Option D: Status quo (manual `CHANGELOG.md`, manual tags)

- **Pros**: No new moving parts.
- **Cons**: Drift between `CHANGELOG.md` and tags is inevitable once releases start; no machine-verified SemVer.

### Option E: Hybrid — Release Please for `@guardedsleep/protocol` only

Restrict automation to the library; apps stay manual.

- **Pros**: Lowest risk if we are uncertain about full-repo automation.
- **Cons**: Two parallel workflows for contributors; loses the cross-package release narrative.

## Decision

**Adopt Option A — Release Please** in **manifest (monorepo) mode**, configured for the three workspace packages with **independent versioning** and the **`node-workspace` plugin** enabled to propagate version bumps across the workspace dependency graph. The **mobile app's version is treated as the user-facing product version**; `protocol` and `signaling-server` carry their own SemVer streams. **Project milestones** (MVP, Phase 2, …) receive **manual meta-tags** (e.g. `mvp-v0.1.0`) that pin the exact package triplet — these are documentation aids, not Release Please artefacts. Triggered both **automatically on push to `main`** and via **`workflow_dispatch`** (manual button on GitHub Actions UI). Initial activation deferred until the **PoC gate** ([ADR-001](../2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)) passes; the wiring is committed in this change so activation is a no-op flag flip.

### Concrete configuration choices

| Decision                          | Value / Setting                                                                                                                                                                                                                                                                                                                         |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mode                              | `manifest` (monorepo)                                                                                                                                                                                                                                                                                                                   |
| Packages tracked                  | `packages/protocol`, `apps/signaling-server`, `apps/mobile` (superseded, see ADR-020 §10)                                                                                                                                                                                                                                               |
| Versioning posture                | **Independent** per package; **`@guardedsleep/mobile` is the user-facing product version** (its `versionName` is what appears on Play Store / F-Droid / About screens). `protocol` and `signaling-server` evolve on their own commit streams. No `linked-versions` plugin.                                                              |
| Inter-package dependency tracking | **`node-workspace` plugin enabled.** When `protocol` bumps, `mobile` and `signaling-server` automatically receive a patch bump and a `Dependencies` changelog entry in the **same** release PR. Tags are produced in topological order.                                                                                                 |
| Milestone tagging                 | **Manual meta-tags** at project milestones: `mvp-v0.1.0`, `phase-2-v0.2.0`, etc. Annotated tags whose message lists the exact `protocol-v…` / `signaling-server-v…` / `mobile-v…` triplet they cover. Release Please does not produce these; the maintainer creates them via `git tag -a` after merging the relevant Release Please PR. |
| Initial version (manifest)        | All three at **`0.0.1`** matching `package.json`                                                                                                                                                                                                                                                                                        |
| Tag pattern                       | **`<component>-v<version>`** with **hyphen** separator (Release Please default with `include-component-in-tag: true`). Examples: `protocol-v0.1.0`, `signaling-server-v0.1.0`, `mobile-v0.1.0`.                                                                                                                                         |
| Tag separator rationale           | Hyphen chosen over slash because (a) it is the Release Please default, (b) it matches the broader monorepo ecosystem (Lerna, Nx, Changesets), (c) GitHub release URLs stay un-encoded, (d) the Go-style slash separator is only mandated by Go's toolchain — not applicable here.                                                       |
| Changelog file                    | **Root** `CHANGELOG.md` (single file — `aggregate-pull-requests: true`)                                                                                                                                                                                                                                                                 |
| Mapping `feat:`                   | Section **`Added`**, minor bump                                                                                                                                                                                                                                                                                                         |
| Mapping `fix:`                    | Section **`Fixed`**, patch bump                                                                                                                                                                                                                                                                                                         |
| Mapping `perf:`                   | Section **`Changed`**, patch bump                                                                                                                                                                                                                                                                                                       |
| Mapping `revert:`                 | Section **`Removed`**, patch bump                                                                                                                                                                                                                                                                                                       |
| Mapping `security:` (custom)      | Section **`Security`**, patch bump (commitlint type-enum updated; see Consequences; commitlint itself superseded, see ADR-020 §8)                                                                                                                                                                                                       |
| Hidden by default                 | `docs`, `chore`, `style`, `refactor`, `test`, `build`, `ci`, `deps`                                                                                                                                                                                                                                                                     |
| Breaking change marker            | `feat!:` or `BREAKING CHANGE:` footer → `⚠ BREAKING CHANGES` block at top of release notes                                                                                                                                                                                                                                              |
| Pre-1.0 SemVer behavior           | **`feat!:` bumps minor** (project accepts this until a deliberate `release-as: 1.0.0` is requested)                                                                                                                                                                                                                                     |
| Compatibility matrix              | Maintained as a table in [`docs/contributing/release-and-changelog.md`](../../../../docs/contributing/release-and-changelog.md). Each meta-tag's annotated message also lists the exact package triplet.                                                                                                                                |
| Mobile `versionName` update       | Via `extra-files` rule in Release Please config targeting `apps/mobile/android/app/build.gradle`                                                                                                                                                                                                                                        |
| Mobile `versionCode` update       | **Manual** for now (Phase 2 follow-up: small script invoked from the release PR)                                                                                                                                                                                                                                                        |
| Publishing                        | **None on tag** in MVP — no `npm publish`, no Docker push, no APK upload. Tag = "code is release-ready".                                                                                                                                                                                                                                |
| App-store / GHCR distribution     | Triggered separately, **after** the Release Please PR is merged (existing `docker-signaling.yml` etc.)                                                                                                                                                                                                                                  |

### Activation flow

1. Configuration files (`release-please-config.json`, `.release-please-manifest.json`, `.github/workflows/release-please.yml`) are committed alongside this ADR.
2. The workflow file is committed in active form (runs on `push: main`), but **no release will trigger until commits with releasable types exist** since the bootstrap.
3. Maintainer sets `bootstrap-sha` in `release-please-config.json` to the SHA of the merge commit that introduces Release Please. After that, all subsequent Conventional Commits to `main` accrue toward the next release PR.
4. The first release PR may be closed without merging if the bootstrap state is undesirable; subsequent runs will reconstruct it.

## Consequences

### Positive

- **Single source of truth** for what shipped: the merged release PR and the corresponding tag.
- No drift between `CHANGELOG.md` and tags possible once active.
- Reuses **existing** Conventional Commits enforcement — no new contributor obligations except for the _security_ commit type addition.
- The release PR provides a natural choke-point for **role-based review** (Critical Reviewer, Security/Privacy Architect) before any version is cut.

### Negative

- One additional GitHub Action runs on every push to `main` (small CI cost).
- Contributors must learn the **`feat!:` syntax** for breaking changes; a malformed footer (`BREAKING-CHANGE:` instead of `BREAKING CHANGE:`) silently misses the major bump signal pre-1.0 — mitigated by documentation and PR template reminder.
- A `security:` commit type was added to commitlint to allow tagging security fixes distinctly. This deviates slightly from stock `@commitlint/config-conventional`. **Recorded as a documented project convention.**

### Risks

- **Risk:** Release Please bootstrap creates a huge initial PR scanning all historical commits. **Mitigation:** Maintainer sets `bootstrap-sha` to the activation commit; documented in [release-and-changelog.md](../../../../docs/contributing/release-and-changelog.md).
- **Risk:** Mobile `versionCode` drifts from `versionName` (manual updates). **Mitigation:** Phase 2 follow-up to script this; in MVP, the release PR description must list the `versionCode` increment as a checklist item.
- **Risk:** Pre-1.0 behavior surprises a contributor who expected `feat!:` → major. **Mitigation:** Explicit note in [release-and-changelog.md](../../../../docs/contributing/release-and-changelog.md) and the PR template.

## Discarded alternatives

- **Changesets** — adds ceremony on top of already-enforced Conventional Commits; replaces our single root `CHANGELOG.md` with per-package files.
- **semantic-release** — release-per-merge model incompatible with app-store cadence; CHANGELOG migration cost.
- **Status quo (manual)** — accepted only as the _current_ state; not durable once releases begin.
- **Release-Please for protocol only** — fragmented developer experience; only useful if Option A turns out to fail for apps in practice.
- **Linked versions across all packages** (Release Please `linked-versions` plugin) — rejected because it would force `mobile` and `signaling-server` to bump in lockstep with `protocol` even when their own code is unchanged. This produces SemVer "lies" (a `mobile-v0.4.0` that contains no mobile-side change) and wastes version numbers. The `node-workspace` plugin gives us the desired behaviour (propagated patch bumps with explicit `Dependencies` changelog entries) without the lockstep cost.
- **Slash-separated tags (`protocol/v0.1.0`)** — rejected because the Release Please default uses hyphens, GitHub release URLs would be URL-encoded, and the slash separator is only mandated by Go's toolchain (irrelevant here). See "Tag separator rationale" in the configuration table.
- **Single umbrella version for the whole project** — rejected because the three packages have different change cadences (signaling can ship security fixes without a mobile release; protocol can evolve without affecting deployed apps). Replaced by the cleaner principle "**mobile version = product version**" + manual milestone meta-tags for human-facing communication.
- **Automated compatibility-matrix updater** — considered as a follow-up that would, on every Release Please merge, re-write a table in `docs/contributing/release-and-changelog.md` listing the latest tagged versions of all packages. Deferred to Phase 2 because the matrix is small enough to maintain manually until releases start.
