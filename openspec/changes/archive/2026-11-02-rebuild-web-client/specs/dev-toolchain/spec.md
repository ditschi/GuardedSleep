# Spec Delta

## ADDED Requirements

### Requirement: Web checks

The web client SHALL pass `svelte-check --fail-on-warnings` (a suppression needs an inline reason), Vitest for logic, and Playwright tests with fake media devices, axe and an overflow check. pnpm SHALL run with an install-script allow-list, a minimum release age of 3 days, blocked exotic sub-dependencies and a frozen lockfile in CI.

#### Scenario: Accessibility regression

- **WHEN** a change removes the label of a button on the web parent screen
- **THEN** the Playwright axe check fails

## MODIFIED Requirements

### Requirement: Delta tests

`just test-affected [base]` SHALL run the test recipes of the areas touched by the changes since the merge base with `base` (default `origin/main`), including uncommitted and untracked files, using the same path map as CI. A change to a global path, or to a path that matches no area, SHALL run the full `just test`. It SHALL name the heavy checks the change needs in the full sweep.

#### Scenario: Web-only change

- **WHEN** only a file under `apps/web/src/` changed
- **THEN** `just test-affected` runs the affected Vitest tests, skips the Rust and Android tests, and names `just e2e` for the full sweep

#### Scenario: Unmapped path

- **WHEN** a file in a new top-level directory changed
- **THEN** `just test-affected` runs the full `just test` and says why
