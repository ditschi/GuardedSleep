# Tasks

Branch `feat/code-health`. Tasks follow design §0 (ai-health); the original plan in §1–§3 is superseded for Rust and TypeScript. Each implementation task is a brief for the `implementer` sub-agent. Layer rule: build tooling only (`scripts/`, `tools/`, `justfile`, hooks, CI, docs, `.claude/`); refactoring tasks (group 6) change product code without changing behaviour. Every version-specific setting carries a comment with a doc link (AGENTS.md §5.2); dependencies are added through pnpm and mise, never with invented versions.

## 1. Tools and pins

- [x] 1.1 **ai-health pin.** Needs the owner's confirmation in this session first (design §0). `tools/code-health/aihealth.lock.txt` from `uv pip compile --generate-hashes` of `git+https://github.com/epatel/ai-health@033d1867fe514867d944d9387ee72db0188eed47`; `just setup` installs it into `target/code-health/venv` with `uv pip sync --require-hashes`. Check whether grammars are downloaded at run time and from where; the gate must work offline after setup. Acceptance: `just setup` on a clean clone; `target/code-health/venv/bin/aihealth --version` prints 0.4.0.
- [ ] 1.2 (Kotlin done 2026-11-28: ai-health v0.7.1, `tools/code-health/android.toml`) **Svelte:** ESLint + eslint-plugin-svelte + sonarjs as root devDependencies, a report without score (owner decision). **Kotlin:** via ai-health once the Kotlin adapter is available (design §0); until then the `android` component is reported as "not yet scored". detekt only as a transitional linter if it is nearly free; no own score aggregator, no tree-sitter check.

## 2. Profiles

- [x] 2.1 `tools/code-health/{rust,web}.toml` with the "strict" profile (design §0), each value commented, paths per component; `android.toml` (`large_file = 600`, done 2026-11-28 on upstream v0.7.1); test paths via `[[overrides]]`. Acceptance: `aihealth scan crates --config tools/code-health/rust.toml` reproduces the trial (about 4.23).

## 3. Gate

- [x] 3.1 `justfile`: `code-health` recipe, always with `--config` (design §0, usage rules) (scan report per component; `--changed [base]` → `aihealth delta --base`; `--staged`; `--json` → `--format json`); `_lint-code` runs the delta against `git merge-base origin/main HEAD` per component, plus the suppression-reason regex check; CI checkout uses `fetch-depth: 0`. `lefthook.yml`: pre-commit `aihealth delta --staged` per component. `.github/workflows/ci.yml`: cache `~/.cache/tree-sitter-language-pack` if grammars are downloaded. Acceptance: `just lint`, `just lint-workflows`; a scratch commit with a function nested four levels deep fails the hook and `just lint-code`.
- [x] 3.2 CI compares each component score between merge base and head (no drop > 0.01; owner decision, design §0). `docs/ci-gates.md`: name the check in the `lint` row with a runbook. Acceptance: `just lint-text`.

## 4. Docs and agents

- [x] 4.1 `docs/contributing/code-health.md` (score, smells, thresholds, how to fix each smell, suppressions, baseline, the Steckbrief from design §11); AGENTS.md: §4 command row, new §5.5 "Code health" (run `just code-health --changed` before every commit; fix, do not suppress). Acceptance: `just lint-text`.
- [ ] 4.2 `.claude/skills/code-health-fix/SKILL.md` (design §6). Acceptance: an agent run on one hotspot follows it end to end.
- [ ] 4.3 Refactor sweep: update `.claude/skills/refactor-sweep/SKILL.md` and `.claude/agents/refactor-scout.md` per design §6; propose the scheduled-task prompt change to the owner (not applied without approval).
- [ ] 4.4 Install the skills.sh skills the owner approved, project-locally, and reference them from `code-health-fix`.

## 5. Final checks of the tooling PR

- [ ] 5.1 `just lint` and `just test` green; report any SKIPPED task; `openspec validate --all --strict`.

## 6. Refactoring toward 10 (floor 9.5) (one commit per step, separate PRs per area)

- [ ] 6.1 Rust core hotspots (`link.rs`, `protocol.rs`, `envelope.rs`, `conformance.rs`, `session.rs`, `tests/common/mod.rs`): vectors and traces unchanged; `scripts/cargo.sh test`, `just test-affected`, `just code-health --changed --check` per step. Splitting `link.rs` into modules only after owner approval.
- [ ] 6.2 (`world.rs` done: `world/` has 21 files, all 10.0 except `synth.rs` 9.06) Simulation (`guardedsleep-sim/src/world.rs`), relay, server, i18n-gen, public-lists.
- [ ] 6.3 Android platform (`LinkController.kt` and the rest of `platform`): JVM tests green.
- [ ] 6.4 Android UI screens (`ParentMonitorScreen.kt`, `AlertScreens.kt`, `BabyScreens.kt`, `StatusBar.kt`, `TalkWidgets.kt`): Roborazzi baselines unchanged, `testTag`s unchanged.
- [ ] 6.5 Web client and E2E helpers (`web-controller.ts`, `apps/e2e/tests/utils/layout.ts`): Vitest and Playwright green.
- [ ] 6.6 When the repo reaches ≥ 9.5: empty the baseline, report before/after scores and remaining smells; archive with `/opsx:archive`.
