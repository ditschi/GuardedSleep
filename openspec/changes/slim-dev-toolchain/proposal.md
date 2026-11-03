# Proposal

## Why

The `dev-toolchain` spec has 33 requirements, and several bind implementation details instead of behaviour: exit codes 75 and 124 of the slot semaphore, the column layout of the timings TSV, sccache mounts, the stale-Husky warning and the worktree prune rules (spec lines 36-48 and 267-325). Every tooling tweak therefore needs a full propose/archive cycle, which eats into the 12 h/month tooling budget it is meant to protect. The orchestration helpers built in wave 3 (timings, machine-profile detection, worktree pruning) and the weekly Swift smoke cost maintenance and catch nothing at alpha. Two Rust execution modes (host cargo in `scripts/cargo.sh`, Docker by default in `scripts/core-build.sh`) compile into the same `target/` from different paths and invalidate each other's build output.

Now, because the repository became public on 2026-11-02: branch protection on GitHub Free is enforceable, so `ci-ok` and linear history can finally be required. Also, the merge rule in AGENTS.md §8 (rebase for a clean history, otherwise squash) is not yet in the spec. The owner approved all five items on 2026-11-02.

Tier **T0** (alpha). Tooling only: no product behaviour, security model or core concept changes. It does revise prior decisions (ADR-020: the weekly Swift smoke, the per-gate register, "protection not enforced while private"), so the Critical Review Gate applies (AGENTS.md §6); its findings and answers are in design.md.

**Persona impact:** none directly (docs/concept/personas.md). The only user is the contributor; the maintainer's tooling budget gains time for alpha features.

## What Changes

- **Spec slimmed to invariants.** Keep: a single `just` entry point, SKIPPED + `GS_STRICT`, one place per pin, `ci-ok` and branch protection, supply chain, gate register and budget. Move the "how" (exit codes, environment variables, file formats, cache layout, hook-repair details) into `docs/contributing/development.md`.
  - MODIFIED: CI and branch protection, Rust rules, Gate register and budget, Load slots, Delta tests.
  - ADDED: Rust execution mode; Pre-commit hooks (replaces Git hooks without the Husky scenario, which OpenSpec does not allow a MODIFIED block to drop).
  - REMOVED: Git hooks (replaced), Local compiler cache (moved to the docs), Run timings (dropped), Worktree cleanup (pruning dropped; the listing moves to the docs). 33 → 31 requirements.
- **Gate register shrunk:** `docs/ci-gates.md` goes from 30 rows to one row per CI job or scheduled workflow (about 12), without the Owner column. The maintainer owns every gate during alpha, and the spec says so. Checks that run inside a job are listed in its row. This removes a merge hotspot named in AGENTS.md §10.3.
- **Swift smoke dropped:** delete `.github/workflows/swift-bindings.yml` and `scripts/core-build.sh swift`. Keep `cargo check --target aarch64-apple-ios` in `just lint`. The iOS gate will reintroduce binding generation when iOS work starts (AGENTS.md §9).
- **Agent orchestration slimmed:** keep the `slot.sh` flock semaphore (with `GS_SLOT_WAIT` and `GS_RUN_TIMEOUT`) and the `just reap` listing, including `--worktrees`. Drop the following:
  - `scripts/lib/timings.sh`, `just timings` and the recording in `slot.sh`;
  - `scripts/lib/machine-profile.sh` and `just machine-profile`: slot counts get documented defaults plus `GS_MEDIUM_SLOTS` and `GS_HEAVY_SLOTS` overrides;
  - `reap --prune-worktrees`.

  AGENTS.md §10.1-10.3 is updated to match.

- **One Rust execution mode:** host rustup (reading `rust-toolchain.toml`) is the default for every Rust command, including `core-build.sh test|wasm|host`. The build image is used for the Android chain (ABI libraries, the core library the JVM tests load, Gradle: the image's glibc is older than a current host's, so a host-built library cannot be loaded there), for the local determinism check, and as the fallback when no host cargo exists. Those image builds write to their own target directory, so switching modes does not invalidate `target/`; on a machine without host rustup the image fallback uses `target/` itself, since there is no host build to protect.
- **Merge and protection rule:** the spec states the merge rule (rebase and merge for a clean Conventional history, otherwise squash; no merge commits). The owner configures `ci-ok` as a required check and linear history on `main` in the GitHub settings (a manual task for the owner). AGENTS.md §8 drops "not enforced while private".

Out of scope: new gates, changes to CI job content beyond removing the Swift workflow, deleting `docs/handoff-wave-3.md` (separate cleanup), and tiers T1 and T2 (cargo-vet, sccache in CI).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `dev-toolchain`: The listed requirements are reduced to invariants, and three detail requirements are removed. The Swift job is removed from "Rust rules". A new "Rust execution mode" requirement states the single mode, "Pre-commit hooks" replaces "Git hooks", and "CI and branch protection" gains the merge rule and enforced protection.

## Impact

- `openspec/specs/dev-toolchain/spec.md` (through archive), `docs/contributing/development.md`, `docs/ci-gates.md`, AGENTS.md (§3, §5.2, §8, §10.1-10.3).
- `justfile` (`setup`, `machine-profile`, `timings`, `reap`), `scripts/lib/{slot,reap}.sh`, deleted `scripts/lib/{timings,machine-profile}.sh`, `scripts/core-build.sh`, `scripts/cargo.sh`, `scripts/gradle.sh`, `scripts/lib/build-image.sh`, `scripts/lib/require.sh`, `apps/android/{core-bindings,platform}/build.gradle.kts` (library path), `apps/android/build-logic` (`CARGO_HOME` default), `docs/backlog.md` (iOS row), `.devcontainer/Dockerfile` (comment), `.github/workflows/swift-bindings.yml` (deleted).
- CI is unchanged: it already sets `CORE_BUILD_NATIVE=1` and runs host cargo.
- GitHub repository settings: a manual task for the owner.
- No runtime, privacy or security effect on the product.
