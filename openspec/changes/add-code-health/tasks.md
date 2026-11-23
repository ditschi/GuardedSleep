# Tasks

Branch `feat/code-health`. Each implementation task is a brief for the `implementer` sub-agent. Layer rule: build tooling only (`scripts/`, `tools/`, `justfile`, hooks, CI, docs, `.claude/`); refactoring tasks (group 6) change product code without changing behaviour. Every version-specific setting carries a comment with a doc link (AGENTS.md §5.2); dependencies are added through pnpm and mise, never with invented versions.

## 1. Tools and pins

- [ ] 1.1 **Pins.** Goal: root devDependencies `eslint`, `typescript-eslint`, `eslint-plugin-sonarjs`, `eslint-plugin-svelte`, `jscpd`, `web-tree-sitter`, `tree-sitter-rust`, `tree-sitter-kotlin`, `tree-sitter-typescript`, `smol-toml` via `pnpm add -Dw` (latest stable, release age 3 days); `detekt-cli` 1.23.8 `-all` jar via the mise `http:` backend like ktfmt, plus `mise lock`. Renovate group "code-health tools". Files: `package.json`, `pnpm-lock.yaml`, `mise.toml`, `mise.lock`, `renovate.json`. Acceptance: `just setup` on a clean clone; `pnpm audit` clean; no install script needs `allowBuilds`.

## 2. Checker

- [ ] 2.1 **Config.** `tools/code-health/config.toml` with includes/excludes (generated bindings, `*.d.ts`, `build/`, `fuzz/`), thresholds, weights and caps from design §2–§3, every value commented, own choices marked `assumption`. Acceptance: taplo fmt/lint.
- [ ] 2.2 **Analyzers + normalize.** The structure pass waits for the ai-health trial (design §2, on hold); build clippy, detekt, ESLint first. `scripts/code-health/analyzers/{clippy,detekt,eslint,structure,jscpd}.mjs` and `normalize.mjs` (design §1–§2); tool configs generated from `config.toml` into `target/code-health/`. Tests (Vitest) with small fixture files per language and smell, including the detekt `ComplexCondition` threshold mapping and the clippy JSON parsing. Acceptance: `pnpm vitest run scripts/code-health`.
- [ ] 2.3 **Score, baseline, report, CLI.** `score.mjs`, `baseline.mjs`, `report.mjs`, `cli.mjs` (design §3–§5); `--json`, `--changed [base]`, `--staged`, `--check`, `--update-baseline`. Pure functions unit-tested (cap, minimum 1.0, LOC weighting, ratchet rules 1–4, suppression parsing). Acceptance: tests green; `just code-health` prints the table in under 2 min cold.
- [ ] 2.4 **Calibrate.** Run on `origin/main`, compare with design §8, adjust weights once, freeze; write `tools/code-health/baseline.json`. Record the measured repo score in the PR.

## 3. Gate

- [ ] 3.1 `justfile`: `code-health` recipe (medium slot), `_lint-code` runs `code-health --check` against the base plus the "no baseline entry lowered" check. `lefthook.yml`: pre-commit job `code-health` on staged sources (design §4, question 3). `.github/workflows/ci.yml`: the `lint` job has what it needs (Java from mise). Acceptance: `just lint`, `just lint-workflows`; a scratch commit with a deeply nested function fails the hook and `just lint-code`.
- [ ] 3.2 `docs/ci-gates.md`: name the check in the `lint` row with a runbook. Acceptance: `just lint-text`.

## 4. Docs and agents

- [ ] 4.1 `docs/contributing/code-health.md` (score, smells, thresholds, how to fix each smell, suppressions, baseline); AGENTS.md: §4 command row, §5.2 ESLint note (metric source only), new §5.5 "Code health" (run `just code-health --changed` before every commit; fix, do not suppress). Acceptance: `just lint-text`.
- [ ] 4.2 `.claude/skills/code-health-fix/SKILL.md` (design §6). Acceptance: an agent run on one hotspot follows it end to end.
- [ ] 4.3 Refactor sweep: update `.claude/skills/refactor-sweep/SKILL.md` and `.claude/agents/refactor-scout.md` per design §6; propose the scheduled-task prompt change to the owner (not applied without approval).
- [ ] 4.4 Install the skills.sh skills the owner approved, project-locally, and reference them from `code-health-fix`.

## 5. Final checks of the tooling PR

- [ ] 5.1 `just lint` and `just test` green; report any SKIPPED task; `openspec validate --all --strict`.

## 6. Refactoring toward ≥ 9.5 (one commit per step, separate PRs per area)

- [ ] 6.1 Rust core hotspots (`link.rs`, `protocol.rs`, `envelope.rs`, `conformance.rs`, `session.rs`, `tests/common/mod.rs`): vectors and traces unchanged; `scripts/cargo.sh test`, `just test-affected`, `just code-health --changed --check` per step. Splitting `link.rs` into modules only after owner approval.
- [ ] 6.2 Simulation (`guardedsleep-sim/src/world.rs`), relay, server, i18n-gen, public-lists.
- [ ] 6.3 Android platform (`LinkController.kt` and the rest of `platform`): JVM tests green.
- [ ] 6.4 Android UI screens (`ParentMonitorScreen.kt`, `AlertScreens.kt`, `BabyScreens.kt`, `StatusBar.kt`, `TalkWidgets.kt`): Roborazzi baselines unchanged, `testTag`s unchanged.
- [ ] 6.5 Web client and E2E helpers (`web-controller.ts`, `apps/e2e/tests/utils/layout.ts`): Vitest and Playwright green.
- [ ] 6.6 When the repo reaches ≥ 9.5: empty the baseline, report before/after scores and remaining smells; archive with `/opsx:archive`.
