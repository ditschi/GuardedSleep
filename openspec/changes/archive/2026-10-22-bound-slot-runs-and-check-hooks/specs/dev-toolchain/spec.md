# Spec Delta

## MODIFIED Requirements

### Requirement: Git hooks

`just setup` SHALL install lefthook pre-commit hooks that format staged files and run typos, whitespace and JSON hygiene and a secret scan of staged changes. There SHALL be no commit-msg hook; hooks are fast feedback and CI re-runs everything. Outside CI, `just lint` SHALL warn, without failing, when the hooks are not active: a stale Husky `core.hooksPath` or a missing or non-lefthook `pre-commit` hook.

#### Scenario: Unformatted Markdown

- **WHEN** a contributor commits an unformatted Markdown file
- **THEN** the hook formats it and stages the result

#### Scenario: Stale Husky hooks path

- **WHEN** `core.hooksPath` still points at `.husky/_` and a contributor runs `just lint` locally
- **THEN** `just lint` prints a warning that names `just setup`, and its verdict is unchanged

## ADDED Requirements

### Requirement: Load slots

Medium and heavy recipes SHALL run through a machine-wide slot semaphore shared by all worktrees. When `GS_RUN_TIMEOUT` is set, the run deadline SHALL start only once the slot is held, and a run that exceeds it SHALL end with status 124; a job that gets no slot within `GS_SLOT_WAIT` SHALL end with status 75.

#### Scenario: Queued job with a run deadline

- **WHEN** `GS_RUN_TIMEOUT=600 just test` waits 20 minutes for a medium slot and then runs for 5 minutes
- **THEN** it completes with the test verdict and is not cut off by the deadline

#### Scenario: Run exceeds the deadline

- **WHEN** a slot-held run takes longer than `GS_RUN_TIMEOUT`
- **THEN** it is terminated and exits with status 124 and a message naming `GS_RUN_TIMEOUT`
