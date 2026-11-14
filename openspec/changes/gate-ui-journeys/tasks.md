# Tasks

Branch from `origin/main` after ditschi/GuardedSleep#61 merged (the journey needs parent pairing). Commit after each task. Heavy runs go through the slot (`GS_RUN_TIMEOUT`).

- [ ] 1. Pin Maestro in `mise.toml`/`mise.lock` with doc link and `[env]` opt-outs; verify tag prefix and lock platforms; `just doctor` lists it; Renovate monthly group. Verify: `mise install`, `maestro --version`, `just doctor`.
- [ ] 2. Runner container: read-only Maestro mount, env forwarding, heap cap; `maestro_assert` with `SKIPPED: gate-ui needs maestro` and strict failure. Run the existing assertion flows and fix their ids. Verify: `GATE_ONLY` run of AC-1 with the assertion flows green locally.
- [ ] 3. AC-12 UI journey: pre-grants, onboarding on both devices, Settings → own relay, project server off, TURN; code transfer by `uiautomator` into a shell variable; paste; verify; test alert; stop; `pm clear` before and after; signaling-log assertion that both joined. `canary-scan.sh --extra-canary-file`, secret dir outside `OUT`, deleted in the EXIT trap. Measure peak RSS and duration, record in design §6. Verify: `DOZE_MINUTES=2 GS_STRICT=1 just gate` green locally; `target/poc-gate` contains no code (grep with the canary).
- [ ] 4. CI: Maestro only in the gate job; job name and header comment; a dispatched run green. Verify: the dispatched run.
- [ ] 5. Docs: `docs/ci-gates.md` (gate row, separate last-catch for AC-12), `development.md` (RAM, `GS_MEDIUM_SLOTS=1`), release checklist item referencing `add-android-release`, AGENTS.md §4, §5.2a (Maestro in CI T0, owner 2026-11-14), §9. Verify: `just lint-text`.
