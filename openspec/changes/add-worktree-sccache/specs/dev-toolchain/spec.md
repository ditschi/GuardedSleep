# Spec Delta

## ADDED Requirements

### Requirement: Local compiler cache

Local Rust builds in the build image SHALL use a shared, size-capped sccache directory so a new worktree reuses compiled dependencies. The cache SHALL be off in CI, for the generate-twice check and for release builds, and `GS_SCCACHE=0` SHALL turn it off locally.

#### Scenario: Second worktree builds warm

- **WHEN** one worktree has built the workspace and a new worktree on another branch runs `just test`
- **THEN** the dependency crates are served from the cache instead of being compiled again

#### Scenario: Determinism check bypasses the cache

- **WHEN** `just check-determinism` runs with a warm cache
- **THEN** both builds compile without sccache, so a non-deterministic output still fails the check
