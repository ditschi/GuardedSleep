# Spec Delta

## ADDED Requirements

### Requirement: Code health score

`just code-health` SHALL score every tracked Rust, Kotlin, TypeScript and Svelte source file from 10.0 down to 1.0 by one scoring rule configured in one versioned, commented file. Each file SHALL belong to one component (`rust`, `android`, `web`) with its own analyzer and score: ai-health with a pinned version and a per-component profile for Rust and TypeScript, and a separately decided analyzer for Kotlin; each smell SHALL have exactly one source.

#### Scenario: Complex function

- **WHEN** a Kotlin function has a cyclomatic complexity of 12
- **THEN** the report lists a Complex Method finding with its line, and the file score is below 10.0

#### Scenario: One component only

- **WHEN** a contributor runs `just code-health --component android`
- **THEN** only the Android pipeline runs and the report shows the Android score and its files

#### Scenario: Missing analyzer prerequisite

- **WHEN** Java is not installed and `just code-health` runs locally
- **THEN** it prints `SKIPPED: code-health android needs java`, scores the other components, and under `GS_STRICT=1` it fails

### Requirement: Code health per component

Each component SHALL reach a score of 9.5 on its own. A repo total, if reported, SHALL be the mean of all file scores weighted by code lines and SHALL NOT be a gate.

#### Scenario: Healthy component hides nothing

- **WHEN** the `web` component scores 9.8 and the `rust` component 9.1
- **THEN** the report shows both, and `rust` stays below target regardless of the repo total

### Requirement: Code health report

The report SHALL list each file with score and smells (smell, line, value, threshold), the component scores, the repo score and the worst file, as a table by default and as JSON with `--json`; `--changed [base]` and `--staged` SHALL limit it to changed files.

#### Scenario: Agent reads findings

- **WHEN** an agent runs `just code-health --changed --json`
- **THEN** it gets each changed file with score and findings as JSON, plus the component scores and the repo total

### Requirement: Code health gate

`just code-health --check` SHALL fail when a new or changed file not in the ratchet baseline scores below 9.5, when a baseline file scores lower than its entry, or when a component score drops more than 0.01 below its baseline value. It SHALL run in the CI `lint` job.

#### Scenario: New unhealthy file

- **WHEN** a pull request adds a file that scores 9.2
- **THEN** the `lint` job fails and names the file and its findings

#### Scenario: Hotspot gets worse

- **WHEN** a change lowers the score of a baseline file from 4.10 to 3.90
- **THEN** `just code-health --check` fails

### Requirement: Code health ratchet baseline

While a component scores below 9.5, `tools/code-health/baseline.json` SHALL record its file and component scores. Entries SHALL only move up; CI SHALL fail when a pull request lowers an entry.

#### Scenario: Hotspot improves

- **WHEN** a change raises a baseline file from 4.10 to 6.00 without updating the baseline
- **THEN** the check fails and asks for `just code-health --update-baseline`, which raises the entry to 6.00

#### Scenario: Baseline loosened by hand

- **WHEN** a pull request edits `baseline.json` to lower an entry
- **THEN** the `lint` job fails

### Requirement: Code health suppressions

A smell SHALL be suppressed only by an inline `code-health:` comment with a reason. A suppression without a reason, or a native suppression of a health rule without such a comment, SHALL count as a finding, and the report SHALL list every suppression.

#### Scenario: Suppression without reason

- **WHEN** a function carries `@Suppress("LongMethod")` without a `code-health:` reason
- **THEN** the suppression itself is reported as a finding

## MODIFIED Requirements

### Requirement: Pre-commit hooks

`just setup` SHALL install lefthook pre-commit hooks that format staged files and run typos, whitespace and JSON hygiene, a secret scan of staged changes and the code health check of staged source files. There SHALL be no commit-msg hook; hooks are fast feedback and CI re-runs everything. Outside CI, `just lint` SHALL warn, without failing, when the hooks are not active.

#### Scenario: Unformatted Markdown

- **WHEN** a contributor commits an unformatted Markdown file
- **THEN** the hook formats it and stages the result

#### Scenario: Hooks not active

- **WHEN** the lefthook pre-commit hook is not installed and a contributor runs `just lint` locally
- **THEN** `just lint` prints a warning that names `just setup`, and its verdict is unchanged

#### Scenario: Unhealthy staged file

- **WHEN** a contributor commits a new TypeScript file with a function nested four levels deep
- **THEN** the hook fails and names the file, the line and the smell
