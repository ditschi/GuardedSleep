# Proposal

## Why

Agents write most of the code in this repository. CodeScene's research ("Code Health", and A. Tornhill, "AI-Ready Code", 2026) finds that AI changes in unhealthy code carry at least 60 % more defects, and that study only looked at code with a health of 7 or more. Healthy code is a precondition for reliable agent work; tests, coverage and review stay necessary.

The owner decided (2026-11-23) not to use the proprietary CodeScene score (paid token). We build our own 1–10 score from free tools, modelled on CodeScene's smells, and make it a gate agents can develop against. Target: repo score at least 9.5, aim 10.

A first measurement with a prototype (lizard 1.26.1 + jscpd, 2026-11-23, see design §8) gives **6.97** for the whole repo (Rust 5.30, Kotlin 7.55, TypeScript 8.89), 132 of 394 files below 9.5 and 18 files red.

This changes a prior decision: ADR-020 says "No ESLint and no Biome at T0". This change adds ESLint as a **metric source for the health score only** (no style rules, not a lint step of its own), so the Critical Reviewer role applies (AGENTS.md §6). Tooling only otherwise: no product behaviour, architecture, scope or security model changes. Tier T0.

## What Changes

1. **Checker `just code-health`** (Node script under `scripts/code-health/`): runs one analyzer per language, merges the raw metrics into one normalized record per function and file, and scores each file from 10.0 down to 1.0. The repo is split into three **components** (`rust`, `android`, `web`), each with its own pipeline, score and baseline (`--component <name>`); each component reaches ≥ 9.5 on its own; a repo total (LOC-weighted mean) is reported for information only, and the worst file is reported.
   - Rust: **clippy** (`cognitive_complexity`, `too_many_lines`, `too_many_arguments`, `fn_params_excessive_bools`) in its own target dir and clippy config.
   - Kotlin: **detekt** CLI (complexity rule set) pinned through mise like ktfmt.
   - TypeScript and Svelte scripts: **ESLint** core rules (`complexity`, `max-depth`, `max-lines-per-function`, `max-params`, `max-lines`) plus **eslint-plugin-sonarjs** (`cognitive-complexity`, `expression-complexity`, `no-identical-functions`).
   - All languages: a small **tree-sitter structure pass** for what no tool reports (Bumpy Road, control-flow nesting, Rust cyclomatic complexity and complex conditionals, assertion blocks, primitive obsession, suppression scan) and **jscpd** for duplication.
2. **Config and docs:** thresholds and weights in `tools/code-health/config.toml` (versioned, every value commented, own choices marked `assumption`); the tool configs (clippy.toml, detekt.yml, ESLint rule options) are generated from it. `docs/contributing/code-health.md` explains score, smells and fixes.
3. **Output:** human table (file, score, smells with line) by default, `--json` for agents, `--changed [base]` and `--staged` for the delta.
4. **Gate** (`--check`) in lefthook and in the CI `lint` job: a new or changed file scores ≥ 9.5; a file listed in the ratchet baseline must not get worse; no component score may sink. **Ratchet baseline** `tools/code-health/baseline.json` while the repo is below 9.5. Suppressions only by inline comment with a reason.
5. **Agents:** AGENTS.md gets a "Code health" section (run `just code-health --changed` before every commit, fix smells instead of suppressing them); `refactor-sweep` and `refactor-scout` use the score as their target metric; a project skill `code-health-fix` (playbook per smell, based on our checker output) and, after owner approval, selected skills.sh skills.
6. **Refactoring** toward ≥ 9.5 in small, tested, behaviour-preserving steps, hotspots (score × churn) first; big restructurings are asked first.

Out of scope: the CodeScene service or token; change coupling and knowledge-distribution metrics (CodeScene's behavioural analysis); a shared implementation for other repositories (dropped by the owner: only the goals are shared, tools are chosen per repository, design §7); ESLint style or correctness rules.

Persona impact (`docs/concept/personas.md`): no end-user persona directly. Parents benefit indirectly through fewer defects in safety code (`link.rs` is the top hotspot). Contributors trade a stricter commit gate for code that agents can change safely.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: code health score, gate and ratchet baseline; the pre-commit hook runs the health check.

## Impact

- New: `scripts/code-health/` (+ Vitest tests), `tools/code-health/{config.toml,baseline.json}`, `docs/contributing/code-health.md`, `.claude/skills/code-health-fix/`.
- Changed: `justfile` (`code-health` recipe, called from `_lint-code`), `lefthook.yml`, `.github/workflows/ci.yml` (lint job), `docs/ci-gates.md` (`lint` row), `mise.toml`/`mise.lock` (detekt-cli jar), root `package.json` + `pnpm-lock.yaml` (eslint, typescript-eslint, eslint-plugin-sonarjs, eslint-plugin-svelte, jscpd, web-tree-sitter + grammars, smol-toml), `AGENTS.md` (§4, §5.2 ESLint note, new section), `.claude/skills/refactor-sweep/SKILL.md`, `.claude/agents/refactor-scout.md`.
- Code across `crates/`, `apps/android`, `apps/web`, `apps/e2e` during the refactoring phase (separate commits/PRs).
- CI: the `lint` job gets about 1–2 min longer (second clippy pass with health lints in `target/code-health`, detekt, ESLint). Maintenance budget: five pinned tools more; Renovate groups them.
