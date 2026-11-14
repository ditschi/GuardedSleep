# Tasks

Branch `test/ui-journey-tests`; the pairing journeys need ditschi/GuardedSleep#61 (parent pairing), so rebase onto `origin/main` once it is merged (or stack on `fix/parent-pairing-entry` until then). Commit after each task; `just test-affected` after each task.

## 1. Contract completeness

- [ ] 1.1 Remove the default bodies from the Kotlin `AppController`; implement every method in `LinkController`, `FakeAppController` and previews. Verify: build.
- [ ] 1.2 Implement the exposed gaps per the specs: `reportTestResult` (`parent-alerts` "Test alert"), `coachPending`/`dismissCoach` (first-run coach, alpha-ux-flows §3.2 step 4), Settings → Test alert while not monitoring (no hidden probe sheet). Verify: `LinkControllerTest`.
- [ ] 1.3 Writer test for every UI contract state field (allowlist with reasons). Verify: the test fails on a planted unwritten field.

## 2. Tier 1 journeys

- [ ] 2.1 Move `FakeNet`, fake media and the controllable clock into shared test fixtures used by `platform` and `app` tests.
- [ ] 2.2 Journey harness: real `LinkController` per device on one `FakeNet`, render `GuardedSleepRoot` for one device, tag-based steps, no sleeps.
- [ ] 2.3 Journeys: first run baby and parent with pairing by pasted code; start and stop on both sides; saved connections, default and automatic start, "Choose another connection"; "Pair again"; noise alert with stop sound, snooze and acknowledge; test alert with the answer; kiosk lock and unlock; link loss and recovery; device check result; switch mode; Settings test alert. Verify: `just test`; each journey fails when its controller method is stubbed out (checked once by hand, noted in the PR).

## 3. Tier 2 on emulators

- [ ] 3.1 Pin Maestro in `mise.toml`/`mise.lock` with a doc link; `just doctor` lists it; the gate runner uses the same binary; `maestro_assert` fails under `GS_STRICT=1` without it. Verify: `just doctor`; the existing assertion flows run.
- [ ] 3.2 UI journey flows: onboarding on both emulators, code transfer by `uiautomator` with log-canary registration, paste and verify, monitor, test alert, stop, "Choose another connection". Verify: `DOZE_MINUTES=2 just gate` locally, flows reported in `target/poc-gate`.
- [ ] 3.3 CI: install Maestro in the gate job; release checklist requires the gate on the tagged commit. Verify: a dispatched gate run is green.

## 4. Docs

- [ ] 4.1 `docs/ci-gates.md` (gate row: UI journeys; test job row: journeys), AGENTS.md §5.2a and §9 (Maestro in CI now T0), `docs/contributing` test guide (when to run what), backlog cleanup. Verify: `just lint-text`.
