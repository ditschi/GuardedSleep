# Proposal

## Why

Two rules from AGENTS.md §10.2 and §10.3 exist only on paper, and both failed in practice during wave 2:

- **Timeouts count slot waits.** An outer `timeout N just test` starts counting while the job still queues for a medium or heavy slot, so a queued job dies before it runs and the agent reports a false failure. The rule "treat it as rerun" depends on the agent noticing.
- **Hooks must be active.** A clone that once ran Husky kept `core.hooksPath=.husky/_`, so no pre-commit hook ran and two broken commits landed. Nothing warned until the next `just lint` failed on the content, not on the cause.

Tier T0, tooling only. No change to architecture, scope, the security model or a core concept. Persona impact: none (contributors and agents only).

Out of scope: sccache for worktrees, a Claude Code stop hook for `just reap`, splitting `docs/ci-gates.md` (separate work).

## What Changes

- `scripts/lib/slot.sh` reads `GS_RUN_TIMEOUT`: the deadline starts once the slot is held, so slot waits never count. A run that exceeds it ends with status 124 and a message that names the variable; the slot-wait timeout keeps status 75.
- `just lint` (via `lint-text`) warns locally when the git hooks are not active (stale `core.hooksPath`, missing or non-lefthook `pre-commit`). It never fails and is skipped in CI.
- AGENTS.md §10.2/§10.3, `docs/contributing/development.md` and the lefthook row in `docs/ci-gates.md` describe both.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: the "Git hooks" requirement adds the local hook warning in `just lint`; a new "Load slots" requirement covers the slot semaphore and the run deadline.

## Impact

`scripts/lib/slot.sh`, new `scripts/lib/hooks-check.sh`, `justfile` (`lint-text`), AGENTS.md, `docs/contributing/development.md`, `docs/ci-gates.md`, `openspec/specs/dev-toolchain/`.
