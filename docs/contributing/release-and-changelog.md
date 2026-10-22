# Releases and Changelog

> **Superseded by [ADR-020](../../openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md) §8 and §10:** the Release Please node setup (components `protocol`, `signaling-server`, `mobile`), the Husky/commitlint commit check and the `release:lint` flow. Alpha uses manual `android-v*` tags; Release Please returns at beta with components `app`, `server`, `core`; Conventional Commits are checked on the PR title only. Replaced in toolchain realignment wave 1 (hooks, PR-title lint) and T1 (Release Please); this procedure stays valid until then.

> **Tooling decision:** [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md) — Release Please in monorepo manifest mode.
>
> **External references** (read once, then come back here):
>
> - Release Please overview: <https://github.com/googleapis/release-please>
> - Release Please GitHub Action (v4): <https://github.com/googleapis/release-please-action>
> - Config schema (full options): <https://github.com/googleapis/release-please/blob/main/docs/manifest-releaser.md>
> - Conventional Commits 1.0: <https://www.conventionalcommits.org/>
> - Keep a Changelog 1.1: <https://keepachangelog.com/>
> - SemVer 2.0: <https://semver.org/>

This document is the single source of truth for **what a contributor needs to do** during normal development so that releases happen automatically and correctly.

---

## 1. TL;DR

- Write commits using **Conventional Commits**. Husky's `commit-msg` hook + CI enforce this.
- Use `feat!:` or a `BREAKING CHANGE:` footer for breaking changes.
- Pull request titles must also be Conventional (squash merge uses them as commit subject).
- Don't edit `CHANGELOG.md` by hand for **released** versions — Release Please owns the file. The `## [Unreleased]` section may still receive manual entries until automation is activated; once active, even Unreleased entries are normally added via merged commits.
- Releases are cut by merging the **"chore(release): main"** pull request that Release Please opens.
- **`@guardedsleep/mobile`'s version is the user-facing product version.** `protocol` and `signaling-server` carry their own SemVer streams. Package versions never need to match. See §16.
- **Inter-package bumps propagate automatically** via the `node-workspace` plugin. See §17.
- **Project milestones get manual meta-tags** like `mvp-v0.1.0` listing the exact package triplet. See §18.

## 2. Commit type → changelog section → SemVer bump

Configured in [`release-please-config.json`](../../release-please-config.json) and [`commitlint.config.mjs`](../../commitlint.config.mjs).

| Commit prefix                                         | CHANGELOG section                                      | Pre-1.0 bump               | Post-1.0 bump | Hidden by default? |
| ----------------------------------------------------- | ------------------------------------------------------ | -------------------------- | ------------- | ------------------ |
| `feat: ...`                                           | **Added**                                              | minor                      | minor         | no                 |
| `fix: ...`                                            | **Fixed**                                              | patch                      | patch         | no                 |
| `perf: ...`                                           | **Changed**                                            | patch                      | patch         | no                 |
| `revert: ...`                                         | **Removed**                                            | patch                      | patch         | no                 |
| `security: ...`                                       | **Security**                                           | patch                      | patch         | no                 |
| `feat!: ...` _or_ footer `BREAKING CHANGE:`           | **Added** + `⚠ BREAKING CHANGES` block at top of notes | **minor** (project policy) | major         | no                 |
| `docs: ...`                                           | Documentation                                          | none                       | none          | **yes**            |
| `refactor: ...`                                       | Refactoring                                            | none                       | none          | **yes**            |
| `test: ...`                                           | Tests                                                  | none                       | none          | **yes**            |
| `build: ...`                                          | Build                                                  | none                       | none          | **yes**            |
| `ci: ...`                                             | CI                                                     | none                       | none          | **yes**            |
| `chore: ...`                                          | Miscellaneous                                          | none                       | none          | **yes**            |
| `style: ...`                                          | Style                                                  | none                       | none          | **yes**            |
| `deps: ...` (non-standard, used inside `chore(deps)`) | Dependencies                                           | none                       | none          | **yes**            |

**Pre-1.0 nuance (project-wide):** while we are on `0.x.y`, `feat!:` bumps **minor** (not major). This is the Conventional Commits / SemVer convention for unstable releases and is the documented Release Please default we accept (`bump-minor-pre-major: true`). When the project reaches `1.0.0`, breaking changes bump major as expected.

**Scope:** scopes are free-form but should reflect the workspace package: `feat(mobile): ...`, `fix(signaling): ...`, `feat(protocol): ...`, `chore(repo): ...`, `docs(adr): ...`. Release Please **does not** route commits to packages by scope — it routes by **changed file paths**. The scope is for humans.

## 3. Examples

```text
feat(mobile): add slide-to-lock gesture for listen mode
fix(signaling): reject duplicate hello frame within 1 s window
perf(protocol): zero-copy heartbeat encode for 32-byte messages
revert(mobile): undo battery-optimization deep-link (broke on MIUI)
security(signaling): patch WS payload validator regex (no CVE assigned)
docs(adr): add ADR-006 release automation
chore(deps): bump prettier to 3.5
feat(mobile)!: change saved-connection storage format

BREAKING CHANGE: clients before mobile-v0.4.0 cannot read the new
saved-connection envelope. Users must re-pair affected devices.
```

The footer **must** read exactly `BREAKING CHANGE:` (note the space, all caps, colon). Variants like `BREAKING-CHANGE:` or `breaking change:` are silently ignored by Release Please.

## 4. Pre-conditions Release Please depends on

These are enforced automatically — listed here so you understand the failure modes.

| Pre-condition                                                         | Enforced where                                                                                     |
| --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Every commit subject is a valid Conventional Commit                   | Husky `.husky/commit-msg` (local) + CI job **`commitlint (PR commits)`** (covers rebases & amends) |
| Every PR title is a valid Conventional Commit                         | CI job **`commitlint (PR title)`** (matters because default merge is **squash**)                   |
| Allowed `type` values include the ones Release Please maps            | [`commitlint.config.mjs`](../../commitlint.config.mjs) `type-enum`                                 |
| Workspace package paths in config match real packages                 | [`release-please-config.json`](../../release-please-config.json) keyed by `packages/*`, `apps/*`   |
| Manifest tracks the current released version per package              | [`.release-please-manifest.json`](../../.release-please-manifest.json)                             |
| `package.json` version matches the manifest before a release          | Release Please updates them in lock-step in the release PR                                         |
| Tag pattern `<component>-v<version>` is free for the proposed version | GitHub repo tag namespace                                                                          |
| Workflow has `contents: write` + `pull-requests: write`               | [`.github/workflows/release-please.yml`](../../.github/workflows/release-please.yml) `permissions` |
| GitHub default branch matches `target-branch` (`main`)                | Repo Settings → Branches                                                                           |

If any of these break, Release Please will either silently skip a commit (most often: non-conforming subject after a rebase) or refuse to open a release PR.

## 5. Local checks before opening a PR

Run these from the repo root (Dev Container or local). They mirror what CI runs and surface Release Please–relevant issues early.

```bash
# 1. All hooks-equivalent checks (formatting, typos, YAML/JSON):
pnpm run lint

# 2. Validate every commit you added on this branch (against origin/main):
pnpm run release:lint
# Override the base if you branched from somewhere else:
RELEASE_LINT_FROM=origin/develop pnpm run release:lint

# 3. Build + test the whole monorepo:
pnpm test
pnpm run build

# 4. (Optional, maintainer only) Preview what Release Please would do.
#    Requires a GitHub token with `contents: read` and `pull-requests: read`:
GH_TOKEN=<token> GITHUB_REPOSITORY=<owner>/<repo> pnpm run release:preview
```

> `release:preview` calls `npx release-please release-pr --dry-run --debug`. It performs **read-only** GitHub API calls and prints what the release PR would contain. No commits, tags, or PRs are created.

## 6. Pull request hygiene (what reviewers expect)

- **Squash merge is the default.** Make sure the PR title is a valid Conventional Commit — it becomes the squash commit subject.
- If the PR contains a **breaking change**, the PR title must use `!:` or the description must include a `BREAKING CHANGE:` footer that survives the squash. The safest approach: put the `BREAKING CHANGE:` block **at the very bottom of the PR description**, preceded by an empty line — GitHub will preserve it in the squashed commit body.
- Tick the boxes in the [PR template](../../.github/pull_request_template.md). The "Conventional title" and "Changelog entry" boxes are non-negotiable for user-visible work.
- If you accidentally land a non-Conventional commit (e.g. via a force-push or web edit), open a follow-up `chore: rewrite commit subject for release please` PR with a corrective `Revert "..."` + redo, or note it under `## [Unreleased]` manually so the changelog stays accurate.

## 7. How a release happens

```
   Conventional commits land on main
              │
              ▼
   release-please-action runs on push
              │
              ▼
   Release PR opened/updated:
     - CHANGELOG.md  ← regenerated sections
     - package.json  ← version bumps (per package)
     - manifest.json ← target versions
     - build.gradle  ← Android versionName (via extra-files)
              │
              ▼
   Maintainer reviews release PR (this is the
   natural Critical Review Gate per role-coordination)
              │
              ▼
   Manual release testing on real Android hardware
   (checklist: manual-release-testing.md) — required
   for user-facing mobile releases; optional for
   protocol/signaling-only bumps
              │
              ▼
   Maintainer merges release PR
              │
              ▼
   release-please-action runs again on the merge:
     - Tags created:  protocol-v0.X.Y, signaling-server-v0.X.Y, mobile-v0.X.Y
     - GitHub Releases created with the same notes as the CHANGELOG diff
              │
              ▼
   (Optional) follow-up workflows trigger on the new tag:
     - docker/push for signaling-server image
     - Android APK build / store upload (manual today)
```

## 8. Trigger Release Please manually

Most of the time, **you don't need to**: the workflow runs on every push to `main` and keeps the release PR up to date.

You only need a manual trigger if:

- You just merged a fix that should appear in the release PR immediately and don't want to wait for the next push.
- You changed `release-please-config.json` and want Release Please to reconcile.
- You want to release from a non-`main` branch (e.g. a hotfix branch).

**How:**

1. GitHub → repo → **Actions** tab.
2. Select the **Release Please** workflow in the sidebar.
3. Click **"Run workflow"**.
4. (Optional) override `target-branch` if releasing from a hotfix branch.
5. Click **Run workflow**.

You can also dispatch via the GitHub CLI:

```bash
gh workflow run "Release Please" --ref main
# or, releasing from a hotfix branch:
gh workflow run "Release Please" --ref hotfix/0.1.x --field target-branch=hotfix/0.1.x
```

## 9. Forcing a specific version

Sometimes you want to release a version Release Please wouldn't pick automatically — for example, the first **`0.1.0` MVP** release after the PoC gate, or a deliberate **`1.0.0`** bump.

Two mechanisms:

- **`Release-As:` commit footer.** Add an empty commit on `main`:

  ```bash
  git commit --allow-empty -m "chore: release 0.1.0" \
    -m "Release-As: 0.1.0"
  git push
  ```

  Release Please picks this up and proposes exactly that version on the **package whose path the commit touches** (use `chore(mobile): release 0.1.0` style scopes when the commit is empty — Release Please reads the directory of the changed files, so for an empty commit, the **next commit** on the package path is what gets the override; the cleanest path is therefore a one-line `release-as` change inside the package directory, e.g. add a comment to `apps/mobile/CHANGELOG.md` or `package.json`).

- **`release-as` in `release-please-config.json`** per package: pin a target version until released, then remove the key. Useful for one-shot major bumps.

For the **initial `0.1.0` MVP**, the cleanest path is to set `release-as: "0.1.0"` for all three packages in the config, let Release Please open the release PR, merge it, then remove the `release-as` keys in a follow-up PR.

## 10. First-time activation (one-off)

This applies **once**, when Release Please is first turned on (which is part of the same commit that introduces this document and ADR-006).

1. Confirm `.release-please-manifest.json` has every package at the version present in its `package.json` (currently `0.0.1` everywhere).
2. Merge the activating PR to `main`.
3. **(Recommended)** Set `bootstrap-sha` in `release-please-config.json` to the **SHA of the merge commit** that activates Release Please:

   ```json
   {
     "bootstrap-sha": "abc1234...",
     "...": "..."
   }
   ```

   This tells Release Please to ignore all commits before activation so the first release PR doesn't try to summarise the entire pre-release history.

4. Wait for the workflow to run. If it opens a release PR you don't want yet, **close it without merging** — Release Please will recompute on the next push.
5. From this point on, follow §7 for every release.

## 11. Phase-aware release strategy

Each project phase has different release expectations.

| Phase                                                                                                                  | Versioning posture                                                                                                               |
| ---------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| **Concept (current)**                                                                                                  | Release Please is wired but inactive — no `Release-As` and no commits since bootstrap. Manual `## [Unreleased]` bullets allowed. |
| **PoC** ([ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)) | Optionally cut a `0.0.x` for each package as the PoC runs; useful for tagging known-good states.                                 |
| **MVP (Phase 1)**                                                                                                      | First real release: `0.1.0` across packages (use `Release-As: 0.1.0`). After that, normal Conventional Commit driven bumps.      |
| **Phase 2 (iOS, multi-parent, talk-back)**                                                                             | Likely `0.2.x → 0.9.x` series; first deliberate `1.0.0` candidate when the feature set is stable.                                |
| **Phase 3+ (video, on-device AI)**                                                                                     | `1.x` series, normal SemVer. Breaking protocol changes go through ADRs first.                                                    |

## 12. Mobile `versionCode` (Android Play Store)

The Play Store requires a **monotonically increasing integer** `versionCode` independent of `versionName`. Release Please updates `versionName` via the `extra-files` rule on `apps/mobile/android/app/build.gradle`. The `versionCode` is **not** managed automatically in MVP — the release PR description must contain a checklist line:

```text
- [ ] Bump `versionCode` in apps/mobile/android/app/build.gradle to <next integer>
```

The maintainer increments it manually before merging the release PR. A small scripted updater is tracked as a Phase 2 follow-up in [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md).

## 13. What `CHANGELOG.md` looks like over time

Until activation, the file mixes a manual `## [Unreleased]` section and historical bullets. After activation, every release Release Please cuts adds an entry like:

```markdown
## [protocol-v0.1.0] - 2026-06-01

### Added

- DataChannel heartbeat encoding (#42)

### Fixed

- Off-by-one in noise threshold parser (#45)

### ⚠ BREAKING CHANGES

- Replace `nonce` field with `n` in saved-connection envelope (#48)
```

Each package's section is prefixed with its component name. The `## [Unreleased]` section is kept at the top until the next release PR rolls it forward.

## 14. Troubleshooting

| Symptom                                                           | Likely cause                                                                                  | Fix                                                                                                                                              |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Release PR not opened after a push                                | No commits with releasable types (`feat`, `fix`, `perf`, `revert`, `security`) since last tag | Verify with `pnpm run release:lint`; ensure your commit isn't a `docs`/`chore` only.                                                             |
| Breaking change didn't bump major (post-1.0)                      | Footer mistyped (must be exactly `BREAKING CHANGE:`)                                          | Add a `chore: trigger breaking bump` empty commit with correct footer.                                                                           |
| Breaking change bumped only minor (pre-1.0)                       | Intended — see §2 "Pre-1.0 nuance". Use `Release-As: 1.0.0` to graduate.                      | n/a                                                                                                                                              |
| Release PR has weird "since-beginning-of-time" content            | `bootstrap-sha` missing or wrong                                                              | Set `bootstrap-sha` in `release-please-config.json` to the activating merge commit and re-run the workflow.                                      |
| Package didn't bump despite commits touching it                   | Commits touched the package directory but had hidden types (`chore`, `docs`, etc.)            | Re-classify next time; or land a `chore: re-release` empty commit with `Release-As: X.Y.Z`.                                                      |
| `commitlint (PR commits)` fails after a rebase                    | Rebase introduced commits with non-Conventional subjects                                      | Either amend the offending commits (`git rebase -i`) or squash them with a Conventional message; Husky catches local edits, CI catches the rest. |
| GitHub Action fails with `Resource not accessible by integration` | Workflow lacks `contents: write` or `pull-requests: write`                                    | Already configured in our workflow file; check repo Settings → Actions → Workflow permissions is set to **Read and write**.                      |

## 15. Discarded alternatives

See [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md) for the full options analysis. Briefly: Changesets duplicates effort already done by Conventional Commits; semantic-release does not fit app-store cadence; manual-only is the status quo we accept until first artifact ships. Linked versions across all packages are rejected (would force needless lockstep bumps); a single umbrella version is rejected in favour of "mobile = product version" plus manual milestone meta-tags.

## 16. Versioning posture — "mobile is the product version"

> Sections 16 to 19 keep the pre-ADR-020 npm component names for history. `@guardedsleep/signaling-server` is now the Rust `guardedsleep-server` (planned component `server`), and `@guardedsleep/protocol` no longer exists. Current posture: AGENTS.md section 8.

The monorepo contains **independently versioned packages (the hand-written `@guardedsleep/protocol` package was removed; protocol types are generated from the Rust core into `packages/core-wasm`, which is private and unversioned for releases, and the signaling server is the Rust `crates/guardedsleep-server` image)**:

| Package                          | Role                                | Whose version do users / operators see?                                             |
| -------------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------- |
| `@guardedsleep/mobile`           | User-facing Android app (Capacitor) | **Yes — this is the headline "GuardedSleep version"** shown in About, Play, F-Droid |
| `@guardedsleep/signaling-server` | Self-hostable WebSocket relay       | Operators see this in Docker tags (`ghcr.io/<owner>/guardedsleep-signaling:vX.Y.Z`) |

**Practical implications:**

- When someone reports a bug, ask for the **mobile version** (`mobile-v0.3.2`). The compatibility matrix (§19) maps that to the protocol and signaling-server versions it was tested against.
- The mobile app's About screen / settings should display its own `versionName`. It **may** also display the protocol version it was built against (`import.meta.env.VITE_PROTOCOL_VERSION`) for debugging.
- A signaling-server operator can independently upgrade their server to a newer `signaling-server-v…` without forcing a mobile-app update, **as long as** the new server is still protocol-compatible (which the compatibility matrix tells them).
- Marketing material, GitHub Discussions, store listings refer to versions as **"GuardedSleep v0.3"** (= mobile's MAJOR.MINOR).

## 17. Inter-package dependency tracking (`node-workspace` plugin)

Release Please is configured with the **`node-workspace` plugin** ([config reference](../../release-please-config.json) → `plugins: ["node-workspace"]`). This automates dependency-graph awareness:

_Illustrative only: no workspace package has a released dependent today (the former `@guardedsleep/protocol` package is gone)._

**Scenario: a shared package `protocol` bumps from `0.1.0` → `0.2.0`** (because a `feat:` commit landed in its directory).

What Release Please does automatically:

1. Detects that `apps/mobile` and `apps/signaling-server` both declare a workspace dependency on `@guardedsleep/protocol`.
2. Computes a **topologically ordered** version bump: `protocol` first, then its consumers.
3. Adds a **patch bump** to `mobile` (e.g. `0.3.1 → 0.3.2`) and `signaling-server` (e.g. `0.1.4 → 0.1.5`).
4. Adds an explicit changelog entry under the consumer's section:

   ```markdown
   ## [mobile-v0.3.2]

   ### Dependencies

   - The following workspace dependencies were updated
     - dependencies
       - @guardedsleep/protocol bumped to 0.2.0
   ```

5. Creates **all three tags** when the release PR is merged: `protocol-v0.2.0`, `signaling-server-v0.1.5`, `mobile-v0.3.2`.

**What this protects against:**

- Silent ABI drift — every consumer of a bumped package gets a visible version bump, so an operator who upgrades `signaling-server-v0.1.5` knows it embeds `protocol-v0.2.0`.
- "Phantom" releases — if a mobile-only commit lands while protocol is unchanged, only `mobile` bumps. No solidarity bumps for protocol/signaling.
- Hand-maintained dependency declarations going stale — the plugin keeps them in lockstep.

**Notes on the `workspace:*` specifier:** Our `package.json` files use `"@guardedsleep/protocol": "workspace:*"`. The `node-workspace` plugin **does not rewrite** this specifier (pnpm resolves it locally regardless). What it does is propagate the **bump signal** through the graph. If we ever publish `@guardedsleep/protocol` to npm, pnpm will replace `workspace:*` with the concrete resolved version at publish time.

## 18. Milestone meta-tags

Project phases ([feature roadmap](../backlog.md)) and major scope checkpoints get **manually created annotated tags** that are not produced by Release Please. They are pure documentation aids.

**Naming convention:** `<phase-or-milestone>-v<semver>`. Examples:

- `mvp-v0.1.0` — the commit that ships the first MVP per [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md).
- `poc-v0.0.1` — the commit at which the PoC gate is declared passed.
- `phase-2-v0.2.0` — entry into Phase 2 (iOS, multi-parent, talk-back).
- `phase-3-v0.3.0` — entry into Phase 3 (video, on-device AI).
- `audit-v0.x.y` — an external security audit was performed against this commit.

**How to create one (maintainer only, after merging the corresponding Release Please PR):**

```bash
# Identify the commit (usually the merge commit of the release PR that closes the milestone):
git checkout main && git pull

# Create the annotated tag with the package triplet in the message:
git tag -a mvp-v0.1.0 -m "GuardedSleep MVP v0.1.0

Package triplet:
- protocol-v0.1.0
- signaling-server-v0.1.0
- mobile-v0.1.0

Scope: matches the MVP definition in ADR-001 (decision archive) and the capability specs in openspec/specs/.
PoC gate: PASSED on YYYY-MM-DD.
License: AGPL-3.0-or-later (see LICENSE).
"

git push origin mvp-v0.1.0
```

**What to do after creating the meta-tag:**

1. Create a **GitHub Release** for the meta-tag (UI: "Draft a new release" → select the meta-tag → reuse the message body). Mark as "latest" if it represents the current canonical user-facing version.
2. Update §19 (Compatibility matrix) below.
3. Reference the meta-tag in any external communication (blog post, F-Droid changelog text, …).

**What meta-tags are not:**

- They are **not** consumed by Release Please. They do not influence next version computation.
- They are **not** auto-created. The maintainer creates them deliberately when a milestone is reached.
- They do **not** replace per-package tags. The per-package tags are still authoritative for SemVer.

## 19. Compatibility matrix

This table is the human-readable answer to "which versions go together?" Update it **manually** whenever a new milestone meta-tag is created, or whenever a non-trivial breaking change lands.

| Milestone meta-tag     | `@guardedsleep/protocol` | `@guardedsleep/signaling-server` | `@guardedsleep/mobile` | Notes                                                                                                                                                |
| ---------------------- | ------------------------ | -------------------------------- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| _(none yet — pre-PoC)_ | `workspace:*` (0.0.1)    | `workspace:*` (0.0.1)            | `workspace:*` (0.0.1)  | Scaffolded code only. No releases. See [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md). |

**Convention for filling rows:**

- One row per **milestone meta-tag**, not per Release Please cut. Intermediate per-package bumps live in `CHANGELOG.md` only.
- The `Notes` column states the headline change relative to the previous row.
- When `protocol` introduces a breaking change (`feat!:` in a shared package or the Rust wire format), add a row even if no milestone meta-tag exists, with the meta-tag column set to `—` and Notes describing the contract change.
- Once Phase 2 follow-up "Automated compatibility-matrix updater" lands (see [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md) discarded alternatives), this table becomes auto-generated.
