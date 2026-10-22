# Spec Delta

## ADDED Requirements

### Requirement: Delta tests

`just test-affected [base]` SHALL run the test recipes of the areas touched by the changes since the merge base with `base` (default `origin/main`), including uncommitted and untracked files, using the same path map as CI. A change to a global path, or to a path that matches no area, SHALL run the full `just test`. It SHALL name the heavy checks the change needs in the full sweep.

#### Scenario: Web-only change

- **WHEN** only a file under `apps/mobile/src/` changed
- **THEN** `just test-affected` runs the affected Vitest tests, skips the Rust and Android tests, and names `just e2e` for the full sweep

#### Scenario: Unmapped path

- **WHEN** a file in a new top-level directory changed
- **THEN** `just test-affected` runs the full `just test` and says why

### Requirement: Run timings

Every slot-held run SHALL append its command, class, wait time, run time and exit status to a timings file shared by all worktrees, and `just timings` SHALL summarise it. Recording SHALL never change a run's exit status.

#### Scenario: Timings file not writable

- **WHEN** the timings file cannot be written
- **THEN** the run's result is unchanged

### Requirement: Worktree cleanup

`just reap --worktrees` SHALL list worktrees that are clean and whose HEAD is contained in a remote branch, and `just reap --prune-worktrees` SHALL remove only those, never with force and never one locked by a running process.

#### Scenario: Worktree with uncommitted changes

- **WHEN** a worktree has an uncommitted change
- **THEN** it is listed as kept and is not removed
