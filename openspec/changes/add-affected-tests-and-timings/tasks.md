# Tasks

## 1. Timings

- [ ] 1.1 Brief (implementer): `scripts/lib/slot.sh` records a TSV line per slot-held run (design §1); new `scripts/lib/timings.sh [days]` and `just timings`. Acceptance: two slot runs add two lines; an unwritable file does not change the exit status; `just timings` prints a table; shellcheck clean.

## 2. Path map and delta tests

- [ ] 2.1 Brief (implementer): `.github/path-filters.yml` with the areas of design §2; `ci.yml` reads `code` and `android` from it (lists unchanged). Acceptance: `actionlint`, `zizmor` clean; the filter lists equal today's inline ones.
- [ ] 2.2 Brief (implementer): `scripts/lib/affected.mjs` + `node --test` tests (map loading, glob matching, fallbacks, gate list equals `android-native.yml` `on.paths`), `just test-affected [base]` through one medium slot. Acceptance: tests pass; dry runs (`--dry-run` prints the plan) for a docs-only, a web-only, a crates change and an unmapped path give the expected plan.

## 3. Gradle measurement

- [ ] 3.1 Fresh worktree, warm shared cache, `just test-android`: record Gradle's task summary here; remove the worktree.

## 4. Worktree cleanup

- [ ] 4.1 Brief (implementer): `reap.sh --worktrees` / `--prune-worktrees` per design §4; `just reap` passes the flags. Acceptance: on this checkout the listing keeps `agent-a2ad61df…` (commit not in any remote branch); a clean, pushed scratch worktree is listed and pruned; a dirty one is kept; shellcheck clean.

## 5. Docs

- [ ] 5.1 AGENTS.md §4 (commands), §10.1 (targeted checks use `just test-affected`), §10.2 (worktree cleanup); `docs/contributing/development.md`.

## 6. Verify

- [ ] 6.1 `just lint` and `just test` (the full sweep).
