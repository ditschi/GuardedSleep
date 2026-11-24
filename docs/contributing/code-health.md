# Code health

GuardedSleep measures code structure with [ai-health](https://github.com/epatel/ai-health) (MIT, deterministic, no AI in the scoring), pinned to v0.4.0 = commit `033d1867fe514867d944d9387ee72db0188eed47`. A score runs from 0 to 10; **the goal is 10 (no finding), 9.5 is only the floor per component** on the way there. Design and decisions: `openspec/changes/add-code-health/design.md` §0.

## What is measured

| Component | Paths                | Score today                               | Notes                                                                                                                   |
| --------- | -------------------- | ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `rust`    | `crates/`            | 4.66 (was 4.04 before the `world/` split) | Inline `#[cfg(test)]` modules count like production code (`score_test_modules`)                                         |
| `web`     | `apps/`, `packages/` | 7.36 (LoC-weighted over both paths)       | TypeScript and JavaScript; Svelte templates and `<script>` blocks are not scored; `android/*`, `pkg/`, `dist/` excluded |
| `android` | `apps/android/`      | not yet scored (Kotlin adapter pending)   | Waits for a Kotlin adapter in ai-health; nothing is built for Kotlin here                                               |

Tests count: test code is scored like production code. Generated output, `target/`, `node_modules/` and shell scripts are out of scope.

## Profile "strict"

One file per component, `tools/code-health/rust.toml` and `tools/code-health/web.toml`, each value commented there. Every call passes `--config`; without it ai-health silently reads `<path>/.aihealth.toml` or its defaults.

| Key                              | Value  | Meaning                                                          |
| -------------------------------- | ------ | ---------------------------------------------------------------- |
| `thresholds.large_file`          | 1000   | Code lines per file before Large File (CodeScene default)        |
| `thresholds.primitive_obsession` | 0.3    | Share of primitive-typed parameters per file (CodeScene default) |
| `score.decay`                    | 8      | Score falls faster with severity than the default 15             |
| `score.symbol_cap`               | 4      | ai-health default, kept explicit                                 |
| `score.density_loc`              | 100000 | No relief for large files through finding density                |
| `delta.tolerance`                | 0.0    | Any score drop of a changed file fails the delta                 |
| `rust.score_test_modules`        | true   | `rust` only: `#[cfg(test)]` modules are scored                   |

All other thresholds are ai-health's defaults, which follow CodeScene: cyclomatic complexity 9, cognitive complexity 15, Large Method 70 lines, 4 arguments, nesting depth 3, Bumpy Road, Complex Conditional, duplication across files.

## Commands

| Command                             | Effect                                                                                                                                                             |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `just setup`                        | Builds the venv `target/code-health/venv` (hash-locked `uv pip sync --require-hashes`, ai-health from its commit pin) and pre-loads the grammars; skips without uv |
| `just code-health`                  | ai-health score per scanned path (`crates`, `apps`, `packages`, each with the tool's own project score) plus the `android` line                                    |
| `just code-health --component rust` | One component (`rust` = `crates`, `web` = `apps` and `packages`)                                                                                                   |
| `just code-health --json`           | `aihealth scan --format json`, one JSON object per path (a single object with `--component rust`)                                                                  |
| `just code-health --changed [base]` | `aihealth delta` since the merge base with `base` (default `origin/main`), uncommitted files included; exits non-zero on degradation                               |
| `just code-health --staged`         | `aihealth delta --staged` (what the hook runs)                                                                                                                     |

The gate is `just lint-code` (recipe `_code-health-gate`): `aihealth delta` per path against `git merge-base origin/main HEAD` (tolerance 0 from the profiles), then the project score of each path must not fall more than 0.01 below its score at the merge base (`delta` reports file scores only, so the base tree is extracted with `git archive` and scanned), then the suppression check. The lefthook jobs `code-health` (staged delta, skips quietly before `just setup`) and `code-health-suppressions` run the same pieces. CI needs `fetch-depth: 0` for the merge base and caches `~/.cache/tree-sitter-language-pack`; the grammars are downloaded once at setup (tree-sitter-language-pack checks their SHA-256) and everything runs offline afterwards.

Updating ai-health is a manual step (Renovate cannot bump a git commit): change the commit in `tools/code-health/aihealth.in`, recompile the lock with `uv pip compile --generate-hashes`, read the upstream changelog, because scores move between releases.

## Suppressions

Only file-wide, with a reason, as a comment in the file:

```
// aihealth: disable=<smell> -- <reason of at least 10 characters>
```

Without the `--` separator ai-health silently does not apply the suppression, so the check in `just lint-code` and lefthook fails on any marker without a reason of 10 characters or more. Prefer fixing the smell; a suppression is for generated tables and the like, never for "too much work".

## Fixing each smell

- **Large File:** split by responsibility into modules (a directory with `mod.rs` or `index.ts`); keep public paths stable with re-exports.
- **Large Method, Brain Method:** extract named steps; a method that is large, complex and nested needs a split first, then the rest.
- **Complex Method, High Cognitive Complexity:** early returns and guard clauses, table-driven `match`, one decision per function.
- **Deep Nesting, Bumpy Road:** flatten with guard clauses, move the inner block into a function; a bumpy road is several nested chunks in one function, so one function per chunk.
- **Complex Conditional:** name the condition (`fn is_...`), one operator pair per expression.
- **Excess Arguments:** group related parameters into a struct or object.
- **Primitive Obsession:** newtypes or small structs for ids, durations, keys; typed enums instead of strings.
- **Code Duplication:** extract the shared logic; in tests a helper or builder, also across crates and files.
- **Low Cohesion:** split the type into parts that share state.

Never dodge a finding without fixing its cause: do not remove or weaken type annotations (no `any`, no dropped types), add purely cosmetic type aliases, or move code into other files or modules only to fall below a check's minimum (for example fewer than 10 typed parameters so Primitive Obsession is skipped). A real fix is a parameter object, newtype or struct that improves the code; otherwise the finding stays for now, with a reasoned file-wide suppression.

Behaviour stays unchanged in refactorings: conformance vectors and traces must not change, and the tests of the area run per step.

## Steckbrief Code Health

Fixed outline for the later comparison across the owner's six repositories.

### 1. Messumfang

Tracked `.rs`, `.kt`, `.ts` and the `<script>` part of `.svelte` files, test code included (with the test smells). Not counted: generated bindings, `*.d.ts`, `build/`, `target/`, `node_modules/`, generated i18n output, shell and `.mjs` tooling scripts (small, their own lint covers them), Svelte templates (no suitable analyzer). Split into three components with their own score and gate: `rust` (`crates/`, `fuzz/`), `android` (`apps/android/`), `web` (`apps/web`, `apps/e2e`, `packages/`). Target ≥ 9.5 per component; the repo total is informational.

### 2. Tools und Versionen, je Sprache

Rust and TypeScript: ai-health v0.4.0 (commit `033d1867fe514867d944d9387ee72db0188eed47`), hash-locked dependencies (`tools/code-health/aihealth.lock.txt`); clippy and ESLint stay linters only. Kotlin: ai-health with a Kotlin adapter (in progress, fork `ditschi/ai-health`); until then not scored, detekt at most as a transitional linter. Svelte: ESLint + sonarjs as a report (open). Grammars: tree-sitter-language-pack 1.25.2, downloaded once at setup.

### 3. Abgebildete Smells

Complex Method, Cognitive Complexity, Large Method, Brain Method, Excess Function Arguments, Primitive Obsession, Deep Nesting, Bumpy Road, Complex Conditional, Code Duplication (across files), Large File, Low Cohesion; assertion smells of test code are not separate findings. Not mapped: Svelte template complexity, CodeScene's behavioural metrics (change coupling, knowledge distribution).

### 4. Schwellen, Gewichte und Score-Formel

Profile "strict" (see above) on ai-health's CodeScene-aligned defaults; weights and score curve are ai-health's (`aihealth/config.py`), with `decay = 8`, `symbol_cap = 4` and no density relief. Score is LoC-weighted per file and per component.

### 5. Bedienung

`just code-health [--component c] [--json] [--changed [base] | --staged]`. Text by default, JSON for agents. A full scan takes about 25 s, the delta a few seconds.

### 6. Gate

`just lint-code` (CI job `lint`): delta against the merge base per component, project score per path no lower than 0.01 below the merge base, suppression check; lefthook runs the staged delta. Suppressions only file-wide with a reason.

### 7. Ergebnis

Before: prototype rust 5.30, android 7.55, web 8.89, total 6.97; ai-health trial rust 6.52 (default curve) / 4.23 (strict), TS in `apps/` 8.57, `packages/` 9.82. Rust before 4.04, after 4.66 after the `world.rs` refactor: `guardedsleep-sim/src/world/` is 21 files that all score 10.0 except `synth.rs` (9.06, one cross-crate duplicate with `guardedsleep-core/tests/common/mod.rs` `colon_fp`). Web: `apps` 7.30, `packages` 9.67 (7.36 LoC-weighted over both). Android: not yet scored. Remaining hotspots are listed by `just code-health` (`link.rs` is the worst file at 1.02).

### 8. Skills

Reviewed: see the design (§9) and the owner shortlist. Adopted: open (owner decision pending).

### 9. Erfahrungen

ai-health replaced the own structure pass before it was built; its young age and unneeded web/MCP dependencies are the price. Always pass `--config` (a missing profile falls back to defaults silently), and a suppression without `--` is silently ignored. Earlier: rust-code-analysis is unmaintained (2023) and clippy's `excessive_nesting` counts `mod`/`impl` blocks; lizard counts Compose lambdas as nesting. Transferable: component split, suppression only with a reason, score comparison against the merge base. Project-specific: three ecosystems in one repo, safety-core traces as the refactoring safety net.
