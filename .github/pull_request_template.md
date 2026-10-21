## Summary

<!-- Short description of what this PR does. -->

## Checklist

- [ ] **Conventional PR title** — squash merge uses it as the commit subject. Format: `<type>(<scope>): <subject>`. Types: `feat`, `fix`, `perf`, `revert`, `security`, `docs`, `refactor`, `test`, `build`, `ci`, `chore`, `style`.
- [ ] **Breaking change?** Use `<type>!:` in the title **and** add a `BREAKING CHANGE: ...` block at the bottom of this description (preserved in the squash commit body). See [release-and-changelog.md §3](../docs/contributing/release-and-changelog.md).
- [ ] **User-visible change?** Add a bullet under `## [Unreleased]` in [CHANGELOG.md](../CHANGELOG.md) **or** rely on Release Please to pick it up from a `feat:` / `fix:` / `perf:` / `revert:` / `security:` commit (see [release-and-changelog.md §2](../docs/contributing/release-and-changelog.md)).
- [ ] Ran **`just lint`** locally.
- [ ] Conventional PR title (checked in CI; commits are not checked, ADR-020 §8).
- [ ] Docs-only: status tags / ADR rules respected (see [`.cursor/rules/docs-conventions.mdc`](../.cursor/rules/docs-conventions.mdc) and [AGENTS.md §6.1](../AGENTS.md)).
- [ ] Architecture/scope/security-model change? An ADR exists or is part of this PR (see [docs/decisions/](../docs/decisions/README.md) and [AGENTS.md §6.2](../AGENTS.md)).

<!-- For breaking changes only, uncomment and fill in:

BREAKING CHANGE: <describe what breaks and the migration path>

-->
