# Proposal

## Why

Agents write most of the code in this repository. CodeScene's research ("Code Health", and A. Tornhill, "AI-Ready Code", 2026) finds that AI changes in unhealthy code carry at least 60 % more defects, and that study only looked at code with a health of 7 or more. Healthy code is a precondition for reliable agent work; tests, coverage and review stay necessary.

The owner decided (2026-11-23) not to use the proprietary CodeScene score (paid token). We build our own 1–10 score from free tools, modelled on CodeScene's smells, and make it a gate agents can develop against. Target: repo score at least 9.5, aim 10.

A first measurement with a prototype (lizard 1.26.1 + jscpd, 2026-11-23, see design §8) gives **6.97** for the whole repo (Rust 5.30, Kotlin 7.55, TypeScript 8.89), 132 of 394 files below 9.5 and 18 files red.

This changes a prior decision: ADR-020 says "No ESLint and no Biome at T0". This change adds ESLint only for a structure report on the Svelte scripts, which ai-health cannot parse (eslint-plugin-svelte + sonarjs, no style rules, no score), so the Critical Reviewer role applies (AGENTS.md §6). Tooling only otherwise: no product behaviour, architecture, scope or security model changes. Tier T0.

## What Changes

1. **Checker `just code-health` on ai-health** (github.com/epatel/ai-health, MIT, pinned to the v0.4.0 commit `033d1867` with hash-locked dependencies; owner decision 2026-11-23, design §0). It replaces the originally planned own tree-sitter structure pass and jscpd. One source per smell: clippy and ESLint stay linters, their structure rules do not gate.
   - Three **components** with their own profile and score: `rust` (`crates/`), `web` (`apps/web`, `apps/e2e`, `packages/`) and `android` (`apps/android/`, via ai-health once a Kotlin adapter exists; until then reported as "not yet scored", detekt at most as a transitional linter). Svelte: ESLint + eslint-plugin-svelte + sonarjs as a report without a score.
   - Profile "strict" (confirmed): CodeScene thresholds, `decay = 8`, `symbol_cap = 4`, `density_loc = 100000`, delta tolerance 0; `large_file` 1000 (Kotlin 600). Goal score 10, floor 9.5 per component; a repo total is informational.
2. **Output:** `aihealth scan` as the report, `--changed`/`--staged` deltas, `--json` for agents; always with an explicit `--config`.
3. **Gate** in lefthook (`delta --staged`) and in the CI `lint` job (`delta` against the merge base, plus a component score comparison base → head with at most 0.01 drop). Suppressions only as file-wide `aihealth: disable=<smell> -- <reason>`, the reason enforced by a lint check.
4. **Agents:** AGENTS.md gets a "Code health" section (run `just code-health --changed` before every commit; fix smells instead of suppressing them); `refactor-sweep` and `refactor-scout` use the score as their target metric; a project skill `code-health-fix`; skills.sh skills after owner approval.
5. **Refactoring** toward 10 in small, tested, behaviour-preserving steps, hotspots first; big restructurings are asked first.

Out of scope: the CodeScene service or token; change coupling and knowledge-distribution metrics (CodeScene's behavioural analysis); a shared implementation for other repositories (dropped by the owner: only the goals are shared, tools are chosen per repository, design §7); ESLint style or correctness rules.

Persona impact (`docs/concept/personas.md`): no end-user persona directly. Parents benefit indirectly through fewer defects in safety code (`link.rs` is the top hotspot). Contributors trade a stricter commit gate for code that agents can change safely.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: code health score, gate and ratchet baseline; the pre-commit hook runs the health check.

## Impact

- New: `tools/code-health/` (component profiles, `aihealth.lock.txt`), `docs/contributing/code-health.md`, `.claude/skills/code-health-fix/`.
- Changed: `justfile` (`code-health` recipe, called from `_lint-code`; `just setup` installs ai-health into `target/code-health/venv` and pre-loads the grammars), `lefthook.yml`, `.github/workflows/ci.yml` (lint job: `fetch-depth: 0`, grammar cache), `docs/ci-gates.md` (`lint` row), root `package.json` + `pnpm-lock.yaml` (eslint, typescript-eslint, eslint-plugin-sonarjs, eslint-plugin-svelte for the Svelte report), `AGENTS.md` (§4, §5.2 ESLint note, new section), `.claude/skills/refactor-sweep/SKILL.md`, `.claude/agents/refactor-scout.md`.
- Code across `crates/`, `apps/android`, `apps/web`, `apps/e2e` during the refactoring phase (separate commits and PRs).
- Supply chain: one young third-party tool (36 Python packages, installed from git with hashes) and its run-time grammar download, pre-loaded at setup (design §0). Maintenance: ai-health updates are manual, because scores change between releases.
