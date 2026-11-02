# Design

## Context

`slot.sh` (AGENTS.md §10.1) is a machine-wide `flock` semaphore. Callers bound their runs with an outer `timeout N`, which also counts the time spent queuing. `just setup` already removes a stale Husky `core.hooksPath`, but only if someone runs it.

## Decisions

### 1. Where the run deadline starts

- [ ] **A. `GS_RUN_TIMEOUT` applied inside `slot.sh` after the slot is acquired (recommended)**
  - Pros: one place, every medium/heavy recipe gets it; wait and run have separate budgets (`GS_SLOT_WAIT`, `GS_RUN_TIMEOUT`) and separate exit codes (75, 124), so "queued too long" and "ran too long" are no longer confused.
  - Cons: recipes without a slot (light) are not covered; callers keep using plain `timeout` there, which is correct because light work never waits.
- [ ] **B. A `just` wrapper recipe (`just timed N <recipe>`)**
  - Pros: visible in `just --list`.
  - Cons: it cannot see when the slot is acquired, so it has the same flaw as the outer `timeout`.

Recommendation: A.

Semantics:

- Unset or empty: no deadline (today's behaviour).
- The value is any GNU `timeout` duration (`900`, `15m`). The command runs under `timeout --kill-after=30s`; TERM first, KILL 30 s later.
- The deadline is applied only by the `slot.sh` call that acquires a slot. It clears `GS_RUN_TIMEOUT` for the child, so a nested acquisition (heavy inside medium) runs inside the outer deadline instead of starting a new one. Re-entrant calls for the same or a lighter class do not acquire, so they never apply it either.
- Sibling acquisitions in one recipe chain (for example `just ci` running `lint-code`, then `test`) each get their own deadline: they are separate slot holds.
- Exit 124 with `error: <class> run exceeded GS_RUN_TIMEOUT=<v> (slot wait not counted)`.

Agent usage (AGENTS.md §10.2): foreground calls use `GS_SLOT_WAIT=60 GS_RUN_TIMEOUT=480 just <recipe>`, so a busy machine returns 75 quickly ("rerun or background it") instead of eating the 10-minute tool limit; background runs use a long `GS_RUN_TIMEOUT` and no outer `timeout`.

### 2. How the hook check reports

- [ ] **A. Warning in `just lint`, local only (recommended)**
  - Pros: `just lint` is what every agent and contributor runs before "done"; a warning does not block someone who deliberately runs without hooks.
  - Cons: a warning can be ignored.
- [ ] **B. Hard failure locally**
  - Pros: impossible to ignore.
  - Cons: `just lint` would disagree with CI (where hooks are irrelevant), breaking "`just ci` gives the same verdict as CI".

Recommendation: A. Skipped when `CI` is set. It is not a gate, so `docs/ci-gates.md` gets no new row; the existing lefthook row mentions it.

Checks, using `git rev-parse --git-path hooks` (respects `core.hooksPath` and linked worktrees):

- the hooks path contains `.husky` → "stale Husky core.hooksPath";
- `<hooks>/pre-commit` is missing, not executable or does not mention lefthook → "pre-commit hook not installed".

Each prints one `warning:` line to stderr ending in "run 'just setup'" and the script exits 0.

## Guardrails and layers

Tooling only; no product layer, no effect on the privacy guardrails. The hook check only reads git config and files.

## Discarded alternatives

- **Outer `timeout` with a larger margin** (current AGENTS.md advice): a margin large enough for the worst queue makes the deadline meaningless for the run itself.
- **A commit-msg or post-checkout hook that verifies hooks**: a hook cannot report that hooks are off.
