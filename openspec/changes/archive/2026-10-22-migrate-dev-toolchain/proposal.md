# Proposal

## Why

The toolchain decides what "done" means for every change: which checks run, where versions are pinned, how secrets and licences are guarded. Wave 1 of ADR-020 built it; ADR-007, ADR-016 and ADR-019 decided the test and language policy. This change records the built toolchain as the `dev-toolchain` spec and closes the ADR folder (tier T0, docs only).

## What Changes

- New capability spec `dev-toolchain`: `just` as the entry point, SKIPPED and strict mode, version pins, git hooks, CI gates and branch protection, supply-chain and licence checks, secret scanning, generated code, the test layers, Rust rules and the gate register.
- Decided but not built (wave 2/3: façade crates, API snapshots, generate-twice hash, fuzzing, log canary, size budgets, Gradle modules, ktfmt in `fmt`, design tokens, the generated Dev Container) goes into the prepared wave changes or the backlog.
- Archived: ADR-002 (superseded), ADR-007, ADR-014 (licence; REUSE is specified here), ADR-015 (architecture; rules stay in AGENTS.md §5.1), ADR-016, ADR-019. ADR-020 is archived after the prepared changes cover its remaining sections.

## Capabilities

### New Capabilities

- `dev-toolchain`: the commands, pins, hooks and CI gates every change in this repository must pass, and the test layers that prove device behaviour.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/dev-toolchain/`, the listed ADRs (archived), ADR-020 note, `docs/backlog.md`.
