# Proposal

## Why

Before wave 3 the inner loop of agents and contributors should be as short as it can be, but we do not know where the time goes: the estimate of 12–24 min for a cold worktree turned out to be about 90 s for the Rust part. Every step today runs whole recipes (`just test` runs Rust, WASM, Vitest and the Android JVM tests) even when a change touches one area. Finished agent worktrees pile up (88 GB before the last manual cleanup).

This change adds measurement first, then delta tests, and makes worktree cleanup a command. Tier T0, tooling only; no change to architecture, scope, the security model or a core concept. Persona impact: none (contributors and agents only).

Out of scope, decided after about a week of timing data: emulator snapshot boot, pulling the build image from GHCR, a long-lived Gradle daemon. Also out of scope: CI path filters in `android-native.yml` (`on.paths` cannot read a file), test selection inside Rust or Gradle beyond what their caches already do.

## What Changes

- **Timings:** `slot.sh` appends one line per slot-held run (recipe, class, wait, run time, exit) to a shared `timings.tsv`; `just timings` summarises it.
- **Delta tests:** `just test-affected [base]` maps the changed paths (committed since the merge base with `base`, default `origin/main`, plus uncommitted and untracked files) to areas and runs only those test recipes, with conservative fallbacks to the full `just test`. It prints which heavy checks (`just e2e`, `just gate`, `just build-android`) the change will need in the full sweep.
- **One path map:** the areas live in `.github/path-filters.yml`, read by `ci.yml` (dorny/paths-filter) and by `just test-affected`.
- **Worktree cleanup:** `just reap --worktrees` lists worktrees that are clean and contained in a pushed branch (stale locks of dead processes reported as such); `just reap --prune-worktrees` removes exactly those.
- **Gradle measurement:** the cross-worktree build-cache hit rate is measured and recorded in this change; no Gradle change unless it is poor.
- AGENTS.md §4, §10.1 and §10.2, `docs/contributing/development.md`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: adds "Delta tests", "Run timings" and "Worktree cleanup".

## Impact

`scripts/lib/slot.sh`, new `scripts/lib/affected.mjs` (+ tests), new `scripts/lib/timings.sh`, `scripts/lib/reap.sh`, `justfile`, `.github/path-filters.yml`, `.github/workflows/ci.yml`, AGENTS.md, `docs/contributing/development.md`.
