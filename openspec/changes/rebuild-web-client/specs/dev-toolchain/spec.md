# Spec Delta

## ADDED Requirements

### Requirement: Web checks

The web client SHALL pass `svelte-check --fail-on-warnings` (a suppression needs an inline reason), Vitest for logic, and Playwright tests with fake media devices, axe and an overflow check. pnpm SHALL run with an install-script allow-list, a minimum release age of 3 days, blocked exotic sub-dependencies and a frozen lockfile in CI.

#### Scenario: Accessibility regression

- **WHEN** a change removes the label of a button on the web parent screen
- **THEN** the Playwright axe check fails
