# Design

Layer: build tooling only (`scripts/`, `tools/`, `justfile`, hooks, CI). No product code changes in this design; the refactorings it drives must preserve behaviour. Privacy guardrails: unaffected. Every tool runs locally and offline after installation, none sends telemetry (ESLint, detekt, jscpd, clippy and tree-sitter have none; checked against their docs during implementation, AGENTS.md §7.5).

Sources: CodeScene "Code Health" guide (https://codescene.io/docs/guides/technical/code-health.html), A. Tornhill, "AI-Ready Code" (2026). CodeScene does not publish its weights; every weight below is our own and marked **assumption**. Thresholds follow the CodeScene defaults the owner named (CC > 9, nesting ≥ 4, ≥ 4 logical operators, ≥ 5 arguments); other thresholds are assumptions.

## 1. Architecture

```
just code-health [--json] [--changed [base] | --staged] [--check] [--update-baseline]
  scripts/code-health/cli.mjs
    ├─ files.mjs         file set: git ls-files ∩ include − exclude; --changed/--staged delta
    ├─ analyzers/
    │   clippy.mjs       cargo clippy --message-format=json, CLIPPY_CONF_DIR + CARGO_TARGET_DIR=target/code-health
    │   detekt.mjs       java -jar detekt-cli (mise), SARIF/XML report
    │   eslint.mjs       ESLint Node API, flat config built in memory from config.toml
    │   structure.mjs    web-tree-sitter (Rust, Kotlin, TypeScript grammars)
    │   jscpd.mjs        jscpd Node API, JSON report
    ├─ normalize.mjs     → one record per finding: {file, line, fn, smell, value, threshold, source}
    ├─ score.mjs         findings + file metrics → file score, repo score (pure, unit-tested)
    ├─ baseline.mjs      ratchet compare/update (pure, unit-tested)
    └─ report.mjs        table / JSON
```

All analyzers write into one normalized finding format (`normalize.mjs`). The scorer never sees tool output, so a tool can be swapped without touching the score, and a later shared implementation (§7) only needs the normalized format.

Analyzers run in parallel; clippy runs through `scripts/lib/slot.sh medium` (AGENTS.md §10.1). A missing prerequisite (no host Rust, no Java) prints `SKIPPED: code-health <lang> needs <x>` and, under `GS_STRICT=1`, fails.

### Components (owner input, 2026-11-23)

The checker is not one tool for the whole repo but three **components**, each with its own analyzer pipeline, score and baseline section; they share only the scoring rule, the structure pass, jscpd and the report:

| Component | Paths (config.toml)                         | Pipeline                                  | Prototype score |
| --------- | ------------------------------------------- | ----------------------------------------- | --------------: |
| `rust`    | `crates/**`, `fuzz/**`                      | clippy + structure pass + jscpd           |            5.30 |
| `android` | `apps/android/**`                           | detekt + structure pass + jscpd           |            7.55 |
| `web`     | `apps/web/**`, `apps/e2e/**`, `packages/**` | ESLint + sonarjs + structure pass + jscpd |            8.89 |

- `just code-health --component <name>` runs one pipeline; a missing prerequisite skips only that component.
- The gate applies per component: a component score must not sink, and each component reaches 9.5 on its own (the Rust core can be done first without waiting for Android).
- The repo score is the LOC-weighted mean over all components, so it equals the combination of the component scores; it is reported, not computed by a separate tool.
- A component without findings tools for a smell (for example Svelte templates) says so in the report instead of silently scoring 10.
- New components (a future iOS or desktop part) are a new pipeline plus a config entry.

## 2. Metric sources

| Smell (CodeScene name)              | Rust                               | Kotlin                           | TypeScript / Svelte                      | Threshold (finding when)                                      |
| ----------------------------------- | ---------------------------------- | -------------------------------- | ---------------------------------------- | ------------------------------------------------------------- |
| Complex Method (cyclomatic)         | structure pass                     | detekt `CyclomaticComplexMethod` | ESLint `complexity`                      | CC > 9 (CodeScene)                                            |
| Cognitive complexity                | clippy `cognitive_complexity`      | detekt `CognitiveComplexMethod`  | sonarjs `cognitive-complexity`           | > 15 (assumption, Sonar default)                              |
| Large Method                        | clippy `too_many_lines`            | detekt `LongMethod`              | ESLint `max-lines-per-function`          | > 70 code lines (assumption)                                  |
| Brain Method                        | derived                            | derived                          | derived                                  | Large Method ∧ CC > 15 ∧ nesting ≥ 3 (assumption)             |
| Excess Function Arguments           | clippy `too_many_arguments`        | detekt `LongParameterList`       | ESLint `max-params`                      | ≥ 5 (CodeScene)                                               |
| Excess boolean arguments            | clippy `fn_params_excessive_bools` | structure pass                   | structure pass                           | ≥ 3 bool params (clippy default)                              |
| Primitive Obsession                 | structure pass                     | structure pass                   | structure pass                           | file: ≥ 10 params, ≥ 70 % primitive (assumption)              |
| Deep, Nested Complexity             | structure pass                     | detekt `NestedBlockDepth`        | ESLint `max-depth`                       | control-flow depth ≥ 4 (CodeScene)                            |
| Bumpy Road                          | structure pass                     | structure pass                   | structure pass                           | ≥ 2 separate blocks of depth ≥ 2 in one function (assumption) |
| Complex Conditional                 | structure pass                     | detekt `ComplexCondition`        | sonarjs `expression-complexity`          | ≥ 4 `&&`/`\|\|` in one condition (CodeScene)                  |
| Code Duplication                    | jscpd                              | jscpd                            | jscpd + sonarjs `no-identical-functions` | clone ≥ 60 tokens (assumption)                                |
| Lines of Code in a Single File      | structure pass                     | structure pass                   | ESLint `max-lines`                       | > 600 code lines (assumption)                                 |
| Brain Class / God Class             | derived                            | detekt `LargeClass` + derived    | derived                                  | > 600 lines ∧ ≥ 1 Brain Method ∧ > 30 functions (assumption)  |
| Many functions per module           | structure pass                     | detekt `TooManyFunctions`        | structure pass                           | > 40 per file, > 30 per class (assumption)                    |
| Low Cohesion (LCOM4)                | structure pass (`self.` fields)    | structure pass (approximate)     | structure pass (`this.` fields)          | LCOM4 ≥ 3 (assumption); phase 2, see question 6               |
| Large Assertion Blocks (tests)      | structure pass                     | structure pass                   | structure pass                           | ≥ 6 consecutive assertions (assumption)                       |
| Duplicated Assertion Blocks (tests) | structure pass                     | structure pass                   | structure pass                           | same block of ≥ 3 assertions twice in a file (assumption)     |

Why the structure pass covers some cells a tool could fill:

- clippy has no cyclomatic complexity (the lint was renamed to cognitive complexity), and `excessive_nesting` counts every block, including `mod`, `impl` and `mod tests`, so a test function inside `mod tests` starts at depth 3 (measured: 290 findings, mostly noise). Rust nesting therefore comes from the structure pass, which counts control flow only (`if`, `match`, loops, closures with control flow), the same rule as detekt and ESLint.
- No tool reports Bumpy Road, assertion blocks or primitive obsession.

Tool thresholds are generated from `config.toml`, never written twice. The detekt `ComplexCondition` threshold counts conditions, so "≥ 4 operators" becomes `threshold: 5`; each such mapping has a unit test.

Test code counts. A file is "test code" by path (`/tests/`, `src/test/`, `*.test.ts`, `*.spec.ts`) or, in Rust, inside `#[cfg(test)]`. Test code gets the assertion smells; ESLint's `max-lines-per-function` ignores the anonymous suite callbacks of `describe(...)` (assumption: CodeScene scores the test functions, not the suite wrapper).

Svelte: only the `<script>` part is analyzed (assumption; template nesting is out of scope).

## 3. Score

Each file starts at 10.0. Each finding costs `weight × severity`, where `severity = min(2, 1 + (value − threshold) / threshold)` (1 at the threshold, 2 at double). The cost per smell category is capped, so one smell type alone cannot drive a file to 1. File score = `max(1.0, 10.0 − Σ category costs)`, rounded to 2 decimals.

| Category                            |    Weight | Cap | Note (all assumptions unless stated)                  |
| ----------------------------------- | --------: | --: | ----------------------------------------------------- |
| Brain Method                        |       2.0 | 4.0 | CodeScene treats it as the most severe function smell |
| Brain/God Class                     |       1.5 | 3.0 |                                                       |
| Complex Method                      |       0.6 | 3.0 |                                                       |
| Deep, Nested Complexity             |       0.6 | 2.5 |                                                       |
| Bumpy Road                          |       0.6 | 2.5 |                                                       |
| Large Method                        |       0.5 | 2.0 |                                                       |
| Cognitive complexity                |       0.4 | 2.0 | overlaps with Complex Method, hence lower             |
| Code Duplication (in-file / cross)  | 0.4 / 0.2 | 2.0 | CodeScene's smell is in-file duplication              |
| Complex Conditional                 |       0.3 | 1.5 |                                                       |
| Excess Function Arguments           |       0.3 | 1.5 |                                                       |
| Lines of Code in a Single File      |       0.5 | 2.0 | severity grows per 400 lines over                     |
| Many functions / Low Cohesion       |       0.5 | 1.0 |                                                       |
| Primitive Obsession, bool arguments |       0.3 | 1.0 |                                                       |
| Large / Duplicated Assertion Blocks |       0.3 | 1.5 |                                                       |

Consequence: one Complex Method at the threshold leaves a file at 9.4, so a new file must be practically smell-free to pass 9.5. That matches the owner's aim of 10.

Repo score = LOC-weighted mean of the file scores (code lines without blanks and comments). The report adds per-language subtotals, the colour bands (green ≥ 9.0, yellow 4.0–8.9, red < 4.0) and the worst file.

The weights are calibrated once against the prototype hotspots (design §8) during task 2 and then frozen; later changes to weights are a reviewed change of `config.toml` and regenerate the baseline in the same commit.

## 4. Gate and ratchet

`just code-health --check` (default delta: `--changed origin/main`, i.e. the merge base plus uncommitted and untracked files, the same rule as `just test-affected`):

1. A changed file **not** in the baseline must score ≥ 9.5.
2. A changed file **in** the baseline must not score lower than its baseline entry. If it improved, the check fails with "run `just code-health --update-baseline`" until the entry is tightened (the ratchet only moves up); at ≥ 9.5 the entry is removed.
3. No component score and not the repo score may sink below its baseline value − 0.01. The tolerance absorbs LOC-weight shifts (for example deleting a healthy file). The repo score is computed from the fresh scores of the changed files and the cached scores of all others (`baseline.json` keeps every file's score and LOC), so the check needs no full run.
4. A deleted file leaves the baseline.

`baseline.json` holds one line per file (`{"path": …, "score": …, "nloc": …}`), sorted, so parallel pull requests conflict only when they touch the same file. `repo_score` is tightened by `--update-baseline` in any pull request and at least by the weekly sweep, not required in every pull request (fewer conflicts on one line).

CI (`lint` job, `just lint-code`): runs `--check` against the pull request base and additionally verifies that no baseline entry got worse compared to the base branch's `baseline.json` (nobody can loosen the ratchet by hand). Once the repo reaches 9.5, the baseline is emptied and rule 1 applies to every changed file; the baseline file stays as an empty ratchet.

Hook: see question 3. Recommendation: lefthook **pre-commit** runs `just code-health --staged --check` with the fast analyzers (structure, ESLint, detekt, jscpd on the staged files, about 3–6 s) and clippy only for the crates of staged `.rs` files, reusing `target/code-health`; without host Rust, Rust files are scored by the structure pass alone and the hook says so. CI is authoritative.

## 5. Suppressions

Only by an inline comment on the line above the function or block, with a reason of at least a few words:

```
// code-health: allow(large-method) -- generated state table, splitting hides the protocol order
```

The structure pass parses these comments; a suppression without a reason is itself a finding (weight 1.0). Native suppressions of the health rules (`#[allow(clippy::too_many_lines)]`, `@Suppress("LongMethod")`, `eslint-disable`) count as findings unless the same line carries a `code-health:` reason. The report lists all suppressions, so they stay visible.

## 6. Agent integration

- **AGENTS.md** new §5.5 "Code health": run `just code-health --changed` before every commit; fix smells instead of suppressing them; `--json` gives the findings with line numbers; read `docs/contributing/code-health.md` for the fix per smell.
- **Skill `code-health-fix`** (`.claude/skills/code-health-fix/SKILL.md`, own, short): loop "run `--json` → pick the worst finding → apply the playbook move for that smell (extract function, guard clause, table/match instead of if-chains, parameter object, split file) → run the affected tests → re-score → commit". Playbooks partly adapted from the reviewed skills.sh skills (§9).
- **Refactor sweep** (proposal for `.claude/skills/refactor-sweep/SKILL.md` and the scheduled task):
  1. Prepare: run `just code-health --json` in the scout worktree; save repo score, per-language scores and the top 10 hotspots (ranked by `(10 − score) × (commits in 90 days + 1)`).
  2. Scout: receives the hotspot list as priority input. At least 2 of the up to 5 findings target hotspot files; each finding names the expected score change (`Health: <file> 6.4 → ≥ 8.0`).
  3. Implement/verify: acceptance always includes `just code-health --changed origin/main --check` and the named file score target; a finding that lowers any file score is discarded.
  4. PR body: score before/after for the touched files and the repo.
  5. Report: repo score trend since the last sweep. If the repo score rose, one additional draft PR `chore(code-health): tighten baseline` with `--update-baseline` (counts toward the 3-PR cap).
  6. Scheduled task prompt: replace the stale `pnpm run lint` / `pnpm test` with `just lint` / `just test`, and re-enable it (it is disabled since 2026-11-09) only after the owner agrees.
- **refactor-scout**: new category `health`, and in diff mode a line with the score delta of the changed files.

## 7. Shared implementation across repositories (option, not built)

The same standard is wanted in dotfiles, homelab, GuardedSleep, narratty, repo-env and looping_louie.

- [ ] **A. Per repo (this change).** Pros: no cross-repo coupling; fits each repo's toolchain (Node here). Cons: six copies of the scorer drift apart.
- [ ] **B. Shared scorer in `github.com/ditschi/pre-commit-hooks`** consuming the normalized finding format (§1); each repo keeps only its analyzer adapters and `config.toml`. Pros: one score definition, one test suite; the format is already tool-agnostic. Cons: a second release process; the hooks repo is probably Python, so the scorer would be ported or run through `npx`/`uvx`.
- [x] **Recommendation:** A now, with the normalized format kept stable and documented, so B is a mechanical extraction once two repos use it. Decide B after the second repo.

## 8. Prototype measurement (2026-11-23, `origin/main` 7e62a798)

Prototype: lizard 1.26.1 (CC, NLOC, parameters, nesting) + jscpd 4.0.5, scored with a draft of §3 (no Bumpy Road, no assertion smells, no cognitive complexity). Not the final checker; lizard counts Compose lambda blocks as nesting, so Kotlin nesting is inflated (detekt: 2 `NestedBlockDepth` findings, lizard: 77).

| Scope      | Score | Code lines | Files | < 9.5 | green / yellow / red | Worst file                                     |
| ---------- | ----: | ---------: | ----: | ----: | -------------------- | ---------------------------------------------- |
| Rust       |  5.30 |     30 862 |    71 |    37 | 35 / 27 / 9          | `crates/guardedsleep-core/src/envelope.rs` 1.0 |
| Kotlin     |  7.55 |     22 703 |   173 |    60 | 133 / 31 / 9         | `…/ui/screens/AlertScreens.kt` 1.0             |
| TypeScript |  8.89 |     19 979 |   150 |    35 | 132 / 18 / 0         | `apps/e2e/tests/utils/layout.ts` 5.0           |
| **Total**  |  6.97 |     73 544 |   394 |   132 | 300 / 76 / 18        |                                                |

Top hotspots by `(10 − score) × (commits in 90 days + 1)`:

| #   | File                                                 | Score | Commits 90 d | Main smells                                          |
| --- | ---------------------------------------------------- | ----: | -----------: | ---------------------------------------------------- |
| 1   | `crates/guardedsleep-core/src/link.rs` (3 893 lines) |   1.0 |           33 | 16 complex, 5 brain methods, 15 long parameter lists |
| 2   | `apps/android/platform/…/link/LinkController.kt`     |   2.8 |           31 | 4 complex, large file, many functions                |
| 3   | `crates/guardedsleep-sim/src/world.rs`               |   1.0 |           19 | 10 complex, 3 brain methods                          |
| 4   | `apps/android/app/…/screens/ParentMonitorScreen.kt`  |   1.0 |           14 | 5 complex, brain method, nesting                     |
| 5   | `crates/guardedsleep-core/src/protocol.rs`           |   1.0 |           14 | 7 complex, 2 brain methods, large file               |
| 6   | `crates/guardedsleep-core/src/conformance.rs`        |   3.5 |           16 | 3 complex, brain method                              |
| 7   | `crates/guardedsleep-core/src/envelope.rs`           |   1.0 |           11 | 9 long parameter lists, brain method                 |
| 8   | `apps/android/app/…/screens/AlertScreens.kt`         |   1.0 |           10 | 2 brain methods, duplication                         |
| 9   | `crates/guardedsleep-core/tests/common/mod.rs`       |   2.7 |           12 | 5 complex, brain method                              |
| 10  | `apps/web/src/lib/platform/web-controller.ts`        |   6.7 |           24 | 2 complex, large file, many functions                |

Smell counts: 162 Complex Method, 108 Excess Arguments, 77 nesting (inflated), 75 duplication, 54 Large Method, 39 many functions, 33 Brain Method, 26 large files.

Tool checks on the same tree: detekt 1.23.8 parses all Kotlin 2.4 sources (5 s, 259 complexity findings); ESLint 10.15 + typescript-eslint 8.74 + sonarjs 4.2.2 + eslint-plugin-svelte 3.24 cover all `.ts` and `.svelte` files (5 s); clippy with the health lints runs in 60 s cold on `target/code-health`.

## 9. Skills

skills.sh candidates were reviewed by reading their SKILL.md (2026-11-23); the shortlist goes to the owner, nothing is installed before approval, and approved skills are installed project-locally under `.claude/skills/`. `*@codehealth-mcp` is excluded (paid token).

## 10. Refactoring order

1. Calibrate and freeze weights; write the baseline.
2. Hotspots in the order of §8, each in its own commit series: first move inline Rust test modules that dominate a file into `tests/` or sibling files only where the crate already does so (no behaviour change), then extract functions, parameter objects and match tables. Safety-logic files (`link.rs`, `protocol.rs`, `envelope.rs`, `session.rs`) keep their vectors and traces unchanged; a trace diff means the refactoring changed behaviour and is reverted.
3. Compose screens: extract sub-composables with stable `testTag`s; Roborazzi baselines must stay byte-identical (they are never re-recorded for a refactoring).
4. After each step: `just test-affected`, `just code-health --changed --check`; at the end of each work package `just lint` and `just test`.

Splitting `link.rs` or `LinkController.kt` into modules is a large restructuring and is put to the owner before it starts.

## Discarded alternatives

- **CodeScene score with a paid token, or the `codehealth-mcp` skills.** Pros: the original metric, calibrated on real defect data. Cons: proprietary, paid, network service. Rejected by the owner (2026-11-23).
- **lizard alone for all languages.** Pros: one tool (MIT), fast, used for the prototype. Cons: no Bumpy Road, no cognitive complexity, counts Compose lambdas as nesting; the owner wants the language-native analyzers. Kept as prototype only.
- **rust-code-analysis-cli for Rust.** Pros: Mozilla, reports cyclomatic and cognitive complexity per function. Cons: last release 0.0.25 on 2023-01-13, with a 2023 tree-sitter-rust grammar that predates edition 2024 syntax used here; unmaintained. Rejected in favour of clippy plus the structure pass.
- **clippy `excessive_nesting` for Rust nesting.** Cons: counts `mod`/`impl`/`mod tests` blocks, 290 noisy findings on the current tree. Replaced by the structure pass.
- **detekt 2.0 alpha.** Pros: newer Kotlin support. Cons: pre-release; 1.23.8 already parses the Kotlin 2.4 sources. Revisit when 2.0 is stable.
- **detekt through the Gradle plugin.** Cons: needs the Android build image and Gradle configuration time (tens of seconds); the CLI jar runs on the host Java from mise in 5 s.
- **ESLint as a general linter.** Cons: duplicates `svelte-check` and Prettier, and was rejected in ADR-020. Only the health rules are configured, in memory, from `config.toml`.
- **Full repo run on every check.** Cons: about 1–2 min and every PR would rewrite `repo_score`; the cached per-file scores in the baseline give the same answer for the delta.
- **Score written to every PR's baseline (`repo_score` always current).** Cons: one line conflicts in every parallel PR; tightened by `--update-baseline` and the weekly sweep instead.
