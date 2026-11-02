# Releases and changelog

Short version for the alpha. The decision history (Release Please, Changesets, versioning posture) is
in [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md)
and ADR-020 §10 in the [decision archive](../../openspec/changes/archive/README.md).

- **Changelog:** [CHANGELOG.md](../../CHANGELOG.md) follows [Keep a Changelog](https://keepachangelog.com/).
  Add user-visible changes as bullets under `## [Unreleased]`; never edit released sections.
- **PR titles:** Conventional Commits (`<type>(<scope>): <subject>`), checked in CI. Commits that reach
  `main` by rebase carry Conventional subjects too (AGENTS.md §8).
- **Breaking changes:** `<type>!:` in the title **and** a `BREAKING CHANGE: <description>` footer in the PR
  description (for a rebase merge, in the commit that breaks).
- **Alpha releases:** manual. Push an `android-v*` tag; `versionCode` is derived from `versionName` in
  `version.properties` and never edited by hand (AGENTS.md §8).
- **Release Please** is T1 (beta). Its workflow sits disabled in
  [`.github/workflows-disabled/`](../../.github/workflows-disabled/) until then; `just release-preview`
  shows what it would do (maintainer, needs a GitHub token).
