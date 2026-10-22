# Tasks

## 1. Run deadline in slot.sh

- [ ] 1.1 Brief (implementer): in `scripts/lib/slot.sh`, after a slot is acquired, run the command under `timeout --kill-after=30s "$GS_RUN_TIMEOUT"` when the variable is non-empty, and clear `GS_RUN_TIMEOUT` in the child's environment. Keep the signal forwarding. On status 124 print `error: <class> run exceeded GS_RUN_TIMEOUT=<v> (slot wait not counted)` to stderr. Update the header comment. Acceptance: with `GS_MEDIUM_SLOTS=1`, a holder `slot.sh medium -- sleep 5` plus a second `GS_RUN_TIMEOUT=3 slot.sh medium -- sleep 1` succeeds; `GS_RUN_TIMEOUT=1 slot.sh medium -- sleep 5` exits 124; unset variable behaves as before; shellcheck clean.

## 2. Hook check

- [ ] 2.1 Brief (implementer): add `scripts/lib/hooks-check.sh` per design §2 (exit 0 always, skipped when `CI` is set) and call it first in the `lint-text` recipe. Acceptance: with `git config core.hooksPath .husky/_` the script warns; after `git config --unset core.hooksPath` and lefthook installed it is silent; `CI=1` is silent; shellcheck clean.

## 3. Docs

- [ ] 3.1 AGENTS.md §10.2 ("Timeouts count slot waits") and §10.3 ("Hooks must be active"), `docs/contributing/development.md` (hooks and slots), lefthook row in `docs/ci-gates.md`.

## 4. Verify

- [ ] 4.1 `just lint` and `just test`.
